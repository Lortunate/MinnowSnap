//! Desired preference acceptance and main-thread native activation.
use crate::{OperationError, OperationResult};
use minnow_settings::{
    AppSettings, PersistenceStatus, SettingsAction, SettingsError, SettingsHandle, SettingsLoadStatus, SettingsRevision, ShortcutSettings,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectiveShortcuts {
    pub capture: Option<String>,
    pub quick_capture: Option<String>,
    pub unrouted: Vec<String>,
    pub uncertain: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct ShortcutUpdateFailure {
    pub message: String,
    pub actual: Box<EffectiveShortcuts>,
    pub compensation_errors: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativePreferencesSnapshot {
    pub auto_start: OperationResult<bool>,
    pub auto_start_failure: Option<OperationError>,
    pub shortcuts: EffectiveShortcuts,
    pub shortcut_failure: Option<ShortcutUpdateFailure>,
}

/// The set attempt and the observed registration state are independent facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AutoStartUpdate {
    pub actual: OperationResult<bool>,
    pub failure: Option<OperationError>,
}

/// Implemented by a main-thread adapter; it is never stored in Send application services.
pub trait NativePreferences {
    fn set_auto_start(&self, enabled: bool) -> AutoStartUpdate;
    fn apply_shortcuts(&self, proposed: &ShortcutSettings) -> Result<EffectiveShortcuts, ShortcutUpdateFailure>;
    fn snapshot(&self) -> NativePreferencesSnapshot;
    /// Explicitly refresh OS state, for example when opening preferences.
    fn refresh(&self) -> NativePreferencesSnapshot;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativePreferenceFailure {
    AutoStart(OperationError),
    Shortcuts(ShortcutUpdateFailure),
}

#[derive(Clone, Debug)]
pub struct PreferenceChange {
    pub revision: SettingsRevision,
    pub persistence: PersistenceStatus,
    pub native: NativePreferencesSnapshot,
    pub native_failure: Option<NativePreferenceFailure>,
}

#[derive(Clone)]
pub struct PreferencesService {
    settings: SettingsHandle,
}

impl PreferencesService {
    pub fn new(settings: SettingsHandle) -> Self {
        Self { settings }
    }

    pub fn snapshot(&self) -> AppSettings {
        self.settings.snapshot()
    }

    pub fn load_status(&self) -> SettingsLoadStatus {
        self.settings.load_status()
    }

    pub fn persistence_status(&self) -> PersistenceStatus {
        self.settings.persistence_status()
    }

    pub fn refresh_native<N: NativePreferences + ?Sized>(&self, native: &N) -> NativePreferencesSnapshot {
        native.refresh()
    }

    /// First accept desired intent. A native failure remains observable separately;
    /// an asynchronous disk failure never rolls back an already activated OS change.
    pub fn apply<N: NativePreferences + ?Sized>(&self, action: SettingsAction, native: &N) -> Result<PreferenceChange, SettingsError> {
        let (revision, desired) = self.settings.apply_with_snapshot(action.clone())?;
        let mut snapshot = native.snapshot();
        let native_failure = match action {
            SettingsAction::AutoStart(enabled) => {
                let update = native.set_auto_start(enabled);
                snapshot.auto_start = update.actual;
                snapshot.auto_start_failure = update.failure.clone();
                update.failure.map(NativePreferenceFailure::AutoStart)
            }
            SettingsAction::CaptureShortcut(_) | SettingsAction::QuickCaptureShortcut(_) | SettingsAction::Shortcuts { .. } => {
                match native.apply_shortcuts(&desired.shortcuts) {
                    Ok(actual) => {
                        snapshot.shortcuts = actual;
                        snapshot.shortcut_failure = None;
                        None
                    }
                    Err(error) => {
                        snapshot.shortcuts = error.actual.as_ref().clone();
                        snapshot.shortcut_failure = Some(error.clone());
                        Some(NativePreferenceFailure::Shortcuts(error))
                    }
                }
            }
            _ => None,
        };
        Ok(PreferenceChange {
            revision,
            persistence: self.persistence_status(),
            native: snapshot,
            native_failure,
        })
    }

    /// Retry the latest accepted shortcuts without creating another desired
    /// revision or implying that disk persistence has succeeded.
    pub fn retry_shortcuts<N: NativePreferences + ?Sized>(&self, native: &N) -> Result<EffectiveShortcuts, ShortcutUpdateFailure> {
        native.apply_shortcuts(&self.snapshot().shortcuts)
    }

    /// Missing settings may install the application's default shortcuts. Only a
    /// loaded file is authoritative enough to replay auto-start intent. Degraded
    /// defaults never change either OS registration.
    pub fn activate_saved<N: NativePreferences + ?Sized>(&self, native: &N) -> Vec<NativePreferenceFailure> {
        let mut failures = Vec::new();
        let settings = self.snapshot();
        match self.load_status() {
            SettingsLoadStatus::Loaded => {
                if let Some(error) = native.set_auto_start(settings.general.auto_start).failure {
                    failures.push(NativePreferenceFailure::AutoStart(error));
                }
            }
            SettingsLoadStatus::Missing => {}
            SettingsLoadStatus::InMemory | SettingsLoadStatus::Degraded { .. } => return failures,
        }
        if let Err(error) = native.apply_shortcuts(&settings.shortcuts) {
            failures.push(NativePreferenceFailure::Shortcuts(error));
        }
        failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;
    use minnow_settings::SettingsStore;
    use std::cell::RefCell;
    use std::time::Duration;

    struct Native {
        calls: RefCell<Vec<&'static str>>,
        proposed_shortcuts: RefCell<Vec<ShortcutSettings>>,
        state: NativePreferencesSnapshot,
    }

    impl Native {
        fn unavailable() -> Self {
            Self {
                calls: RefCell::new(Vec::new()),
                proposed_shortcuts: RefCell::new(Vec::new()),
                state: NativePreferencesSnapshot {
                    auto_start: Ok(false),
                    auto_start_failure: None,
                    shortcuts: EffectiveShortcuts::default(),
                    shortcut_failure: None,
                },
            }
        }
    }

    impl NativePreferences for Native {
        fn set_auto_start(&self, _: bool) -> AutoStartUpdate {
            self.calls.borrow_mut().push("auto-start");
            AutoStartUpdate {
                actual: self.state.auto_start.clone(),
                failure: Some(OperationError::new(ErrorKind::Native, "OS rejected registration")),
            }
        }
        fn apply_shortcuts(&self, proposed: &ShortcutSettings) -> Result<EffectiveShortcuts, ShortcutUpdateFailure> {
            self.calls.borrow_mut().push("shortcuts");
            self.proposed_shortcuts.borrow_mut().push(proposed.clone());
            Err(ShortcutUpdateFailure {
                message: "registration failed".into(),
                actual: Box::new(EffectiveShortcuts {
                    capture: Some("F1".into()),
                    quick_capture: Some("F2".into()),
                    unrouted: vec!["F3".into()],
                    uncertain: vec!["F4".into()],
                }),
                compensation_errors: vec!["failed to remove F3".into()],
            })
        }
        fn snapshot(&self) -> NativePreferencesSnapshot {
            self.state.clone()
        }
        fn refresh(&self) -> NativePreferencesSnapshot {
            self.snapshot()
        }
    }

    struct ReturnedNative {
        baseline: NativePreferencesSnapshot,
        auto_start: AutoStartUpdate,
        shortcuts: EffectiveShortcuts,
    }

    impl NativePreferences for ReturnedNative {
        fn set_auto_start(&self, _: bool) -> AutoStartUpdate {
            self.auto_start.clone()
        }
        fn apply_shortcuts(&self, _: &ShortcutSettings) -> Result<EffectiveShortcuts, ShortcutUpdateFailure> {
            Ok(self.shortcuts.clone())
        }
        fn snapshot(&self) -> NativePreferencesSnapshot {
            self.baseline.clone()
        }
        fn refresh(&self) -> NativePreferencesSnapshot {
            self.snapshot()
        }
    }

    #[test]
    fn successful_shortcut_apply_uses_returned_actual_state_and_clears_stale_failure_without_cache_dual_write() {
        let preferences = PreferencesService::new(SettingsStore::in_memory(AppSettings::default()));
        let mut baseline = Native::unavailable().state;
        baseline.shortcut_failure = Some(ShortcutUpdateFailure {
            message: "older attempt failed".into(),
            actual: Box::default(),
            compensation_errors: vec![],
        });
        let actual = EffectiveShortcuts {
            capture: Some("F8".into()),
            quick_capture: Some("F9".into()),
            ..Default::default()
        };
        let native = ReturnedNative {
            baseline,
            auto_start: AutoStartUpdate {
                actual: Ok(false),
                failure: None,
            },
            shortcuts: actual.clone(),
        };
        let change = preferences.apply(SettingsAction::CaptureShortcut("F3".into()), &native).unwrap();
        assert_eq!(preferences.snapshot().shortcuts.capture, "F3");
        assert_eq!(change.native.shortcuts, actual);
        assert!(change.native.shortcut_failure.is_none());
        assert!(change.native_failure.is_none());
        assert!(
            native.snapshot().shortcut_failure.is_some(),
            "the adapter deliberately does not update its cached snapshot"
        );
    }

    #[test]
    fn auto_start_observation_and_set_failure_are_consumed_independently_of_the_cached_snapshot() {
        let query_error = OperationError::new(ErrorKind::Native, "cannot query registration");
        let set_error = OperationError::new(ErrorKind::Native, "cannot set registration");
        for update in [
            AutoStartUpdate {
                actual: Ok(true),
                failure: None,
            },
            AutoStartUpdate {
                actual: Err(query_error),
                failure: None,
            },
            AutoStartUpdate {
                actual: Ok(false),
                failure: Some(set_error),
            },
        ] {
            let preferences = PreferencesService::new(SettingsStore::in_memory(AppSettings::default()));
            let mut baseline = Native::unavailable().state;
            baseline.auto_start_failure = Some(OperationError::new(ErrorKind::Native, "older attempt failed"));
            let native = ReturnedNative {
                baseline,
                auto_start: update.clone(),
                shortcuts: EffectiveShortcuts::default(),
            };
            let change = preferences.apply(SettingsAction::AutoStart(true), &native).unwrap();
            assert!(
                preferences.snapshot().general.auto_start,
                "desired intent is independent of actual OS state"
            );
            assert_eq!(change.native.auto_start, update.actual);
            assert_eq!(change.native.auto_start_failure, update.failure);
            assert_eq!(change.native_failure, update.failure.map(NativePreferenceFailure::AutoStart));
            assert!(matches!(change.persistence, PersistenceStatus::InMemory { revision } if revision == change.revision));
            assert!(
                native.snapshot().auto_start_failure.is_some(),
                "this apply must not depend on a second snapshot write"
            );
        }
    }

    #[test]
    fn editing_one_shortcut_keeps_the_other_desired_value_after_native_failure() {
        let preferences = PreferencesService::new(SettingsStore::in_memory(AppSettings::default()));
        let native = Native::unavailable();
        preferences
            .apply(SettingsAction::CaptureShortcut("F3".into()), &native)
            .expect("accept capture intent");
        let change = preferences
            .apply(SettingsAction::QuickCaptureShortcut("F4".into()), &native)
            .expect("accept quick capture intent");

        let desired = ShortcutSettings {
            capture: "F3".into(),
            quick_capture: "F4".into(),
        };
        assert_eq!(preferences.snapshot().shortcuts, desired);
        assert_eq!(native.proposed_shortcuts.borrow().last(), Some(&desired));
        assert_eq!(change.native.shortcuts.capture.as_deref(), Some("F1"));
        assert_eq!(change.native.shortcuts.quick_capture.as_deref(), Some("F2"));
        assert!(change.native_failure.is_some());

        preferences.retry_shortcuts(&native).expect_err("native registration still unavailable");
        assert_eq!(native.proposed_shortcuts.borrow().last(), Some(&desired));
        assert_eq!(
            preferences.settings.revision(),
            change.revision,
            "retry must not accept another desired revision"
        );
        assert_eq!(
            preferences.persistence_status(),
            change.persistence,
            "native retry must not claim another write"
        );
    }

    #[test]
    fn rejected_desired_change_never_calls_the_os() {
        let settings = SettingsStore::in_memory(AppSettings::default());
        settings.shutdown(Duration::ZERO).expect("close settings admission");
        let native = Native::unavailable();
        let preferences = PreferencesService::new(settings);
        assert!(matches!(
            preferences.apply(SettingsAction::AutoStart(true), &native),
            Err(SettingsError::Closed)
        ));
        assert!(native.calls.borrow().is_empty());
    }

    #[test]
    fn native_failure_preserves_accepted_desired_intent_and_complete_failure() {
        let preferences = PreferencesService::new(SettingsStore::in_memory(AppSettings::default()));
        let native = Native::unavailable();
        let change = preferences
            .apply(
                SettingsAction::Shortcuts {
                    capture: "F3".into(),
                    quick_capture: "F4".into(),
                },
                &native,
            )
            .expect("desired intent accepted");
        assert_eq!(preferences.snapshot().shortcuts.capture, "F3");
        assert!(matches!(change.persistence, PersistenceStatus::InMemory { revision } if revision == change.revision));
        let Some(NativePreferenceFailure::Shortcuts(error)) = change.native_failure else {
            panic!("retain native failure");
        };
        assert_eq!(error.actual.unrouted, ["F3"]);
        assert_eq!(error.actual.uncertain, ["F4"]);
        assert_eq!(error.compensation_errors, ["failed to remove F3"]);
        assert_eq!(change.native.shortcuts, *error.actual);
        assert_eq!(change.native.shortcut_failure.as_ref(), Some(&error));
    }

    #[test]
    fn degraded_unknown_saved_intent_never_replays_defaults_to_the_os() {
        let settings = SettingsStore::degraded(
            AppSettings::default(),
            SettingsError::Parse {
                path: "config.toml".into(),
                message: "invalid TOML".into(),
            },
        );
        let preferences = PreferencesService::new(settings);
        let native = Native::unavailable();
        assert!(preferences.activate_saved(&native).is_empty());
        assert!(native.calls.borrow().is_empty());
    }
}

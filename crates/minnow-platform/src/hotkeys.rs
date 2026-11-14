use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyManager, HotKeyState};
use minnow_application::settings::{DEFAULT_CAPTURE_SHORTCUT, DEFAULT_QUICK_CAPTURE_SHORTCUT};
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

pub use global_hotkey::GlobalHotKeyEvent;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ShortcutAction {
    Capture,
    QuickCapture,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutBindings {
    pub capture: String,
    pub quick_capture: String,
}

impl Default for ShortcutBindings {
    fn default() -> Self {
        Self {
            capture: DEFAULT_CAPTURE_SHORTCUT.into(),
            quick_capture: DEFAULT_QUICK_CAPTURE_SHORTCUT.into(),
        }
    }
}

impl ShortcutBindings {
    pub fn validate(&self) -> Result<(), String> {
        parse_bindings(self).map(|_| ())
    }
}

/// Confirmed routed registrations plus any native registration that could not
/// be removed during compensation. An unrouted key is never hidden as success.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectiveBindings {
    pub capture: Option<String>,
    pub quick_capture: Option<String>,
    pub unrouted: Vec<String>,
    /// Native removal failed after the backend discarded its handle. These
    /// keys may still be reserved, but are never presented as working routes.
    pub uncertain: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct HotkeyUpdateFailure {
    pub message: String,
    pub actual: Box<EffectiveBindings>,
    pub compensation_errors: Vec<String>,
}

#[derive(Clone)]
struct Binding {
    hotkey: HotKey,
    label: String,
}

trait RegistrationBackend {
    fn register(&mut self, key: HotKey) -> Result<(), String>;
    fn unregister(&mut self, key: HotKey) -> Result<(), String>;
    fn uncertain_registration(&self, _key: HotKey) -> bool {
        false
    }
}

struct NativeBackend {
    manager: GlobalHotKeyManager,
    #[cfg(target_os = "macos")]
    failed_removals: BTreeMap<u32, String>,
}

impl RegistrationBackend for NativeBackend {
    fn register(&mut self, key: HotKey) -> Result<(), String> {
        let result = self.manager.register(key).map_err(|error| error.to_string());
        #[cfg(target_os = "macos")]
        if let Err(error) = &result {
            // Media-key registration inserts into the dependency's local set
            // before requesting event-tap permission. Roll back that local
            // insertion as well, otherwise retry can report AlreadyRegistered.
            if let Err(cleanup) = self.manager.unregister(key) {
                self.failed_removals.insert(key.id(), cleanup.to_string());
                return Err(format!("{error}; failed to roll back native registration: {cleanup}"));
            }
        }
        result
    }

    fn uncertain_registration(&self, key: HotKey) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.failed_removals.contains_key(&key.id())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = key;
            false
        }
    }

    fn unregister(&mut self, key: HotKey) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        if let Some(error) = self.failed_removals.get(&key.id()) {
            return Err(format!("native removal cannot safely be retried: {error}"));
        }
        let result = self.manager.unregister(key).map_err(|error| error.to_string());
        // global-hotkey's macOS backend removes its Carbon handle before asking the OS to
        // unregister. A retry after an OS failure would return a false success,
        // so keep the failed registration observable for the process lifetime.
        #[cfg(target_os = "macos")]
        if let Err(error) = &result {
            self.failed_removals.insert(key.id(), error.clone());
        }
        result
    }
}

struct RegistryState<B> {
    backend: B,
    active: BTreeMap<ShortcutAction, Binding>,
    registered: BTreeMap<u32, Binding>,
}

impl<B: RegistrationBackend> RegistryState<B> {
    fn new(backend: B) -> Self {
        Self {
            backend,
            active: BTreeMap::new(),
            registered: BTreeMap::new(),
        }
    }

    fn current(&self) -> EffectiveBindings {
        let label = |action| {
            self.active
                .get(&action)
                .filter(|binding| !self.backend.uncertain_registration(binding.hotkey))
                .map(|binding| binding.label.clone())
        };
        EffectiveBindings {
            capture: label(ShortcutAction::Capture),
            quick_capture: label(ShortcutAction::QuickCapture),
            unrouted: self
                .registered
                .iter()
                .filter(|(id, binding)| {
                    !self.backend.uncertain_registration(binding.hotkey) && !self.active.values().any(|binding| binding.hotkey.id() == **id)
                })
                .map(|(_, binding)| binding.label.clone())
                .collect(),
            uncertain: self
                .registered
                .values()
                .filter(|binding| self.backend.uncertain_registration(binding.hotkey))
                .map(|binding| binding.label.clone())
                .collect(),
        }
    }

    fn apply(&mut self, requested: &ShortcutBindings) -> Result<EffectiveBindings, HotkeyUpdateFailure> {
        let desired = parse_bindings(requested).map_err(|message| self.failure(message, Vec::new()))?;
        self.replace(desired)
    }

    fn clear(&mut self) -> Result<EffectiveBindings, HotkeyUpdateFailure> {
        self.replace(BTreeMap::new())
    }

    fn replace(&mut self, desired: BTreeMap<ShortcutAction, Binding>) -> Result<EffectiveBindings, HotkeyUpdateFailure> {
        if !self.current().uncertain.is_empty() {
            return Err(self.failure(
                "native shortcut ownership cannot be confirmed; restart the application before reconfiguring shortcuts".into(),
                Vec::new(),
            ));
        }
        let previous = self.registered.clone();
        let desired_by_id: BTreeMap<_, _> = desired.values().map(|binding| (binding.hotkey.id(), binding.clone())).collect();
        let additions: Vec<_> = desired_by_id
            .iter()
            .filter(|(id, _)| !self.registered.contains_key(id))
            .map(|(_, binding)| binding.clone())
            .collect();

        // Keep old shortcuts working until all genuinely new keys are reserved.
        // A swap of the two actions needs only a routing update, not an OS gap.
        for binding in additions {
            if let Err(error) = self.backend.register(binding.hotkey) {
                if self.backend.uncertain_registration(binding.hotkey) {
                    self.registered.insert(binding.hotkey.id(), binding.clone());
                }
                let compensation = self.restore_registered(&previous);
                self.reconcile_active();
                return Err(self.failure(format!("failed to register {}: {error}", binding.label), compensation));
            }
            self.registered.insert(binding.hotkey.id(), binding);
        }

        let removals: Vec<_> = self
            .registered
            .iter()
            .filter(|(id, _)| !desired_by_id.contains_key(id))
            .map(|(_, binding)| binding.clone())
            .collect();
        for binding in removals {
            if let Err(error) = self.backend.unregister(binding.hotkey) {
                let compensation = self.restore_registered(&previous);
                self.reconcile_active();
                return Err(self.failure(format!("failed to unregister {}: {error}", binding.label), compensation));
            }
            self.registered.remove(&binding.hotkey.id());
        }

        self.active = desired;
        self.registered = desired_by_id;
        Ok(self.current())
    }

    fn restore_registered(&mut self, previous: &BTreeMap<u32, Binding>) -> Vec<String> {
        let mut errors = Vec::new();
        let missing: Vec<_> = previous
            .iter()
            .filter(|(id, _)| !self.registered.contains_key(id))
            .map(|(_, binding)| binding.clone())
            .collect();
        for binding in missing {
            match self.backend.register(binding.hotkey) {
                Ok(()) => {
                    self.registered.insert(binding.hotkey.id(), binding);
                }
                Err(error) => errors.push(format!("restore {}: {error}", binding.label)),
            }
        }
        let extras: Vec<_> = self
            .registered
            .iter()
            .filter(|(id, _)| !previous.contains_key(id))
            .map(|(_, binding)| binding.clone())
            .collect();
        for binding in extras {
            match self.backend.unregister(binding.hotkey) {
                Ok(()) => {
                    self.registered.remove(&binding.hotkey.id());
                }
                Err(error) => errors.push(format!("remove provisional {}: {error}", binding.label)),
            }
        }
        errors
    }

    fn reconcile_active(&mut self) {
        self.active
            .retain(|_, binding| self.registered.contains_key(&binding.hotkey.id()) && !self.backend.uncertain_registration(binding.hotkey));
    }

    fn failure(&self, message: String, compensation_errors: Vec<String>) -> HotkeyUpdateFailure {
        HotkeyUpdateFailure {
            message,
            actual: Box::new(self.current()),
            compensation_errors,
        }
    }
}

fn parse_bindings(bindings: &ShortcutBindings) -> Result<BTreeMap<ShortcutAction, Binding>, String> {
    let defaults = ShortcutBindings::default();
    let mut parsed = BTreeMap::new();
    for (action, requested, default) in [
        (ShortcutAction::Capture, &bindings.capture, defaults.capture.as_str()),
        (ShortcutAction::QuickCapture, &bindings.quick_capture, defaults.quick_capture.as_str()),
    ] {
        let label = if requested.trim().is_empty() { default } else { requested.trim() };
        let hotkey = HotKey::from_str(label).map_err(|error| format!("invalid shortcut {label}: {error}"))?;
        parsed.insert(
            action,
            Binding {
                hotkey,
                label: label.to_string(),
            },
        );
    }
    if parsed[&ShortcutAction::Capture].hotkey.id() == parsed[&ShortcutAction::QuickCapture].hotkey.id() {
        return Err("capture and quick capture cannot use the same shortcut".into());
    }
    Ok(parsed)
}

#[derive(Clone, Default)]
pub struct EventRouter(Arc<Mutex<BTreeMap<u32, ShortcutAction>>>);

impl EventRouter {
    pub fn action_for_event(&self, event: &GlobalHotKeyEvent) -> Option<ShortcutAction> {
        if event.state != HotKeyState::Pressed {
            return None;
        }
        self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).get(&event.id).copied()
    }
}

/// Own on the native event-loop thread. The sendable event router contains no
/// native manager and may be used by the host's event callback. The host owns
/// event delivery: global-hotkey's process-wide handler can only be set once.
pub struct NativeHotkeyRegistry {
    state: RegistryState<NativeBackend>,
    router: EventRouter,
    _event_thread: PhantomData<Rc<()>>,
}

impl NativeHotkeyRegistry {
    pub fn new() -> Result<Self, HotkeyUpdateFailure> {
        #[cfg(target_os = "macos")]
        if objc2::MainThreadMarker::new().is_none() {
            return Err(HotkeyUpdateFailure {
                message: "the hotkey registry must be created on the main thread".into(),
                actual: Box::default(),
                compensation_errors: Vec::new(),
            });
        }
        let manager = GlobalHotKeyManager::new().map_err(|error| HotkeyUpdateFailure {
            message: format!("failed to create hotkey manager: {error}"),
            actual: Box::default(),
            compensation_errors: Vec::new(),
        })?;
        Ok(Self {
            state: RegistryState::new(NativeBackend {
                manager,
                #[cfg(target_os = "macos")]
                failed_removals: BTreeMap::new(),
            }),
            router: EventRouter::default(),
            _event_thread: PhantomData,
        })
    }

    pub fn apply(&mut self, bindings: &ShortcutBindings) -> Result<EffectiveBindings, HotkeyUpdateFailure> {
        let result = self.state.apply(bindings);
        self.publish_routes();
        result
    }

    pub fn current(&self) -> EffectiveBindings {
        self.state.current()
    }

    pub fn router(&self) -> EventRouter {
        self.router.clone()
    }

    pub fn unregister_all(&mut self) -> Result<EffectiveBindings, HotkeyUpdateFailure> {
        let result = self.state.clear();
        self.publish_routes();
        result
    }

    fn publish_routes(&self) {
        let routes = self.state.active.iter().map(|(action, binding)| (binding.hotkey.id(), *action)).collect();
        *self.router.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = routes;
    }
}

impl Drop for NativeHotkeyRegistry {
    fn drop(&mut self) {
        if let Err(error) = self.state.clear() {
            tracing::warn!("failed to release native hotkeys: {error}; actual state: {:?}", error.actual);
        }
        self.router.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[derive(Default)]
    struct FakeBackend {
        keys: BTreeSet<u32>,
        fail_register: BTreeSet<u32>,
        fail_unregister: BTreeSet<u32>,
        uncertain_removals: BTreeSet<u32>,
        uncertain_keys: BTreeSet<u32>,
        calls: usize,
    }

    impl RegistrationBackend for FakeBackend {
        fn register(&mut self, key: HotKey) -> Result<(), String> {
            self.calls += 1;
            if self.fail_register.contains(&key.id()) {
                return Err("registration rejected".into());
            }
            assert!(self.keys.insert(key.id()), "registering an already owned key");
            Ok(())
        }

        fn unregister(&mut self, key: HotKey) -> Result<(), String> {
            self.calls += 1;
            if self.fail_unregister.contains(&key.id()) {
                if self.uncertain_removals.contains(&key.id()) {
                    self.uncertain_keys.insert(key.id());
                }
                return Err("unregistration rejected".into());
            }
            assert!(self.keys.remove(&key.id()), "unregistering an unknown key");
            Ok(())
        }

        fn uncertain_registration(&self, key: HotKey) -> bool {
            self.uncertain_keys.contains(&key.id())
        }
    }

    fn id(shortcut: &str) -> u32 {
        HotKey::from_str(shortcut).unwrap().id()
    }

    fn initial_registry() -> RegistryState<FakeBackend> {
        let mut state = RegistryState::new(FakeBackend::default());
        state.apply(&ShortcutBindings::default()).unwrap();
        state
    }

    #[test]
    fn failed_new_registration_preserves_old_shortcuts() {
        let mut state = initial_registry();
        state.backend.fail_register.insert(id("F4"));
        let error = state
            .apply(&ShortcutBindings {
                capture: "F3".into(),
                quick_capture: "F4".into(),
            })
            .unwrap_err();
        assert_eq!(error.actual.capture.as_deref(), Some("F1"));
        assert_eq!(error.actual.quick_capture.as_deref(), Some("F2"));
        assert!(error.actual.unrouted.is_empty());
        assert_eq!(state.backend.keys, BTreeSet::from([id("F1"), id("F2")]));
    }

    #[test]
    fn swapping_actions_changes_routes_without_releasing_keys() {
        let mut state = initial_registry();
        let previous_calls = state.backend.calls;
        let actual = state
            .apply(&ShortcutBindings {
                capture: "F2".into(),
                quick_capture: "F1".into(),
            })
            .unwrap();
        assert_eq!(actual.capture.as_deref(), Some("F2"));
        assert_eq!(actual.quick_capture.as_deref(), Some("F1"));
        assert_eq!(state.backend.calls, previous_calls);
    }

    #[test]
    fn failed_compensation_reports_every_remaining_native_registration() {
        let mut state = initial_registry();
        state.backend.fail_register.insert(id("F4"));
        state.backend.fail_unregister.insert(id("F3"));
        let error = state
            .apply(&ShortcutBindings {
                capture: "F3".into(),
                quick_capture: "F4".into(),
            })
            .unwrap_err();
        assert_eq!(error.actual.capture.as_deref(), Some("F1"));
        assert_eq!(error.actual.unrouted, ["F3"]);
        assert_eq!(error.compensation_errors.len(), 1);
        assert_eq!(state.backend.keys, BTreeSet::from([id("F1"), id("F2"), id("F3")]));
    }

    #[test]
    fn loss_of_an_old_key_during_compensation_is_explicit() {
        let mut state = initial_registry();
        state.backend.fail_unregister.insert(id("F2"));
        state.backend.fail_register.insert(id("F1"));
        let error = state
            .apply(&ShortcutBindings {
                capture: "F3".into(),
                quick_capture: "F4".into(),
            })
            .unwrap_err();
        assert_eq!(error.actual.capture, None);
        assert_eq!(error.actual.quick_capture.as_deref(), Some("F2"));
        assert_eq!(state.backend.keys, BTreeSet::from([id("F2")]));
        assert_eq!(error.compensation_errors.len(), 1);
    }

    #[test]
    fn uncertain_native_ownership_cannot_be_reported_as_success() {
        let mut state = initial_registry();
        state.backend.fail_unregister.insert(id("F2"));
        state.backend.uncertain_removals.insert(id("F2"));
        let error = state
            .apply(&ShortcutBindings {
                capture: "F3".into(),
                quick_capture: "F4".into(),
            })
            .unwrap_err();
        assert_eq!(error.actual.capture.as_deref(), Some("F1"));
        assert_eq!(error.actual.quick_capture, None);
        assert_eq!(error.actual.uncertain, ["F2"]);
        let previous_calls = state.backend.calls;
        let retry = state.apply(&ShortcutBindings::default()).unwrap_err();
        assert_eq!(retry.actual, error.actual);
        assert_eq!(state.backend.calls, previous_calls);
    }

    #[test]
    fn invalid_and_conflicting_bindings_do_not_change_native_state() {
        let mut state = initial_registry();
        let previous_calls = state.backend.calls;
        for bindings in [
            ShortcutBindings {
                capture: "not a valid shortcut".into(),
                quick_capture: "F4".into(),
            },
            ShortcutBindings {
                capture: "F3".into(),
                quick_capture: "f3".into(),
            },
        ] {
            assert!(state.apply(&bindings).is_err());
        }
        assert_eq!(state.backend.calls, previous_calls);
        assert_eq!(state.current().capture.as_deref(), Some("F1"));
    }
}

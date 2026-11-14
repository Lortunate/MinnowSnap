use super::super::{PreferencesNotice, PreferencesState, general, ocr, shortcuts};
use super::rows::{ActionRowProps, ButtonProps, SelectRowProps, ToggleRowProps};
use crate::presentation::app_meta::APP_NAME;
use crate::presentation::hotkeys::{HotkeyAction, shortcuts_conflict};
use crate::presentation::i18n;
use gpui::SharedString;
use minnow_application::{
    ports::{ModelStatus, RecognitionAvailability},
    preferences::NativePreferencesSnapshot,
    settings,
};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneralPageProps {
    pub(crate) refresh_native: ActionRowProps,
    pub(crate) auto_start: ToggleRowProps,
    pub(crate) language: SelectRowProps,
    pub(crate) theme: SelectRowProps,
    pub(crate) font: SelectRowProps,
    pub(crate) save_path: ActionRowProps,
    pub(crate) image_compression: ToggleRowProps,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NotificationsPageProps {
    pub(crate) enabled: ToggleRowProps,
    pub(crate) save_notification: ToggleRowProps,
    pub(crate) copy_notification: ToggleRowProps,
    pub(crate) qr_code_notification: ToggleRowProps,
    pub(crate) shutter_sound: ToggleRowProps,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShortcutsPageProps {
    pub(crate) capture: ActionRowProps,
    pub(crate) quick_capture: ActionRowProps,
    pub(crate) recording_notice: Option<PreferencesNotice>,
    pub(crate) conflict_notice: Option<PreferencesNotice>,
    pub(crate) activation_notice: Option<PreferencesNotice>,
    pub(crate) retry: Option<ButtonProps>,
    pub(crate) restore_defaults: ButtonProps,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OcrPageProps {
    pub(crate) enabled: ToggleRowProps,
    pub(crate) model: Option<ActionRowProps>,
    pub(crate) runtime_notice: PreferencesNotice,
    pub(crate) note: Option<SharedString>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AboutPageProps {
    pub(crate) app_name: SharedString,
    pub(crate) version_label: SharedString,
    pub(crate) summary: SharedString,
    pub(crate) log_directory_path: PathBuf,
    pub(crate) github_repository: ActionRowProps,
    pub(crate) report_issue: ActionRowProps,
    pub(crate) open_logs: ActionRowProps,
}

pub(super) fn build_general_props(
    settings: &settings::AppSettings,
    fonts: &[SharedString],
    default_path: Result<PathBuf, String>,
    auto_start: Result<bool, String>,
) -> GeneralPageProps {
    let auto_start_enabled = auto_start.as_ref().copied().unwrap_or(false);
    let auto_start_error = auto_start.err();
    GeneralPageProps {
        refresh_native: ActionRowProps::new(
            "preferences-refresh-native",
            i18n::preferences::startup_shortcuts_status(),
            i18n::preferences::startup_shortcuts_status_description(),
            i18n::preferences::refresh_status(),
        ),
        auto_start: ToggleRowProps::new(
            "preferences-auto-start",
            i18n::preferences::auto_start(),
            auto_start_error.clone().unwrap_or_else(i18n::preferences::auto_start_description),
            auto_start_enabled,
        )
        .disabled(auto_start_error.is_some()),
        language: SelectRowProps::new(
            "preferences-language",
            i18n::preferences::language(),
            i18n::preferences::language_description(),
            settings.general.language.clone(),
            general::language_options(),
        ),
        theme: SelectRowProps::new(
            "preferences-theme",
            i18n::preferences::theme(),
            i18n::preferences::theme_description(),
            settings.general.theme.clone(),
            general::theme_options(),
        ),
        font: SelectRowProps::new(
            "preferences-font-family",
            i18n::preferences::font_family(),
            i18n::preferences::font_family_description(),
            settings.general.font_family.clone().unwrap_or_default(),
            general::font_options(fonts),
        ),
        save_path: ActionRowProps::new(
            "preferences-save-path",
            i18n::preferences::save_directory(),
            general::save_directory_description(settings, &default_path),
            i18n::preferences::browse(),
        ),
        image_compression: ToggleRowProps::new(
            "preferences-image-compression",
            i18n::preferences::image_compression(),
            i18n::preferences::image_compression_description(),
            settings.output.oxipng_enabled,
        ),
    }
}

pub(super) fn build_notifications_props(snapshot: &settings::NotificationSettings) -> NotificationsPageProps {
    NotificationsPageProps {
        enabled: ToggleRowProps::new(
            "preferences-notifications-enabled",
            i18n::preferences::notifications_enabled(),
            i18n::preferences::notifications_enabled_description(),
            snapshot.enabled,
        ),
        save_notification: ToggleRowProps::new(
            "preferences-save-notification",
            i18n::preferences::save_notification(),
            i18n::preferences::save_notification_description(),
            snapshot.save_notification,
        )
        .disabled(!snapshot.enabled),
        copy_notification: ToggleRowProps::new(
            "preferences-copy-notification",
            i18n::preferences::copy_notification(),
            i18n::preferences::copy_notification_description(),
            snapshot.copy_notification,
        )
        .disabled(!snapshot.enabled),
        qr_code_notification: ToggleRowProps::new(
            "preferences-qr-notification",
            i18n::preferences::qr_code_notification(),
            i18n::preferences::qr_code_notification_description(),
            snapshot.qr_code_notification,
        )
        .disabled(!snapshot.enabled),
        shutter_sound: ToggleRowProps::new(
            "preferences-shutter-sound",
            i18n::preferences::shutter_sound(),
            i18n::preferences::shutter_sound_description(),
            snapshot.shutter_sound,
        ),
    }
}

pub(super) fn build_shortcuts_props(
    state: &PreferencesState,
    desired: &settings::ShortcutSettings,
    native: &NativePreferencesSnapshot,
) -> ShortcutsPageProps {
    let recording_capture = state.shortcut_recording == Some(HotkeyAction::Capture);
    let recording_quick = state.shortcut_recording == Some(HotkeyAction::QuickCapture);
    let retry_needed = shortcuts::retry_needed(desired, native);

    ShortcutsPageProps {
        capture: ActionRowProps::new(
            "preferences-shortcuts-capture",
            i18n::preferences::capture_shortcut(),
            format!(
                "{} {}",
                i18n::preferences::capture_shortcut_description(),
                shortcuts::effective_label(native.shortcuts.capture.as_deref().unwrap_or_default())
            ),
            if recording_capture {
                SharedString::from(i18n::preferences::shortcuts_recording())
            } else {
                desired.capture.clone().into()
            },
        ),
        quick_capture: ActionRowProps::new(
            "preferences-shortcuts-quick",
            i18n::preferences::quick_capture_shortcut(),
            format!(
                "{} {}",
                i18n::preferences::quick_capture_shortcut_description(),
                shortcuts::effective_label(native.shortcuts.quick_capture.as_deref().unwrap_or_default())
            ),
            if recording_quick {
                SharedString::from(i18n::preferences::shortcuts_recording())
            } else {
                desired.quick_capture.clone().into()
            },
        ),
        recording_notice: (recording_capture || recording_quick).then(|| PreferencesNotice::info(i18n::preferences::shortcuts_recording_hint())),
        conflict_notice: shortcuts_conflict(&desired.capture, &desired.quick_capture)
            .then(|| PreferencesNotice::error(i18n::preferences::shortcuts_conflict())),
        activation_notice: retry_needed.then(|| PreferencesNotice::info(i18n::preferences::shortcuts_pending_activation())),
        retry: retry_needed.then(|| ButtonProps::new("preferences-shortcuts-retry", i18n::preferences::shortcuts_retry())),
        restore_defaults: ButtonProps::new("preferences-shortcuts-restore-defaults", i18n::preferences::shortcuts_restore_defaults()),
    }
}

pub(super) fn build_ocr_props(enabled: bool, status: Option<ModelStatus>, availability: RecognitionAvailability) -> OcrPageProps {
    OcrPageProps {
        enabled: ToggleRowProps::new(
            "preferences-ocr-enabled",
            i18n::preferences::ocr(),
            i18n::preferences::ocr_enabled_description(),
            enabled,
        ),
        model: status.as_ref().map(|status| {
            ActionRowProps::new(
                "preferences-ocr-download",
                i18n::preferences::ocr_model(),
                ocr::ocr_status_label(status),
                ocr_download_label(status),
            )
            .disabled(!availability.is_available() || status.is_downloading())
        }),
        runtime_notice: match availability {
            RecognitionAvailability::Available => PreferencesNotice::info(i18n::preferences::ocr_runtime_available()),
            RecognitionAvailability::Unavailable { reason } => PreferencesNotice::error(i18n::preferences::ocr_runtime_unavailable(reason)),
        },
        note: status.is_some().then(|| i18n::preferences::ocr_note().into()),
    }
}

pub(super) fn build_about_props(log_directory_path: PathBuf) -> AboutPageProps {
    AboutPageProps {
        app_name: APP_NAME.into(),
        version_label: format!("{} {}", i18n::preferences::version_label(), env!("CARGO_PKG_VERSION")).into(),
        summary: i18n::preferences::about_summary().into(),
        log_directory_path: log_directory_path.clone(),
        github_repository: ActionRowProps::new(
            "preferences-about-github",
            i18n::preferences::github_repository(),
            i18n::preferences::github_repository_description(),
            i18n::preferences::open(),
        ),
        report_issue: ActionRowProps::new(
            "preferences-about-issues",
            i18n::preferences::report_issue(),
            i18n::preferences::report_issue_description(),
            i18n::preferences::open(),
        ),
        open_logs: ActionRowProps::new(
            "preferences-about-logs",
            i18n::preferences::open_log_folder(),
            log_directory_path.to_string_lossy().into_owned(),
            i18n::preferences::open(),
        ),
    }
}

fn ocr_download_label(status: &ModelStatus) -> SharedString {
    if status.is_downloading() {
        i18n::preferences::ocr_download_in_progress().into()
    } else if status.is_ready() {
        i18n::preferences::ocr_redownload_action().into()
    } else {
        i18n::preferences::ocr_download_action().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minnow_application::{ports::ModelDownloadStatus, preferences::EffectiveShortcuts};

    #[test]
    fn general_projection_keeps_font_choices_and_native_autostart_failure() {
        let mut settings = settings::AppSettings::default();
        settings.general.font_family = Some("Example Font".into());
        settings.general.auto_start = true;
        let fonts = [SharedString::from("Example Font")];
        let props = build_general_props(&settings, &fonts, Ok("/captures".into()), Ok(false));
        assert_eq!(props.font.current_value.as_ref(), "Example Font");
        assert_eq!(props.font.options[0].value.as_ref(), "");
        assert_eq!(props.font.options[1].value.as_ref(), "Example Font");
        assert!(!props.auto_start.checked, "show native state rather than desired autostart");

        let unavailable = build_general_props(&settings, &fonts, Ok("/captures".into()), Err("native query failed".into()));
        assert!(!unavailable.auto_start.checked);
        assert!(unavailable.auto_start.disabled);
        assert_eq!(unavailable.auto_start.description.as_ref(), "native query failed");
    }

    #[test]
    fn notifications_props_disable_children_when_master_toggle_is_off() {
        let props = build_notifications_props(&settings::NotificationSettings {
            enabled: false,
            ..settings::NotificationSettings::default()
        });
        assert!(props.save_notification.disabled);
        assert!(props.copy_notification.disabled);
        assert!(props.qr_code_notification.disabled);
        assert!(!props.shutter_sound.disabled);
    }

    #[test]
    fn shortcuts_projection_keeps_desired_keys_after_native_failure() {
        let mut desired = settings::ShortcutSettings {
            capture: "F3".into(),
            quick_capture: "F4".into(),
        };
        let native = NativePreferencesSnapshot {
            auto_start: Ok(false),
            auto_start_failure: None,
            shortcuts: EffectiveShortcuts {
                capture: Some("F1".into()),
                ..Default::default()
            },
            shortcut_failure: None,
        };
        let mut state = PreferencesState::new();
        let props = build_shortcuts_props(&state, &desired, &native);
        assert_eq!(props.capture.button_label.as_ref(), "F3");
        assert_eq!(props.quick_capture.button_label.as_ref(), "F4");
        assert!(props.capture.description.contains("F1"));
        assert!(props.quick_capture.description.contains(&i18n::preferences::shortcut_not_registered()));
        assert!(props.activation_notice.is_some());
        assert!(props.retry.is_some());
        assert!(shortcuts::conflicts_with_desired(&desired, HotkeyAction::QuickCapture, "F3"));
        assert!(!shortcuts::conflicts_with_desired(&desired, HotkeyAction::QuickCapture, "F1"));

        state.start_shortcut_recording(HotkeyAction::Capture);
        desired.quick_capture = desired.capture.clone();
        let recording = build_shortcuts_props(&state, &desired, &native);
        assert_eq!(recording.capture.button_label.as_ref(), i18n::preferences::shortcuts_recording());
        assert!(recording.recording_notice.is_some());
        assert!(recording.conflict_notice.as_ref().is_some_and(PreferencesNotice::is_error));
    }

    #[test]
    fn ocr_projection_keeps_runtime_model_and_desired_states_independent() {
        let downloading = build_ocr_props(
            true,
            Some(ModelStatus {
                active_generation: Some("ready".into()),
                download: ModelDownloadStatus::Downloading { progress_percent: 42 },
            }),
            RecognitionAvailability::Available,
        );
        assert!(downloading.model.as_ref().unwrap().disabled);
        assert!(
            downloading
                .model
                .as_ref()
                .unwrap()
                .description
                .contains(&i18n::preferences::ocr_status_ready())
        );

        let unsupported = build_ocr_props(
            true,
            Some(ModelStatus::default()),
            RecognitionAvailability::Unavailable {
                reason: "runtime not bundled".into(),
            },
        );
        assert!(unsupported.enabled.checked, "runtime capability must not erase user intent");
        assert!(!unsupported.enabled.disabled);
        assert!(unsupported.model.as_ref().unwrap().disabled);
        assert!(unsupported.runtime_notice.is_error());
        assert!(unsupported.runtime_notice.message.contains("runtime not bundled"));

        let system = build_ocr_props(false, None, RecognitionAvailability::Available);
        assert!(!system.enabled.checked);
        assert!(
            system.model.is_none(),
            "a backend without model management must not show download controls"
        );
        assert!(system.note.is_none());
        assert!(!system.runtime_notice.is_error());
    }
}

use super::{
    general::{self, GeneralSnapshot},
    notifications::{self, NotificationsSnapshot},
    ocr, shortcuts,
    state::{PreferencesNotice, PreferencesPage, PreferencesState},
};
use crate::core::{app::APP_NAME, hotkey::HotkeyAction, i18n, logging, ocr_service::OcrModelStatus};
use gpui::{App, SharedString};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SelectOption {
    pub(crate) value: SharedString,
    pub(crate) label: SharedString,
}

impl SelectOption {
    pub(crate) fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }

    pub(crate) fn label_for(current: &str, options: &[Self]) -> SharedString {
        options
            .iter()
            .find(|option| option.value.as_ref() == current)
            .map(|option| option.label.clone())
            .unwrap_or_else(|| SharedString::from(current.to_owned()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SidebarItemProps {
    pub(crate) page: PreferencesPage,
    pub(crate) title: SharedString,
    pub(crate) is_active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ToggleRowProps {
    pub(crate) id: &'static str,
    pub(crate) title: SharedString,
    pub(crate) description: SharedString,
    pub(crate) checked: bool,
    pub(crate) disabled: bool,
}

impl ToggleRowProps {
    fn new(id: &'static str, title: impl Into<SharedString>, description: impl Into<SharedString>, checked: bool) -> Self {
        Self {
            id,
            title: title.into(),
            description: description.into(),
            checked,
            disabled: false,
        }
    }

    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SelectRowProps {
    pub(crate) id: &'static str,
    pub(crate) title: SharedString,
    pub(crate) description: SharedString,
    pub(crate) current_value: SharedString,
    pub(crate) disabled: bool,
    pub(crate) options: Vec<SelectOption>,
}

impl SelectRowProps {
    fn new(
        id: &'static str,
        title: impl Into<SharedString>,
        description: impl Into<SharedString>,
        current_value: impl Into<SharedString>,
        options: Vec<SelectOption>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            description: description.into(),
            current_value: current_value.into(),
            disabled: false,
            options,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActionRowProps {
    pub(crate) id: &'static str,
    pub(crate) title: SharedString,
    pub(crate) description: SharedString,
    pub(crate) button_label: SharedString,
    pub(crate) disabled: bool,
}

impl ActionRowProps {
    fn new(id: &'static str, title: impl Into<SharedString>, description: impl Into<SharedString>, button_label: impl Into<SharedString>) -> Self {
        Self {
            id,
            title: title.into(),
            description: description.into(),
            button_label: button_label.into(),
            disabled: false,
        }
    }

    pub(crate) fn button(&self) -> ButtonProps {
        ButtonProps::new(self.id, self.button_label.clone()).disabled(self.disabled)
    }

    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ButtonProps {
    pub(crate) id: &'static str,
    pub(crate) label: SharedString,
    pub(crate) disabled: bool,
}

impl ButtonProps {
    pub(crate) fn new(id: &'static str, label: impl Into<SharedString>) -> Self {
        Self {
            id,
            label: label.into(),
            disabled: false,
        }
    }

    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneralPageProps {
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
    pub(crate) restore_defaults: ButtonProps,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OcrPageProps {
    pub(crate) enabled: ToggleRowProps,
    pub(crate) model: ActionRowProps,
    pub(crate) show_model: bool,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreferencesFrame {
    pub(crate) active_page: PreferencesPage,
    pub(crate) page_title: SharedString,
    pub(crate) notice: Option<PreferencesNotice>,
    pub(crate) sidebar_items: Vec<SidebarItemProps>,
    pub(crate) general: GeneralPageProps,
    pub(crate) notifications: NotificationsPageProps,
    pub(crate) shortcuts: ShortcutsPageProps,
    pub(crate) ocr: OcrPageProps,
    pub(crate) about: AboutPageProps,
}

pub(crate) fn build(state: &PreferencesState, cx: &App) -> PreferencesFrame {
    PreferencesFrame {
        active_page: state.active_page,
        page_title: state.active_page.title(),
        notice: state.notice.clone(),
        sidebar_items: build_sidebar_items(state.active_page),
        general: build_general_props(general::snapshot()),
        notifications: build_notifications_props(notifications::snapshot()),
        shortcuts: build_shortcuts_props(state, shortcuts::snapshot(cx)),
        ocr: build_ocr_props(ocr::snapshot(state)),
        about: build_about_props(),
    }
}

fn build_sidebar_items(active_page: PreferencesPage) -> Vec<SidebarItemProps> {
    PreferencesPage::ALL
        .into_iter()
        .map(|page| SidebarItemProps {
            page,
            title: page.title(),
            is_active: page == active_page,
        })
        .collect()
}

fn build_general_props(snapshot: GeneralSnapshot) -> GeneralPageProps {
    GeneralPageProps {
        auto_start: ToggleRowProps::new(
            "preferences-auto-start",
            i18n::preferences::auto_start(),
            i18n::preferences::auto_start_description(),
            snapshot.auto_start,
        ),
        language: SelectRowProps::new(
            "preferences-language",
            i18n::preferences::language(),
            i18n::preferences::language_description(),
            snapshot.language,
            snapshot.language_options,
        ),
        theme: SelectRowProps::new(
            "preferences-theme",
            i18n::preferences::theme(),
            i18n::preferences::theme_description(),
            snapshot.theme,
            snapshot.theme_options,
        ),
        font: SelectRowProps::new(
            "preferences-font-family",
            i18n::preferences::font_family(),
            i18n::preferences::font_family_description(),
            snapshot.font_family,
            snapshot.font_options,
        ),
        save_path: ActionRowProps::new(
            "preferences-save-path",
            i18n::preferences::save_directory(),
            snapshot.save_directory_description,
            i18n::preferences::browse(),
        ),
        image_compression: ToggleRowProps::new(
            "preferences-image-compression",
            i18n::preferences::image_compression(),
            i18n::preferences::image_compression_description(),
            snapshot.oxipng_enabled,
        ),
    }
}

fn build_notifications_props(snapshot: NotificationsSnapshot) -> NotificationsPageProps {
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

fn build_shortcuts_props(state: &PreferencesState, snapshot: shortcuts::ShortcutsSnapshot) -> ShortcutsPageProps {
    let recording_capture = state.shortcut_recording == Some(HotkeyAction::Capture);
    let recording_quick = state.shortcut_recording == Some(HotkeyAction::QuickCapture);

    ShortcutsPageProps {
        capture: ActionRowProps::new(
            "preferences-shortcuts-capture",
            i18n::preferences::capture_shortcut(),
            i18n::preferences::capture_shortcut_description(),
            if recording_capture {
                SharedString::from(i18n::preferences::shortcuts_recording())
            } else {
                snapshot.bindings.capture.clone().into()
            },
        ),
        quick_capture: ActionRowProps::new(
            "preferences-shortcuts-quick",
            i18n::preferences::quick_capture_shortcut(),
            i18n::preferences::quick_capture_shortcut_description(),
            if recording_quick {
                SharedString::from(i18n::preferences::shortcuts_recording())
            } else {
                snapshot.bindings.quick_capture.clone().into()
            },
        ),
        recording_notice: (recording_capture || recording_quick).then(|| PreferencesNotice::info(i18n::preferences::shortcuts_recording_hint())),
        conflict_notice: snapshot
            .conflict_message
            .as_ref()
            .map(|message| PreferencesNotice::error(message.clone())),
        restore_defaults: ButtonProps::new("preferences-shortcuts-restore-defaults", i18n::preferences::shortcuts_restore_defaults()),
    }
}

fn build_ocr_props(snapshot: ocr::OcrSnapshot) -> OcrPageProps {
    OcrPageProps {
        enabled: ToggleRowProps::new(
            "preferences-ocr-enabled",
            i18n::preferences::ocr(),
            i18n::preferences::ocr_enabled_description(),
            snapshot.enabled,
        ),
        model: ActionRowProps::new(
            "preferences-ocr-download",
            i18n::preferences::ocr_model(),
            ocr::ocr_status_label(&snapshot.status),
            ocr_download_label(&snapshot.status),
        )
        .disabled(snapshot.status.is_downloading()),
        show_model: snapshot.enabled,
        note: snapshot.enabled.then(|| i18n::preferences::ocr_note().into()),
    }
}

fn build_about_props() -> AboutPageProps {
    let log_directory_path = logging::log_dir(APP_NAME);

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

fn ocr_download_label(status: &OcrModelStatus) -> SharedString {
    if status.is_downloading() {
        i18n::preferences::ocr_download_in_progress().into()
    } else if matches!(status, OcrModelStatus::Ready) {
        i18n::preferences::ocr_redownload_action().into()
    } else {
        i18n::preferences::ocr_download_action().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::hotkey::ShortcutBindings;

    #[test]
    fn select_option_label_uses_matching_label() {
        let options = vec![SelectOption::new("system", "Follow System"), SelectOption::new("dark", "Dark")];

        assert_eq!(SelectOption::label_for("dark", &options), SharedString::from("Dark"));
        assert_eq!(SelectOption::label_for("missing", &options), SharedString::from("missing"));
    }

    #[test]
    fn general_props_keep_dynamic_font_and_language_options() {
        let props = build_general_props(GeneralSnapshot {
            language: "System".into(),
            theme: "System".into(),
            font_family: "".into(),
            auto_start: false,
            save_directory_description: "Default".into(),
            oxipng_enabled: true,
            language_options: vec![SelectOption::new("System", i18n::preferences::follow_system())],
            theme_options: vec![SelectOption::new("System", i18n::preferences::follow_system())],
            font_options: vec![SelectOption::new("", i18n::preferences::follow_system())],
        });

        assert_eq!(props.language.options[0].label, SharedString::from(i18n::preferences::follow_system()));
        assert_eq!(props.font.options[0].label, SharedString::from(i18n::preferences::follow_system()));
    }

    #[test]
    fn notifications_props_disable_children_when_master_toggle_is_off() {
        let props = build_notifications_props(NotificationsSnapshot {
            enabled: false,
            save_notification: true,
            copy_notification: true,
            qr_code_notification: true,
            shutter_sound: true,
        });

        assert!(props.save_notification.disabled);
        assert!(props.copy_notification.disabled);
        assert!(props.qr_code_notification.disabled);
        assert!(!props.shutter_sound.disabled);
    }

    #[test]
    fn shortcuts_props_show_recording_and_conflict_state() {
        let mut state = PreferencesState::new();
        state.start_shortcut_recording(HotkeyAction::Capture);

        let props = build_shortcuts_props(
            &state,
            shortcuts::ShortcutsSnapshot {
                bindings: ShortcutBindings {
                    capture: "Ctrl+Shift+1".to_string(),
                    quick_capture: "Ctrl+Shift+1".to_string(),
                },
                conflict_message: Some(i18n::preferences::shortcuts_conflict().into()),
            },
        );

        assert_eq!(props.capture.button_label, SharedString::from(i18n::preferences::shortcuts_recording()));
        assert!(props.recording_notice.is_some());
        assert!(props.conflict_notice.as_ref().is_some_and(PreferencesNotice::is_error));
    }

    #[test]
    fn ocr_props_map_download_states() {
        let missing = build_ocr_props(ocr::OcrSnapshot {
            enabled: true,
            status: OcrModelStatus::Missing,
        });
        assert_eq!(missing.model.button_label, SharedString::from(i18n::preferences::ocr_download_action()));
        assert!(!missing.model.disabled);

        let downloading = build_ocr_props(ocr::OcrSnapshot {
            enabled: true,
            status: OcrModelStatus::Downloading { progress_percent: 42 },
        });
        assert_eq!(
            downloading.model.button_label,
            SharedString::from(i18n::preferences::ocr_download_in_progress())
        );
        assert!(downloading.model.disabled);

        let ready = build_ocr_props(ocr::OcrSnapshot {
            enabled: true,
            status: OcrModelStatus::Ready,
        });
        assert_eq!(ready.model.button_label, SharedString::from(i18n::preferences::ocr_redownload_action()));

        let failed = build_ocr_props(ocr::OcrSnapshot {
            enabled: true,
            status: OcrModelStatus::Failed {
                message: "network error".to_string(),
            },
        });
        assert!(failed.model.description.as_ref().contains("network error"));
    }
}

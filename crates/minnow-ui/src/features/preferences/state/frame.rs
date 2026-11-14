mod pages;
mod rows;

pub(crate) use pages::{AboutPageProps, GeneralPageProps, NotificationsPageProps, OcrPageProps, ShortcutsPageProps};
pub(crate) use rows::{ActionRowProps, ButtonProps, SelectOption, SelectRowProps, SidebarItemProps, ToggleRowProps};

use super::{PreferencesNotice, PreferencesPage, PreferencesState};
use crate::presentation::i18n;
use gpui::{App, SharedString};
use minnow_application::preferences::NativePreferencesSnapshot;
use minnow_application::settings::{PersistenceStatus, SettingsLoadStatus};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreferencesFrame {
    pub(crate) page_title: SharedString,
    pub(crate) notices: Vec<PreferencesNotice>,
    pub(crate) sidebar_items: Vec<SidebarItemProps>,
    pub(crate) page: ActivePageProps,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ActivePageProps {
    General(Box<GeneralPageProps>),
    Notifications(NotificationsPageProps),
    Shortcuts(ShortcutsPageProps),
    Ocr(OcrPageProps),
    About(AboutPageProps),
}

pub(crate) fn build(state: &PreferencesState, native: &NativePreferencesSnapshot, fonts: &[SharedString], cx: &App) -> PreferencesFrame {
    let application = crate::application(cx);
    let preferences = application.preferences();
    let page = match state.active_page {
        PreferencesPage::General => ActivePageProps::General(Box::new(pages::build_general_props(
            &preferences.snapshot(),
            fonts,
            application.default_output_directory().map_err(|error| error.to_string()),
            native.auto_start.clone().map_err(|error| error.to_string()),
        ))),
        PreferencesPage::Notifications => ActivePageProps::Notifications(pages::build_notifications_props(&preferences.snapshot().notification)),
        PreferencesPage::Shortcuts => ActivePageProps::Shortcuts(pages::build_shortcuts_props(state, &preferences.snapshot().shortcuts, native)),
        PreferencesPage::Ocr => ActivePageProps::Ocr(pages::build_ocr_props(
            preferences.snapshot().ocr.enabled,
            application.model_status(),
            application.recognition_availability(),
        )),
        PreferencesPage::About => ActivePageProps::About(pages::build_about_props(crate::host(cx).paths().logs_dir.clone())),
    };
    PreferencesFrame {
        page_title: state.active_page.title(),
        notices: notices(state, native, preferences.load_status(), preferences.persistence_status()),
        sidebar_items: build_sidebar_items(state.active_page),
        page,
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

fn notices(
    state: &PreferencesState,
    native: &NativePreferencesSnapshot,
    loaded: SettingsLoadStatus,
    persistence: PersistenceStatus,
) -> Vec<PreferencesNotice> {
    let mut notices = Vec::new();
    match loaded {
        SettingsLoadStatus::Degraded { error } => notices.push(PreferencesNotice::error(i18n::preferences::settings_load_failed(error.to_string()))),
        SettingsLoadStatus::InMemory => notices.push(PreferencesNotice::info(i18n::preferences::settings_temporary())),
        _ => match persistence {
            PersistenceStatus::Failed { error, .. } => {
                notices.push(PreferencesNotice::error(i18n::preferences::settings_save_failed(error.to_string())))
            }
            PersistenceStatus::Pending { .. } => notices.push(PreferencesNotice::info(i18n::preferences::settings_saving())),
            _ => {}
        },
    }
    if let Some(error) = &native.auto_start_failure {
        notices.push(PreferencesNotice::error(i18n::preferences::auto_start_update_failed(error.to_string())));
    }
    if let Some(error) = &native.shortcut_failure {
        notices.push(PreferencesNotice::error(i18n::preferences::shortcuts_update_failed(&error.message)));
        if !error.compensation_errors.is_empty() {
            notices.push(PreferencesNotice::error(i18n::preferences::shortcut_cleanup_failed(
                error.compensation_errors.join("; "),
            )));
        }
    }
    if !native.shortcuts.unrouted.is_empty() {
        notices.push(PreferencesNotice::error(i18n::preferences::shortcuts_unavailable(
            native.shortcuts.unrouted.join(", "),
        )));
    }
    if !native.shortcuts.uncertain.is_empty() {
        notices.push(PreferencesNotice::error(i18n::preferences::shortcuts_restart_required(
            native.shortcuts.uncertain.join(", "),
        )));
    }
    notices.extend(state.notice.clone());
    notices
}

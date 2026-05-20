mod pages;
mod rows;

pub(crate) use pages::{AboutPageProps, GeneralPageProps, NotificationsPageProps, OcrPageProps, ShortcutsPageProps};
pub(crate) use rows::{ActionRowProps, ButtonProps, SelectOption, SelectRowProps, SidebarItemProps, ToggleRowProps};

use super::{PreferencesNotice, PreferencesPage, PreferencesState, general, ocr, shortcuts};
use crate::services::settings;
use gpui::{App, SharedString};

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
        general: pages::build_general_props(general::snapshot()),
        notifications: pages::build_notifications_props(settings::notification_settings()),
        shortcuts: pages::build_shortcuts_props(state, shortcuts::snapshot(cx)),
        ocr: pages::build_ocr_props(ocr::snapshot(state)),
        about: pages::build_about_props(),
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

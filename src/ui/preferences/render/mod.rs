mod about;
pub(super) mod chrome;
pub(super) mod components;
mod general;
mod notifications;
mod ocr;
mod shortcuts;

use super::{session::frame::PreferencesFrame, view::PreferencesView};
use gpui::{AnyElement, App, ClickEvent, Context, SharedString, Window};

pub(super) type ToggleAction = fn(&mut PreferencesView, bool, &mut Window, &mut Context<PreferencesView>);
pub(super) type ClickAction = fn(&mut PreferencesView, &ClickEvent, &mut Window, &mut Context<PreferencesView>);
pub(super) type SelectAction = fn(SharedString, &mut Window, &mut App);
pub(super) type PlainClickAction = fn(&ClickEvent, &mut Window, &mut App);

#[derive(Clone, Copy)]
pub(super) struct GeneralPageActions {
    pub(super) auto_start: ToggleAction,
    pub(super) language: SelectAction,
    pub(super) theme: SelectAction,
    pub(super) font: SelectAction,
    pub(super) browse_save_path: ClickAction,
    pub(super) image_compression: ToggleAction,
}

#[derive(Clone, Copy)]
pub(super) struct NotificationsPageActions {
    pub(super) enabled: ToggleAction,
    pub(super) save_notification: ToggleAction,
    pub(super) copy_notification: ToggleAction,
    pub(super) qr_code_notification: ToggleAction,
    pub(super) shutter_sound: ToggleAction,
}

#[derive(Clone, Copy)]
pub(super) struct ShortcutsPageActions {
    pub(super) record_capture: ClickAction,
    pub(super) record_quick_capture: ClickAction,
    pub(super) restore_defaults: ClickAction,
}

#[derive(Clone, Copy)]
pub(super) struct OcrPageActions {
    pub(super) enabled: ToggleAction,
    pub(super) download_models: ClickAction,
}

#[derive(Clone, Copy)]
pub(super) struct AboutPageActions {
    pub(super) open_repository: PlainClickAction,
    pub(super) report_issue: PlainClickAction,
}

#[derive(Clone, Copy)]
pub(super) struct PreferencesRenderActions {
    pub(super) general: GeneralPageActions,
    pub(super) notifications: NotificationsPageActions,
    pub(super) shortcuts: ShortcutsPageActions,
    pub(super) ocr: OcrPageActions,
    pub(super) about: AboutPageActions,
}

impl Default for PreferencesRenderActions {
    fn default() -> Self {
        Self {
            general: GeneralPageActions {
                auto_start: PreferencesView::on_auto_start_changed,
                language: PreferencesView::on_language_selected,
                theme: PreferencesView::on_theme_selected,
                font: PreferencesView::on_font_selected,
                browse_save_path: PreferencesView::on_browse_save_path,
                image_compression: PreferencesView::on_image_compression_changed,
            },
            notifications: NotificationsPageActions {
                enabled: PreferencesView::on_notifications_enabled_changed,
                save_notification: PreferencesView::on_save_notification_changed,
                copy_notification: PreferencesView::on_copy_notification_changed,
                qr_code_notification: PreferencesView::on_qr_notification_changed,
                shutter_sound: PreferencesView::on_shutter_sound_changed,
            },
            shortcuts: ShortcutsPageActions {
                record_capture: PreferencesView::on_capture_shortcut_record,
                record_quick_capture: PreferencesView::on_quick_shortcut_record,
                restore_defaults: PreferencesView::on_restore_default_shortcuts,
            },
            ocr: OcrPageActions {
                enabled: PreferencesView::on_ocr_enabled_changed,
                download_models: PreferencesView::on_download_ocr_models,
            },
            about: AboutPageActions {
                open_repository: PreferencesView::on_open_repository,
                report_issue: PreferencesView::on_report_issue,
            },
        }
    }
}

pub(super) fn render_active_page(frame: &PreferencesFrame, actions: &PreferencesRenderActions, cx: &mut Context<PreferencesView>) -> AnyElement {
    match frame.active_page {
        super::session::state::PreferencesPage::General => general::render(&frame.general, actions.general, cx),
        super::session::state::PreferencesPage::Notifications => notifications::render(&frame.notifications, actions.notifications, cx),
        super::session::state::PreferencesPage::Shortcuts => shortcuts::render(&frame.shortcuts, actions.shortcuts, cx),
        super::session::state::PreferencesPage::Ocr => ocr::render(&frame.ocr, actions.ocr, cx),
        super::session::state::PreferencesPage::About => about::render(&frame.about, actions.about, cx),
    }
}

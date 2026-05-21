use anyhow::Result;
use gpui::{App, Window, WindowOptions};

pub(crate) use super::hotkey::HotkeyService;
use super::native_window::{Level, WindowLevelExt};
pub(crate) use super::notify::NotificationType;
pub(crate) use super::system::UiSystemActions;
pub(crate) use super::window_drag::{PopupDragBehavior, PopupDragRegionExt};
pub(crate) use super::windowing::PopupWindowSpec;
use crate::services::app_meta::APP_ID;

pub(crate) fn popup_window_options(spec: PopupWindowSpec) -> WindowOptions {
    super::windowing::popup_window_options(spec, APP_ID)
}

pub(crate) fn configure_window(window: &mut Window, cx: &mut App, focus: bool) {
    super::windowing::configure_window(window, cx, focus);
}

pub(crate) fn with_always_on_top<R: 'static>(
    f: impl FnOnce(&mut Window, &mut App) -> R + 'static,
) -> impl FnOnce(&mut Window, &mut App) -> R + 'static {
    super::native_window::with_level(Level::AlwaysOnTop, f)
}

pub(crate) fn set_always_on_top(window: &mut Window) -> Result<()> {
    window.set_level(Level::AlwaysOnTop)
}

pub(crate) fn set_click_through(window: &mut Window, enabled: bool) -> Result<()> {
    window.set_click_through(enabled)
}

pub(crate) fn show_notification(title: &str, message: &str, notification_type: NotificationType) {
    super::notify::show(title, message, notification_type);
}

pub(crate) fn copy_text_to_clipboard(text: String) -> bool {
    super::clipboard::copy_text_to_clipboard(text)
}

pub(crate) fn default_save_path() -> String {
    super::storage::get_default_save_path()
}

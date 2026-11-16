//! GPUI window construction, dragging and injected native window effects.
mod drag;
mod open;
mod options;

pub(crate) use drag::{PopupDragBehavior, PopupDragRegionExt};
use gpui::{App, Window};
pub(crate) use open::open_prepared_window;
pub(crate) use options::{PopupWindowSpec, configure_window, popup_window_options};

pub(crate) fn set_always_on_top(window: &mut Window, cx: &App) -> minnow_application::OperationResult<()> {
    crate::host(cx).set_always_on_top(window)
}

pub(crate) fn set_click_through(window: &mut Window, enabled: bool, cx: &App) -> minnow_application::OperationResult<()> {
    crate::host(cx).set_click_through(window, enabled)
}

/// Shared preparation for all three long-capture windows.
pub(crate) fn prepare_capture_window(window: &mut Window, cx: &App) -> minnow_application::OperationResult<()> {
    crate::host(cx).exclude_from_capture(window)?;
    set_always_on_top(window, cx)
}

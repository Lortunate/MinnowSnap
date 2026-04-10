mod actions;
mod annotation;
mod interaction;
pub(crate) mod render;
mod state;
mod view;
pub(crate) mod window_catalog;

use crate::shell::windowing::{PopupWindowSpec, configure_window, popup_window_options};
use gpui::{App, AppContext, Bounds, WindowBounds, WindowKind, WindowOptions};
use gpui_component::Root;
use minnow_core::app_meta::APP_ID;

pub use actions::bind_keys;
pub use state::OverlayHandle;
use view::OverlayView;

pub fn open_window(cx: &mut App) {
    let options = window_options(cx);
    let overlay_handle = cx.global::<OverlayHandle>().clone();

    if let Err(err) = cx.open_window(options, move |window, cx| {
        configure_window(window, cx, true);
        let focus_handle = cx.focus_handle();
        let overlay_handle = overlay_handle.clone();
        let view = cx.new(move |cx| OverlayView::new(overlay_handle, focus_handle, cx));
        cx.new(move |cx| Root::new(view, window, cx))
    }) {
        tracing::error!("Failed to open overlay window: {err}");
    }
}

fn window_options(cx: &App) -> WindowOptions {
    let fullscreen_bounds = Bounds::maximized(None, cx);

    popup_window_options(
        PopupWindowSpec {
            window_bounds: Some(WindowBounds::Fullscreen(fullscreen_bounds)),
            kind: WindowKind::PopUp,
            focus: false,
            show: true,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            display_id: None,
            window_min_size: None,
        },
        APP_ID,
    )
}

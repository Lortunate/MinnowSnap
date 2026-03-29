mod actions;
mod interaction;
mod render;
mod session;
mod view;

use crate::core::app::APP_ID;
use gpui::{App, AppContext, Bounds, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions};
use gpui_component::Root;

pub use actions::bind_keys;
pub use session::OverlayHandle;
use view::OverlayView;

pub fn open_window(cx: &mut App) {
    let options = window_options(cx);
    let overlay_handle = cx.global::<OverlayHandle>().clone();

    if let Err(err) = cx.open_window(options, move |window, cx| {
        crate::core::appearance::apply_saved_preferences(Some(window), cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        let overlay_handle = overlay_handle.clone();
        let view = cx.new(move |cx| OverlayView::new(overlay_handle, focus_handle, cx));
        cx.new(move |cx| Root::new(view, window, cx))
    }) {
        tracing::error!("Failed to open overlay window: {err}");
    }
}

fn window_options(cx: &App) -> WindowOptions {
    let fullscreen_bounds = Bounds::maximized(None, cx);

    WindowOptions {
        window_bounds: Some(WindowBounds::Fullscreen(fullscreen_bounds)),
        kind: WindowKind::PopUp,
        focus: false,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: None,
        window_background: WindowBackgroundAppearance::Transparent,
        window_decorations: None,
        app_id: Some(APP_ID.to_string()),
        ..WindowOptions::default()
    }
}

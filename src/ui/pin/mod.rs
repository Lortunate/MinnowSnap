mod actions;
mod render;
mod request;
mod session;
mod view;

pub use actions::bind_keys;
pub use request::PinRequest;

use crate::core::app::APP_ID;
use gpui::{App, AppContext, Bounds, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, point, px, size};
use gpui_component::Root;
use session::{PinManager, PinSession};
use tracing::info;
use view::PinView;

pub(crate) fn install(cx: &mut App) {
    let manager = PinManager::new(cx);
    cx.set_global(manager);
}

pub fn open_window(cx: &mut App, request: PinRequest) {
    let options = window_options(cx, &request);
    let manager = cx.global::<PinManager>().clone();

    if let Err(err) = cx.open_window(
        options,
        gpui_window_ext::with_level(gpui_window_ext::Level::AlwaysOnTop, move |window, cx| {
            crate::core::appearance::apply_saved_preferences(Some(window), cx);
            let focus_handle = cx.focus_handle();
            focus_handle.focus(window);
            manager.register(window.window_handle(), cx);
            let session = PinSession::new(cx, request);
            let manager = manager.clone();
            let view = cx.new(move |cx| PinView::new(session, manager, focus_handle, cx));
            cx.new(move |cx| Root::new(view, window, cx))
        }),
    ) {
        tracing::error!("Failed to open pin window: {err}");
    }
}

pub fn window_options(cx: &App, request: &PinRequest) -> WindowOptions {
    let geometry = PinSession::initial_geometry(request);
    let window_size = geometry.window_size();
    let bounds = if let Some((x, y)) = geometry.origin() {
        Bounds::new(point(px(x), px(y)), window_size)
    } else if let Some(display) = cx.displays().first().cloned() {
        Bounds::centered(Some(display.id()), window_size, cx)
    } else {
        Bounds::new(point(px(0.0), px(0.0)), window_size)
    };
    info!(
        target: "minnowsnap::pin",
        bounds = ?bounds,
        "pin window options prepared"
    );

    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        window_background: WindowBackgroundAppearance::Transparent,
        focus: true,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: true,
        is_resizable: true,
        is_minimizable: false,
        display_id: cx.displays().first().map(|display| display.id()),
        app_id: Some(APP_ID.to_string()),
        window_decorations: None,
        tabbing_identifier: None,
        window_min_size: Some(size(px(geometry.min_size()), px(geometry.min_size()))),
        ..WindowOptions::default()
    }
}

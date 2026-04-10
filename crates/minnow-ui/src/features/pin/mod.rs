mod actions;
mod render;
mod request;
mod state;
mod view;

pub use actions::bind_keys;
pub use request::PinRequest;

use crate::shell::native_window::{Level, with_level};
use crate::shell::windowing::{PopupWindowSpec, configure_window, popup_window_options};
use gpui::{App, AppContext, Bounds, WindowBounds, WindowKind, WindowOptions, point, px, size};
use gpui_component::Root;
use minnow_core::app_meta::APP_ID;
use state::{PinManager, PinSession};
use tracing::info;
use view::PinView;

pub fn install(cx: &mut App) {
    let manager = PinManager::new(cx);
    cx.set_global(manager);
}

pub fn open_window(cx: &mut App, request: PinRequest) {
    let options = window_options(cx, &request);
    let manager = cx.global::<PinManager>().clone();

    if let Err(err) = cx.open_window(
        options,
        with_level(Level::AlwaysOnTop, move |window, cx| {
            configure_window(window, cx, true);
            let focus_handle = cx.focus_handle();
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

fn window_options(cx: &App, request: &PinRequest) -> WindowOptions {
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

    popup_window_options(
        PopupWindowSpec {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            focus: true,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: true,
            is_resizable: true,
            is_minimizable: false,
            display_id: cx.displays().first().map(|display| display.id()),
            window_min_size: Some(size(px(geometry.min_size()), px(geometry.min_size()))),
        },
        APP_ID,
    )
}

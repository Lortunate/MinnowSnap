mod render;
mod request;
mod selection_text;
mod state;
mod view;

pub use request::PinRequest;

use crate::presentation::appearance;
use crate::windowing::{self, PopupWindowSpec};
use gpui::{App, AppContext, Bounds, KeyBinding, WindowBounds, WindowKind, WindowOptions, actions, point, px, size};
use gpui_component::Root;
use state::{PinManager, PinSession};
use tracing::info;
use view::PinView;

const PIN_CONTEXT: &str = "MinnowSnapPin";

actions!(pin, [ClosePin, CloseAllPins, CopyPinContent, SavePinImage]);

pub fn install(cx: &mut App) {
    let manager = PinManager::new(cx);
    cx.set_global(manager);
}

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", ClosePin, Some(PIN_CONTEXT)),
        KeyBinding::new("ctrl-w", ClosePin, Some(PIN_CONTEXT)),
        KeyBinding::new("ctrl-shift-w", CloseAllPins, Some(PIN_CONTEXT)),
        KeyBinding::new("ctrl-c", CopyPinContent, Some(PIN_CONTEXT)),
        KeyBinding::new("ctrl-s", SavePinImage, Some(PIN_CONTEXT)),
    ]);
}

pub(crate) fn open_window(cx: &mut App, request: PinRequest) -> minnow_application::OperationResult<()> {
    let manager = cx.global::<PinManager>().clone();
    let lease = manager.reserve(&request)?;
    let options = window_options(cx, &request);
    let view_manager = manager.clone();

    let opened = windowing::open_prepared_window(cx, options, windowing::set_always_on_top, move |window, cx| {
        appearance::apply_saved_preferences(Some(window), cx);
        let focus_handle = cx.focus_handle();
        windowing::configure_window(window, cx, &focus_handle);
        view_manager.register(window.window_handle(), cx);
        let session = PinSession::new(cx, request, lease);
        let manager = view_manager.clone();
        let view = cx.new(|cx| PinView::new(session, manager, focus_handle, window, cx));
        cx.new(move |cx| Root::new(view, window, cx))
    });
    if opened.is_err() {
        // Programmatic rejection bypasses the pin's close action. The window
        // removal drops its view/session; also discard its manager handle.
        manager.prune_closed(cx);
    }
    opened.map(|_| ())
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

    windowing::popup_window_options(PopupWindowSpec {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        focus: true,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: true,
        is_resizable: true,
        is_minimizable: false,
        display_id: cx.displays().first().map(|display| display.id()),
        window_min_size: Some(size(px(geometry.min_size()), px(geometry.min_size()))),
    })
}

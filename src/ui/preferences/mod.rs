mod render;
mod session;
mod view;

use crate::core::app::APP_ID;
use crate::ui::windowing::{PopupWindowSpec, configure_window, popup_window_options};
use gpui::{App, AppContext, Bounds, WindowBounds, WindowKind, WindowOptions, px, size};
use gpui_component::Root;
use view::PreferencesView;

fn window_options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(760.0), px(520.0)), cx);

    popup_window_options(
        PopupWindowSpec {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::PopUp,
            focus: true,
            show: true,
            is_movable: true,
            is_resizable: true,
            is_minimizable: false,
            display_id: cx.displays().first().map(|display| display.id()),
            window_min_size: Some(size(px(640.0), px(420.0))),
        },
        APP_ID,
    )
}

pub fn open_window(cx: &mut App) {
    let options = window_options(cx);

    if let Err(err) = cx.open_window(options, |window, cx| {
        configure_window(window, cx, true);
        let focus_handle = cx.focus_handle();
        let view = cx.new(move |_| PreferencesView::new(focus_handle));
        cx.new(move |cx| Root::new(view, window, cx))
    }) {
        tracing::error!("Failed to open preferences window: {err}");
    }
}

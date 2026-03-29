mod render;
mod session;
mod view;

use crate::core::app::APP_ID;
use gpui::{App, AppContext, Bounds, WindowBackgroundAppearance, WindowBounds, WindowOptions, px, size};
use gpui_component::Root;
use view::PreferencesView;

pub fn window_options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(760.0), px(520.0)), cx);

    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        window_background: WindowBackgroundAppearance::Transparent,
        focus: true,
        show: true,
        is_movable: true,
        is_resizable: true,
        is_minimizable: false,
        display_id: cx.displays().first().map(|display| display.id()),
        app_id: Some(APP_ID.to_string()),
        window_decorations: None,
        tabbing_identifier: None,
        window_min_size: Some(size(px(640.0), px(420.0))),
        ..WindowOptions::default()
    }
}

pub fn open_window(cx: &mut App) {
    let options = window_options(cx);

    if let Err(err) = cx.open_window(options, |window, cx| {
        crate::core::appearance::apply_saved_preferences(Some(window), cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        let view = cx.new(move |_| PreferencesView::new(focus_handle));
        cx.new(move |cx| Root::new(view, window, cx))
    }) {
        tracing::error!("Failed to open preferences window: {err}");
    }
}

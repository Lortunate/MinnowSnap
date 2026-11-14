mod actions;
mod annotation;
pub(crate) mod render;
mod state;
mod view;
pub(crate) mod window_catalog;

use crate::presentation::appearance;
use crate::windowing::{self, PopupWindowSpec};
use gpui::{App, AppContext, Bounds, WindowBounds, WindowKind, WindowOptions};
use gpui_component::Root;

pub use actions::bind_keys;
pub use state::OverlayHandle;
pub(crate) use state::OverlaySurface;
use view::OverlayView;

pub(crate) fn open_window(cx: &mut App) -> minnow_application::OperationResult<gpui::AnyWindowHandle> {
    let options = window_options(cx);
    let overlay_handle = cx.global::<OverlayHandle>().clone();

    cx.open_window(options, move |window, cx| {
        appearance::apply_saved_preferences(Some(window), cx);
        let focus_handle = cx.focus_handle();
        windowing::configure_window(window, cx, &focus_handle);
        let overlay_handle = overlay_handle.clone();
        let view = cx.new(|cx| OverlayView::new(overlay_handle, focus_handle, window, cx));
        cx.new(move |cx| Root::new(view, window, cx))
    })
    .map(Into::into)
    .map_err(|error| minnow_application::OperationError::new(minnow_application::ErrorKind::Native, error.to_string()))
}

fn window_options(cx: &App) -> WindowOptions {
    let geometry = cx.global::<OverlayHandle>().capture_geometry(cx);
    let bounds = geometry
        .map(|geometry| {
            Bounds::new(
                gpui::point(gpui::px(geometry.screen_origin.x as f32), gpui::px(geometry.screen_origin.y as f32)),
                gpui::size(
                    gpui::px(geometry.pixel_size.0 as f32 / geometry.scale_factor as f32),
                    gpui::px(geometry.pixel_size.1 as f32 / geometry.scale_factor as f32),
                ),
            )
        })
        .unwrap_or_else(|| Bounds::maximized(None, cx));

    windowing::popup_window_options(PopupWindowSpec {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        kind: WindowKind::PopUp,
        focus: false,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: None,
        window_min_size: None,
    })
}

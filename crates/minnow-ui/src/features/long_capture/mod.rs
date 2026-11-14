mod coordinator;
mod layout;
mod view;

use crate::presentation::appearance;
use crate::windowing::{self, PopupWindowSpec};
use gpui::{App, AppContext, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions};
use minnow_imaging::geometry::{Rect, RectF};
use std::sync::Arc;
use view::{FrameWindowView, LongCaptureToolbarAction, PreviewWindowView, ToolbarWindowView};
use {coordinator::LongCaptureCoordinator, coordinator::LongCaptureWindowKind, layout::compute_window_layout};

#[derive(Clone)]
pub struct LongCaptureRequest {
    pub geometry: minnow_imaging::CaptureGeometry,
    pub selection_rect: Rect,
    pub viewport_rect: RectF,
    pub viewport_origin_screen: (f64, f64),
}

impl LongCaptureRequest {
    #[must_use]
    pub fn selection_rectf(&self) -> RectF {
        RectF::new(
            f64::from(self.selection_rect.x),
            f64::from(self.selection_rect.y),
            f64::from(self.selection_rect.width.max(0)),
            f64::from(self.selection_rect.height.max(0)),
        )
    }

    #[must_use]
    pub fn map_local_rect_to_screen(&self, rect: RectF) -> RectF {
        RectF::new(
            self.viewport_origin_screen.0 + rect.x,
            self.viewport_origin_screen.1 + rect.y,
            rect.width,
            rect.height,
        )
    }
}

pub(crate) fn open_window(
    cx: &mut App,
    request: LongCaptureRequest,
    capture: minnow_application::LongCaptureHandle,
) -> minnow_application::OperationResult<()> {
    let layout = compute_window_layout(&request, LongCaptureToolbarAction::ORDERED.len());
    let coordinator = Arc::new(LongCaptureCoordinator::new(capture));

    if let Err(error) = windowing::open_prepared_window(
        cx,
        window_options(layout.frame_bounds, false),
        |window, cx| {
            windowing::set_always_on_top(window, cx)?;
            windowing::set_click_through(window, true, cx)
        },
        {
            let request = request.clone();
            let coordinator = coordinator.clone();
            move |window, cx| {
                appearance::apply_saved_preferences(Some(window), cx);
                window.set_background_appearance(WindowBackgroundAppearance::Transparent);
                coordinator.register_window(LongCaptureWindowKind::Frame, window.window_handle());
                cx.new(|cx| FrameWindowView::new(request, coordinator, window, cx))
            }
        },
    ) {
        tracing::warn!("Long capture selection frame hidden: {error}");
        coordinator.on_frame_click_through_result(false);
        coordinator.report_window_warning(format!("Selection frame hidden: {error}"));
    }

    if let Err(error) = windowing::open_prepared_window(cx, window_options(layout.toolbar_bounds, true), windowing::set_always_on_top, {
        let coordinator = coordinator.clone();
        move |window, cx| {
            appearance::apply_saved_preferences(Some(window), cx);
            window.set_background_appearance(WindowBackgroundAppearance::Transparent);
            let focus_handle = cx.focus_handle();
            windowing::configure_window(window, cx, &focus_handle);
            coordinator.register_window(LongCaptureWindowKind::Toolbar, window.window_handle());
            cx.new(|cx| ToolbarWindowView::new(coordinator, focus_handle, window, cx))
        }
    }) {
        tracing::error!("Failed to prepare long capture toolbar: {error}");
        coordinator.cancel_capture();
        coordinator.close_windows_except(None, cx);
        return Err(error);
    }

    if let Err(error) = windowing::open_prepared_window(cx, window_options(layout.preview_bounds, false), windowing::set_always_on_top, {
        let coordinator = coordinator.clone();
        move |window, cx| {
            appearance::apply_saved_preferences(Some(window), cx);
            window.set_background_appearance(WindowBackgroundAppearance::Transparent);
            coordinator.register_window(LongCaptureWindowKind::Preview, window.window_handle());
            cx.new(|cx| PreviewWindowView::new(coordinator, window, cx))
        }
    }) {
        tracing::warn!("Long capture preview hidden: {error}");
        coordinator.report_window_warning(format!("Preview hidden: {error}"));
    }
    Ok(())
}

fn window_options(bounds: gpui::Bounds<gpui::Pixels>, focus: bool) -> WindowOptions {
    windowing::popup_window_options(PopupWindowSpec {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        kind: WindowKind::PopUp,
        focus,
        show: true,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: None,
        window_min_size: None,
    })
}

#[cfg(test)]
pub(crate) fn test_geometry() -> minnow_imaging::CaptureGeometry {
    minnow_imaging::CaptureGeometry {
        display_id: minnow_imaging::DisplayId("test".into()),
        screen_origin: minnow_imaging::ScreenPoint { x: 0.0, y: 0.0 },
        scale_factor: 1.0,
        pixel_size: (1200, 800),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_maps_local_rect_to_screen_coordinates() {
        let request = LongCaptureRequest {
            geometry: test_geometry(),
            selection_rect: Rect::new(10, 20, 100, 120),
            viewport_rect: RectF::new(0.0, 0.0, 1200.0, 800.0),
            viewport_origin_screen: (320.0, -80.0),
        };

        let local = RectF::new(50.0, 70.0, 200.0, 100.0);
        let mapped = request.map_local_rect_to_screen(local);

        assert_eq!(mapped, RectF::new(370.0, -10.0, 200.0, 100.0));
    }
}

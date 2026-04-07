mod coordinator;
mod layout;
mod view;

use crate::core::app::APP_ID;
use crate::core::geometry::{Rect, RectF};
use crate::ui::windowing::{PopupWindowSpec, configure_window, popup_window_options};
use gpui::{App, AppContext, WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions};
use gpui_window_ext::{Level, WindowLevelExt};
use std::sync::Arc;
use view::{FrameWindowView, LongCaptureToolbarAction, PreviewWindowView, ToolbarWindowView};
use {coordinator::LongCaptureCoordinator, coordinator::LongCaptureWindowKind, layout::compute_window_layout};

#[derive(Clone)]
pub struct LongCaptureRequest {
    pub selection_rect: Rect,
    pub viewport_rect: RectF,
    pub viewport_scale: f64,
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

pub fn open_window(cx: &mut App, request: LongCaptureRequest) {
    let layout = compute_window_layout(&request, LongCaptureToolbarAction::ORDERED.len());
    let coordinator = Arc::new(LongCaptureCoordinator::new(request.clone()));

    if let Err(err) = cx.open_window(window_options(layout.frame_bounds, false), {
        let request = request.clone();
        let coordinator = coordinator.clone();
        move |window, cx| {
            configure_window(window, cx, false);
            window.set_background_appearance(WindowBackgroundAppearance::Transparent);
            if let Err(err) = window.set_level(Level::AlwaysOnTop) {
                tracing::warn!("Failed to set frame window level: {err}");
            }

            let frame_click_through_ok = window.set_click_through(true).is_ok();
            if !frame_click_through_ok {
                coordinator.on_frame_click_through_result(false);
                window.defer(cx, |window, _| {
                    window.remove_window();
                });
            } else {
                coordinator.register_window(LongCaptureWindowKind::Frame, window.window_handle());
            }

            cx.new(|cx| FrameWindowView::new(request, coordinator, window, cx))
        }
    }) {
        tracing::error!("Failed to open long capture frame window: {err}");
        coordinator.on_frame_click_through_result(false);
    }

    if let Err(err) = cx.open_window(window_options(layout.toolbar_bounds, true), {
        let coordinator = coordinator.clone();
        move |window, cx| {
            configure_window(window, cx, true);
            window.set_background_appearance(WindowBackgroundAppearance::Transparent);
            if let Err(err) = window.set_level(Level::AlwaysOnTop) {
                tracing::warn!("Failed to set toolbar window level: {err}");
            }
            let focus_handle = cx.focus_handle();
            coordinator.register_window(LongCaptureWindowKind::Toolbar, window.window_handle());
            cx.new(|cx| ToolbarWindowView::new(coordinator, focus_handle, window, cx))
        }
    }) {
        tracing::error!("Failed to open long capture toolbar window: {err}");
        coordinator.cancel_capture();
        coordinator.close_windows_except(None, cx);
        return;
    }

    if let Err(err) = cx.open_window(window_options(layout.preview_bounds, false), move |window, cx| {
        configure_window(window, cx, false);
        window.set_background_appearance(WindowBackgroundAppearance::Transparent);
        if let Err(err) = window.set_level(Level::AlwaysOnTop) {
            tracing::warn!("Failed to set preview window level: {err}");
        }
        coordinator.register_window(LongCaptureWindowKind::Preview, window.window_handle());
        cx.new(|cx| PreviewWindowView::new(coordinator, window, cx))
    }) {
        tracing::error!("Failed to open long capture preview window: {err}");
    }
}

fn window_options(bounds: gpui::Bounds<gpui::Pixels>, focus: bool) -> WindowOptions {
    popup_window_options(
        PopupWindowSpec {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::PopUp,
            focus,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_maps_local_rect_to_screen_coordinates() {
        let request = LongCaptureRequest {
            selection_rect: Rect::new(10, 20, 100, 120),
            viewport_rect: RectF::new(0.0, 0.0, 1200.0, 800.0),
            viewport_scale: 1.0,
            viewport_origin_screen: (320.0, -80.0),
        };

        let local = RectF::new(50.0, 70.0, 200.0, 100.0);
        let mapped = request.map_local_rect_to_screen(local);

        assert_eq!(mapped, RectF::new(370.0, -10.0, 200.0, 100.0));
    }
}

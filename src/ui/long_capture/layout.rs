use super::LongCaptureRequest;
use crate::core::geometry::RectF;
use crate::ui::overlay::render::layout::{resolve_toolbar_layout, toolbar_size};
use gpui::{Bounds, Pixels, point, px, size};

const PREVIEW_WIDTH: f64 = 320.0;
const PREVIEW_HEIGHT: f64 = 230.0;
const PREVIEW_MARGIN: f64 = 20.0;
const WARNING_HEIGHT: f64 = 34.0;
pub(crate) const TOOLBAR_TOP_RESERVED: f64 = WARNING_HEIGHT + 8.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct LongCaptureWindowLayout {
    pub(crate) frame_bounds: Bounds<Pixels>,
    pub(crate) toolbar_bounds: Bounds<Pixels>,
    pub(crate) preview_bounds: Bounds<Pixels>,
}

pub(crate) fn compute_window_layout(request: &LongCaptureRequest, toolbar_action_count: usize) -> LongCaptureWindowLayout {
    let frame_local = RectF::new(0.0, 0.0, request.viewport_rect.width, request.viewport_rect.height);
    let toolbar_local = compute_toolbar_window_local_rect(request, toolbar_action_count);
    let preview_local = compute_preview_window_local_rect(request);

    LongCaptureWindowLayout {
        frame_bounds: rectf_to_bounds(request.map_local_rect_to_screen(frame_local)),
        toolbar_bounds: rectf_to_bounds(request.map_local_rect_to_screen(toolbar_local)),
        preview_bounds: rectf_to_bounds(request.map_local_rect_to_screen(preview_local)),
    }
}

pub(crate) fn frame_visibility_after_click_through(success: bool) -> bool {
    success
}

fn compute_toolbar_window_local_rect(request: &LongCaptureRequest, action_count: usize) -> RectF {
    let selection = request.selection_rectf();
    let toolbar_layout = resolve_toolbar_layout(selection, action_count, request.viewport_rect.width, request.viewport_rect.height, &[]);
    let (toolbar_width, toolbar_height) = toolbar_size(action_count);
    RectF::new(
        toolbar_layout.x,
        (toolbar_layout.y - TOOLBAR_TOP_RESERVED).max(0.0),
        toolbar_width,
        toolbar_height + TOOLBAR_TOP_RESERVED,
    )
}

fn compute_preview_window_local_rect(request: &LongCaptureRequest) -> RectF {
    let selection = request.selection_rectf();
    let viewport_w = request.viewport_rect.width;
    let viewport_h = request.viewport_rect.height;

    let right_space = viewport_w - (selection.x + selection.width);
    let desired_x = if right_space >= PREVIEW_WIDTH + PREVIEW_MARGIN {
        selection.x + selection.width + PREVIEW_MARGIN
    } else {
        selection.x - PREVIEW_WIDTH - PREVIEW_MARGIN
    };

    let min_x = PREVIEW_MARGIN;
    let max_x = (viewport_w - PREVIEW_WIDTH - PREVIEW_MARGIN).max(min_x);
    let x = desired_x.clamp(min_x, max_x);

    let min_y = PREVIEW_MARGIN;
    let max_y = (viewport_h - PREVIEW_HEIGHT - PREVIEW_MARGIN).max(min_y);
    let y = selection.y.clamp(min_y, max_y);

    RectF::new(x, y, PREVIEW_WIDTH, PREVIEW_HEIGHT)
}

fn rectf_to_bounds(rect: RectF) -> Bounds<Pixels> {
    Bounds::new(
        point(px(rect.x as f32), px(rect.y as f32)),
        size(px(rect.width.max(1.0) as f32), px(rect.height.max(1.0) as f32)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::Rect;

    #[test]
    fn preview_and_toolbar_windows_remain_inside_viewport() {
        let request = LongCaptureRequest {
            selection_rect: Rect::new(860, 660, 260, 120),
            viewport_rect: RectF::new(0.0, 0.0, 1200.0, 800.0),
            viewport_scale: 1.0,
            viewport_origin_screen: (0.0, 0.0),
        };

        let preview = compute_preview_window_local_rect(&request);
        let toolbar = compute_toolbar_window_local_rect(&request, 4);

        assert!(preview.x >= PREVIEW_MARGIN);
        assert!(preview.y >= PREVIEW_MARGIN);
        assert!(preview.x + PREVIEW_WIDTH <= request.viewport_rect.width - PREVIEW_MARGIN + 0.0001);
        assert!(preview.y + PREVIEW_HEIGHT <= request.viewport_rect.height - PREVIEW_MARGIN + 0.0001);
        assert!(toolbar.x >= 0.0);
        assert!(toolbar.y >= 0.0);
        assert!(toolbar.x + toolbar.width <= request.viewport_rect.width + 0.0001);
        assert!(toolbar.y + toolbar.height <= request.viewport_rect.height + 0.0001);
    }

    #[test]
    fn frame_hides_when_click_through_setup_fails() {
        assert!(frame_visibility_after_click_through(true));
        assert!(!frame_visibility_after_click_through(false));
    }
}

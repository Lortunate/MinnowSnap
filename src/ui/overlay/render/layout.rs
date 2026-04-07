use crate::core::geometry::RectF;

const VIEWPORT_MARGIN: f64 = 16.0;
pub(crate) const SELECTION_PANEL_GAP: f64 = 8.0;
const TOOLBAR_BUTTON_SIZE: f64 = 32.0;
const TOOLBAR_BUTTON_GAP: f64 = 2.0;
const TOOLBAR_PADDING_X: f64 = 8.0;
const TOOLBAR_PADDING_Y: f64 = 4.0;
const PROPERTY_BUTTON_SIZE: f64 = 32.0;
const PROPERTY_SWATCH_SIZE: f64 = 26.0;
const PROPERTY_SWATCH_COUNT: f64 = 6.0;
const PROPERTY_SWATCH_GAP: f64 = 4.0;
const PROPERTY_CUSTOM_COLOR_BUTTON_WIDTH: f64 = 84.0;
const PROPERTY_BUTTON_GAP: f64 = 4.0;
const PROPERTY_PANEL_PADDING_X: f64 = 8.0;
const PROPERTY_PANEL_PADDING_Y: f64 = 8.0;
const PROPERTY_ROW_PADDING_X: f64 = 8.0;
const PROPERTY_ROW_PADDING_Y: f64 = 4.0;
const PROPERTY_ROW_GAP: f64 = 4.0;
const PROPERTY_SIZE_LABEL_WIDTH: f64 = 56.0;
const WINDOW_INFO_MAX_WIDTH: f64 = 340.0;
const WINDOW_INFO_RESERVED_HEIGHT: f64 = 44.0;
const RESOLUTION_TOOLTIP_HEIGHT: f64 = 36.0;
const RESOLUTION_TOOLTIP_MIN_WIDTH: f64 = 88.0;
const TOOLTIP_HORIZONTAL_PADDING: f64 = 16.0;
const TOOLTIP_CHARACTER_WIDTH: f64 = 7.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OverlayPanelLayout {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl OverlayPanelLayout {
    pub fn as_rect(self) -> RectF {
        RectF::new(self.x, self.y, self.width, self.height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VerticalSide {
    Above,
    Below,
}

#[derive(Clone, Copy)]
struct PanelLayoutRequest<'a> {
    target: RectF,
    desired_x: f64,
    width: f64,
    height: f64,
    viewport_w: f64,
    viewport_h: f64,
    preferred_side: VerticalSide,
    occupied: &'a [OverlayPanelLayout],
}

pub(crate) fn toolbar_size(action_count: usize) -> (f64, f64) {
    let button_count = action_count.max(1) as f64;
    let width = button_count * TOOLBAR_BUTTON_SIZE + (button_count - 1.0) * TOOLBAR_BUTTON_GAP + TOOLBAR_PADDING_X * 2.0;
    let height = TOOLBAR_BUTTON_SIZE + TOOLBAR_PADDING_Y * 2.0;
    (width, height)
}

pub(crate) fn property_panel_size(include_text_action: bool, include_mosaic_controls: bool) -> (f64, f64) {
    let row_height = PROPERTY_BUTTON_SIZE.max(PROPERTY_SWATCH_SIZE) + PROPERTY_ROW_PADDING_Y * 2.0;

    let swatch_strip_width = PROPERTY_SWATCH_COUNT * PROPERTY_SWATCH_SIZE + (PROPERTY_SWATCH_COUNT - 1.0) * PROPERTY_SWATCH_GAP;
    let color_row_width = PROPERTY_ROW_PADDING_X * 2.0 + swatch_strip_width + PROPERTY_BUTTON_GAP + PROPERTY_CUSTOM_COLOR_BUTTON_WIDTH;

    let parameter_button_count = 3.0;
    let parameter_elements = parameter_button_count + 1.0;
    let parameter_row_width = PROPERTY_ROW_PADDING_X * 2.0
        + parameter_button_count * PROPERTY_BUTTON_SIZE
        + PROPERTY_SIZE_LABEL_WIDTH
        + (parameter_elements - 1.0) * PROPERTY_BUTTON_GAP;

    let mode_button_count = (if include_mosaic_controls { 2.0 } else { 0.0 }) + (if include_text_action { 1.0 } else { 0.0 });
    let mode_row_width = if mode_button_count > 0.0 {
        PROPERTY_ROW_PADDING_X * 2.0 + mode_button_count * PROPERTY_BUTTON_SIZE + (mode_button_count - 1.0) * PROPERTY_BUTTON_GAP
    } else {
        0.0
    };

    let row_count = if mode_button_count > 0.0 { 3.0 } else { 2.0 };
    let width = color_row_width.max(parameter_row_width).max(mode_row_width) + PROPERTY_PANEL_PADDING_X * 2.0;
    let height = row_height * row_count + PROPERTY_ROW_GAP * (row_count - 1.0) + PROPERTY_PANEL_PADDING_Y * 2.0;
    (width, height)
}

fn resolution_tooltip_size(selection: RectF) -> (f64, f64) {
    let label = format!("{:.0} × {:.0}", selection.width.max(0.0), selection.height.max(0.0));
    let width = (label.chars().count() as f64 * TOOLTIP_CHARACTER_WIDTH + TOOLTIP_HORIZONTAL_PADDING).max(RESOLUTION_TOOLTIP_MIN_WIDTH);
    (width, RESOLUTION_TOOLTIP_HEIGHT)
}

fn resolve_panel_layout(request: PanelLayoutRequest<'_>) -> OverlayPanelLayout {
    let opposite_side = match request.preferred_side {
        VerticalSide::Above => VerticalSide::Below,
        VerticalSide::Below => VerticalSide::Above,
    };

    let inside_side = preferred_inside_side(request.target, request.viewport_h);
    let mut first_colliding = None;

    for candidate_y in [
        outside_position(request.target, request.height, request.viewport_h, request.preferred_side),
        outside_position(request.target, request.height, request.viewport_h, opposite_side),
        Some(inside_position(request.target, request.height, inside_side)),
    ]
    .into_iter()
    .flatten()
    {
        let candidate = clamp_layout(
            request.desired_x,
            candidate_y,
            request.width,
            request.height,
            request.viewport_w,
            request.viewport_h,
        );
        if !overlaps_any(candidate, request.occupied) {
            return candidate;
        }
        first_colliding.get_or_insert(candidate);
    }

    first_colliding.unwrap_or_else(|| {
        clamp_layout(
            request.desired_x,
            inside_position(request.target, request.height, inside_side),
            request.width,
            request.height,
            request.viewport_w,
            request.viewport_h,
        )
    })
}

fn preferred_inside_side(target: RectF, viewport_h: f64) -> VerticalSide {
    let free_above = (target.y - VIEWPORT_MARGIN).max(0.0);
    let free_below = (viewport_h - VIEWPORT_MARGIN - (target.y + target.height)).max(0.0);

    if free_below > free_above {
        VerticalSide::Below
    } else {
        VerticalSide::Above
    }
}

fn outside_position(target: RectF, panel_h: f64, viewport_h: f64, side: VerticalSide) -> Option<f64> {
    match side {
        VerticalSide::Above => {
            let y = target.y - panel_h - SELECTION_PANEL_GAP;
            (y >= VIEWPORT_MARGIN).then_some(y)
        }
        VerticalSide::Below => {
            let y = target.y + target.height + SELECTION_PANEL_GAP;
            (y + panel_h <= viewport_h - VIEWPORT_MARGIN).then_some(y)
        }
    }
}

fn inside_position(target: RectF, panel_h: f64, side: VerticalSide) -> f64 {
    match side {
        VerticalSide::Above => target.y + SELECTION_PANEL_GAP,
        VerticalSide::Below => target.y + target.height - panel_h - SELECTION_PANEL_GAP,
    }
}

fn overlaps_any(candidate: OverlayPanelLayout, occupied: &[OverlayPanelLayout]) -> bool {
    occupied.iter().copied().any(|other| rects_overlap(candidate.as_rect(), other.as_rect()))
}

fn rects_overlap(a: RectF, b: RectF) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

pub(crate) fn resolve_resolution_tooltip_layout_with_occupied(
    selection: RectF,
    viewport_w: f64,
    viewport_h: f64,
    occupied: &[OverlayPanelLayout],
) -> OverlayPanelLayout {
    let (tooltip_w, tooltip_h) = resolution_tooltip_size(selection);
    resolve_panel_layout(PanelLayoutRequest {
        target: selection,
        desired_x: selection.x,
        width: tooltip_w,
        height: tooltip_h,
        viewport_w,
        viewport_h,
        preferred_side: VerticalSide::Above,
        occupied,
    })
}

pub(crate) fn resolve_info_tooltip_layout(target: RectF, tooltip_height: f64, viewport_w: f64, viewport_h: f64) -> OverlayPanelLayout {
    resolve_panel_layout(PanelLayoutRequest {
        target,
        desired_x: target.x,
        width: WINDOW_INFO_MAX_WIDTH,
        height: tooltip_height,
        viewport_w,
        viewport_h,
        preferred_side: VerticalSide::Above,
        occupied: &[],
    })
}

pub(crate) fn resolve_toolbar_layout(
    selection: RectF,
    action_count: usize,
    viewport_w: f64,
    viewport_h: f64,
    occupied: &[OverlayPanelLayout],
) -> OverlayPanelLayout {
    let (toolbar_w, toolbar_h) = toolbar_size(action_count);
    resolve_panel_layout(PanelLayoutRequest {
        target: selection,
        desired_x: selection.x + selection.width - toolbar_w,
        width: toolbar_w,
        height: toolbar_h,
        viewport_w,
        viewport_h,
        preferred_side: VerticalSide::Below,
        occupied,
    })
}

pub(crate) fn resolve_info_reserved_slot_layout(selection: RectF, viewport_w: f64, viewport_h: f64) -> OverlayPanelLayout {
    resolve_panel_layout(PanelLayoutRequest {
        target: selection,
        desired_x: selection.x,
        width: WINDOW_INFO_MAX_WIDTH,
        height: WINDOW_INFO_RESERVED_HEIGHT,
        viewport_w,
        viewport_h,
        preferred_side: VerticalSide::Above,
        occupied: &[],
    })
}

pub(crate) fn resolve_property_layout(
    toolbar_layout: OverlayPanelLayout,
    reserved_info_slot: OverlayPanelLayout,
    include_text_action: bool,
    include_mosaic_controls: bool,
    viewport_w: f64,
    viewport_h: f64,
    occupied: &[OverlayPanelLayout],
) -> OverlayPanelLayout {
    let (panel_w, panel_h) = property_panel_size(include_text_action, include_mosaic_controls);

    let candidates = [
        (toolbar_layout.x + toolbar_layout.width + SELECTION_PANEL_GAP, toolbar_layout.y),
        (toolbar_layout.x, toolbar_layout.y + toolbar_layout.height + SELECTION_PANEL_GAP),
        (toolbar_layout.x, toolbar_layout.y - panel_h - SELECTION_PANEL_GAP),
        (toolbar_layout.x - panel_w - SELECTION_PANEL_GAP, toolbar_layout.y),
    ];

    let mut first = None;
    let mut first_without_reserved_overlap = None;
    for (x, y) in candidates {
        let candidate = clamp_layout(x, y, panel_w, panel_h, viewport_w, viewport_h);
        let overlaps_reserved = rects_overlap(candidate.as_rect(), reserved_info_slot.as_rect());
        let overlaps_occupied = overlaps_any(candidate, occupied);
        if !overlaps_reserved && !overlaps_occupied {
            return candidate;
        }
        first.get_or_insert(candidate);
        if !overlaps_reserved {
            first_without_reserved_overlap.get_or_insert(candidate);
        }
    }

    first_without_reserved_overlap.or(first).unwrap_or_else(|| {
        clamp_layout(
            toolbar_layout.x + toolbar_layout.width + SELECTION_PANEL_GAP,
            toolbar_layout.y,
            panel_w,
            panel_h,
            viewport_w,
            viewport_h,
        )
    })
}

fn clamp_layout(x: f64, y: f64, width: f64, height: f64, viewport_w: f64, viewport_h: f64) -> OverlayPanelLayout {
    let max_x = (viewport_w - width - VIEWPORT_MARGIN).max(VIEWPORT_MARGIN);
    let max_y = (viewport_h - height - VIEWPORT_MARGIN).max(VIEWPORT_MARGIN);
    OverlayPanelLayout {
        x: x.clamp(VIEWPORT_MARGIN, max_x),
        y: y.clamp(VIEWPORT_MARGIN, max_y),
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_ACTION_COUNT: usize = 5;

    #[test]
    fn toolbar_size_scales_with_actions() {
        let (width, height) = toolbar_size(TEST_ACTION_COUNT);

        assert!(width > height);
        assert_eq!(height, TOOLBAR_BUTTON_SIZE + TOOLBAR_PADDING_Y * 2.0);
    }

    #[test]
    fn toolbar_defaults_below_selection() {
        let selection = RectF::new(300.0, 200.0, 260.0, 140.0);
        let layout = resolve_toolbar_layout(selection, TEST_ACTION_COUNT, 1200.0, 800.0, &[]);

        assert_eq!(layout.y, selection.y + selection.height + SELECTION_PANEL_GAP);
    }

    #[test]
    fn toolbar_moves_above_only_when_bottom_has_no_space() {
        let selection = RectF::new(300.0, 700.0, 260.0, 80.0);
        let layout = resolve_toolbar_layout(selection, TEST_ACTION_COUNT, 1200.0, 800.0, &[]);

        assert_eq!(layout.y, selection.y - layout.height - SELECTION_PANEL_GAP);
    }

    #[test]
    fn toolbar_falls_back_inside_when_outside_sides_do_not_fit() {
        let selection = RectF::new(300.0, 24.0, 260.0, 760.0);
        let layout = resolve_toolbar_layout(selection, TEST_ACTION_COUNT, 1200.0, 800.0, &[]);

        assert_eq!(layout.y, selection.y + SELECTION_PANEL_GAP);
    }

    #[test]
    fn toolbar_clamps_horizontally_into_viewport() {
        let selection = RectF::new(760.0, 200.0, 120.0, 120.0);
        let layout = resolve_toolbar_layout(selection, TEST_ACTION_COUNT, 900.0, 700.0, &[]);

        assert!(layout.x >= VIEWPORT_MARGIN);
        assert!(layout.x + layout.width <= 900.0 - VIEWPORT_MARGIN + f64::EPSILON);
    }

    #[test]
    fn resolution_tooltip_prefers_above_selection() {
        let selection = RectF::new(120.0, 240.0, 180.0, 100.0);
        let layout = resolve_resolution_tooltip_layout_with_occupied(selection, 1200.0, 800.0, &[]);

        assert_eq!(layout.y, selection.y - layout.height - SELECTION_PANEL_GAP);
    }

    #[test]
    fn resolution_tooltip_moves_below_when_top_has_no_space() {
        let selection = RectF::new(120.0, 10.0, 180.0, 100.0);
        let layout = resolve_resolution_tooltip_layout_with_occupied(selection, 1200.0, 800.0, &[]);

        assert_eq!(layout.y, selection.y + selection.height + SELECTION_PANEL_GAP);
    }

    #[test]
    fn resolution_tooltip_avoids_reserved_toolbar_rect() {
        let selection = RectF::new(120.0, 240.0, 180.0, 100.0);
        let reserved_toolbar = OverlayPanelLayout {
            x: selection.x,
            y: selection.y - RESOLUTION_TOOLTIP_HEIGHT - SELECTION_PANEL_GAP,
            width: 120.0,
            height: RESOLUTION_TOOLTIP_HEIGHT,
        };
        let layout = resolve_resolution_tooltip_layout_with_occupied(selection, 1200.0, 800.0, &[reserved_toolbar]);

        assert_eq!(layout.y, selection.y + selection.height + SELECTION_PANEL_GAP);
        assert!(!rects_overlap(layout.as_rect(), reserved_toolbar.as_rect()));
    }

    #[test]
    fn info_tooltip_prefers_above_target() {
        let target = RectF::new(220.0, 220.0, 160.0, 120.0);
        let layout = resolve_info_tooltip_layout(target, 44.0, 1200.0, 800.0);

        assert_eq!(layout.y, target.y - layout.height - SELECTION_PANEL_GAP);
    }

    #[test]
    fn info_tooltip_moves_below_when_top_has_no_space() {
        let target = RectF::new(220.0, 8.0, 160.0, 120.0);
        let layout = resolve_info_tooltip_layout(target, 44.0, 1200.0, 800.0);

        assert_eq!(layout.y, target.y + target.height + SELECTION_PANEL_GAP);
    }

    #[test]
    fn info_tooltip_falls_back_inside_top_when_upper_space_is_larger() {
        let target = RectF::new(220.0, 24.0, 160.0, 760.0);
        let layout = resolve_info_tooltip_layout(target, 44.0, 1200.0, 800.0);

        assert_eq!(layout.y, target.y + SELECTION_PANEL_GAP);
    }

    #[test]
    fn info_tooltip_falls_back_inside_bottom_when_lower_space_is_larger() {
        let target = RectF::new(220.0, 16.0, 160.0, 760.0);
        let layout = resolve_info_tooltip_layout(target, 44.0, 1200.0, 800.0);

        assert_eq!(layout.y, target.y + target.height - layout.height - SELECTION_PANEL_GAP);
    }

    #[test]
    fn fullscreen_target_falls_back_inside_top_on_tie() {
        let target = RectF::new(0.0, 0.0, 1200.0, 800.0);
        let layout = resolve_info_tooltip_layout(target, 44.0, 1200.0, 800.0);

        assert_eq!(layout.y, VIEWPORT_MARGIN);
    }

    #[test]
    fn property_panel_prefers_toolbar_right() {
        let toolbar = OverlayPanelLayout {
            x: 360.0,
            y: 320.0,
            width: 260.0,
            height: 40.0,
        };
        let reserved = OverlayPanelLayout {
            x: 120.0,
            y: 160.0,
            width: 340.0,
            height: 44.0,
        };
        let layout = resolve_property_layout(toolbar, reserved, false, false, 1400.0, 900.0, &[toolbar]);

        assert_eq!(layout.x, toolbar.x + toolbar.width + SELECTION_PANEL_GAP);
        assert_eq!(layout.y, toolbar.y);
        assert!(!rects_overlap(layout.as_rect(), reserved.as_rect()));
    }

    #[test]
    fn property_panel_follows_toolbar_fallback_order() {
        let toolbar = OverlayPanelLayout {
            x: 360.0,
            y: 320.0,
            width: 260.0,
            height: 40.0,
        };
        let reserved = OverlayPanelLayout {
            x: 20.0,
            y: 20.0,
            width: 1.0,
            height: 1.0,
        };
        let (panel_w, panel_h) = property_panel_size(false, false);
        let right_rect = OverlayPanelLayout {
            x: toolbar.x + toolbar.width + SELECTION_PANEL_GAP,
            y: toolbar.y,
            width: panel_w,
            height: toolbar.height,
        };
        let below_rect = OverlayPanelLayout {
            x: toolbar.x,
            y: toolbar.y + toolbar.height + SELECTION_PANEL_GAP,
            width: toolbar.width,
            height: toolbar.height,
        };
        let above_rect = OverlayPanelLayout {
            x: toolbar.x,
            y: toolbar.y - panel_h - SELECTION_PANEL_GAP,
            width: panel_w,
            height: toolbar.height,
        };

        let right = resolve_property_layout(toolbar, reserved, false, false, 1400.0, 900.0, &[toolbar]);
        assert_eq!(right.x, right_rect.x);
        assert_eq!(right.y, right_rect.y);

        let below = resolve_property_layout(toolbar, reserved, false, false, 1400.0, 900.0, &[toolbar, right_rect]);
        assert_eq!(below.x, below_rect.x);
        assert_eq!(below.y, below_rect.y);

        let above = resolve_property_layout(toolbar, reserved, false, false, 1400.0, 900.0, &[toolbar, right_rect, below_rect]);
        assert_eq!(above.x, above_rect.x);
        assert_eq!(above.y, above_rect.y);

        let left = resolve_property_layout(
            toolbar,
            reserved,
            false,
            false,
            1400.0,
            900.0,
            &[toolbar, right_rect, below_rect, above_rect],
        );
        assert_eq!(left.x, toolbar.x - panel_w - SELECTION_PANEL_GAP);
        assert_eq!(left.y, toolbar.y);
    }

    #[test]
    fn property_panel_size_grows_when_mode_section_is_visible() {
        let base = property_panel_size(false, false);
        let text_mode = property_panel_size(true, false);
        let mosaic_mode = property_panel_size(false, true);
        let all_mode = property_panel_size(true, true);

        assert_eq!(base.0, text_mode.0);
        assert_eq!(base.0, mosaic_mode.0);
        assert_eq!(base.0, all_mode.0);
        assert!(text_mode.1 > base.1);
        assert_eq!(text_mode.1, mosaic_mode.1);
        assert_eq!(mosaic_mode.1, all_mode.1);
    }

    #[test]
    fn all_panel_layouts_stay_inside_viewport() {
        let selection = RectF::new(1180.0, 760.0, 80.0, 60.0);
        let toolbar = resolve_toolbar_layout(selection, TEST_ACTION_COUNT, 1200.0, 800.0, &[]);
        let reserved = resolve_info_reserved_slot_layout(selection, 1200.0, 800.0);
        let property = resolve_property_layout(toolbar, reserved, true, true, 1200.0, 800.0, &[toolbar]);
        let resolution = resolve_resolution_tooltip_layout_with_occupied(selection, 1200.0, 800.0, &[]);
        let info = resolve_info_tooltip_layout(selection, 44.0, 1200.0, 800.0);

        for layout in [toolbar, property, resolution, info] {
            assert!(layout.x >= VIEWPORT_MARGIN);
            assert!(layout.y >= VIEWPORT_MARGIN);
            assert!(layout.x + layout.width <= 1200.0 - VIEWPORT_MARGIN + f64::EPSILON);
            assert!(layout.y + layout.height <= 800.0 - VIEWPORT_MARGIN + f64::EPSILON);
        }
    }
}

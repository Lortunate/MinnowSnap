use crate::core::geometry::RectF;

const VIEWPORT_MARGIN: f64 = 16.0;
pub(crate) const SELECTION_PANEL_GAP: f64 = 8.0;
const TOOLBAR_BUTTON_SIZE: f64 = 32.0;
const TOOLBAR_BUTTON_GAP: f64 = 2.0;
const TOOLBAR_PADDING_X: f64 = 8.0;
const TOOLBAR_PADDING_Y: f64 = 4.0;
const WINDOW_INFO_MAX_WIDTH: f64 = 340.0;
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

pub(crate) fn toolbar_size(action_count: usize) -> (f64, f64) {
    let button_count = action_count.max(1) as f64;
    let width = button_count * TOOLBAR_BUTTON_SIZE + (button_count - 1.0) * TOOLBAR_BUTTON_GAP + TOOLBAR_PADDING_X * 2.0;
    let height = TOOLBAR_BUTTON_SIZE + TOOLBAR_PADDING_Y * 2.0;
    (width, height)
}

fn resolution_tooltip_size(selection: RectF) -> (f64, f64) {
    let label = format!("{:.0} × {:.0}", selection.width.max(0.0), selection.height.max(0.0));
    let width = (label.chars().count() as f64 * TOOLTIP_CHARACTER_WIDTH + TOOLTIP_HORIZONTAL_PADDING).max(RESOLUTION_TOOLTIP_MIN_WIDTH);
    (width, RESOLUTION_TOOLTIP_HEIGHT)
}

fn resolve_panel_layout(
    target: RectF,
    desired_x: f64,
    width: f64,
    height: f64,
    viewport_w: f64,
    viewport_h: f64,
    preferred_side: VerticalSide,
    occupied: &[OverlayPanelLayout],
) -> OverlayPanelLayout {
    let opposite_side = match preferred_side {
        VerticalSide::Above => VerticalSide::Below,
        VerticalSide::Below => VerticalSide::Above,
    };

    let inside_side = preferred_inside_side(target, viewport_h);
    let mut first_colliding = None;

    for candidate_y in [
        outside_position(target, height, viewport_h, preferred_side),
        outside_position(target, height, viewport_h, opposite_side),
        Some(inside_position(target, height, inside_side)),
    ]
    .into_iter()
    .flatten()
    {
        let candidate = clamp_layout(desired_x, candidate_y, width, height, viewport_w, viewport_h);
        if !overlaps_any(candidate, occupied) {
            return candidate;
        }
        first_colliding.get_or_insert(candidate);
    }

    first_colliding.unwrap_or_else(|| {
        clamp_layout(
            desired_x,
            inside_position(target, height, inside_side),
            width,
            height,
            viewport_w,
            viewport_h,
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
    resolve_panel_layout(
        selection,
        selection.x,
        tooltip_w,
        tooltip_h,
        viewport_w,
        viewport_h,
        VerticalSide::Above,
        occupied,
    )
}

pub(crate) fn resolve_info_tooltip_layout(target: RectF, tooltip_height: f64, viewport_w: f64, viewport_h: f64) -> OverlayPanelLayout {
    resolve_panel_layout(
        target,
        target.x,
        WINDOW_INFO_MAX_WIDTH,
        tooltip_height,
        viewport_w,
        viewport_h,
        VerticalSide::Above,
        &[],
    )
}

pub(crate) fn resolve_toolbar_layout(
    selection: RectF,
    action_count: usize,
    viewport_w: f64,
    viewport_h: f64,
    occupied: &[OverlayPanelLayout],
) -> OverlayPanelLayout {
    let (toolbar_w, toolbar_h) = toolbar_size(action_count);
    resolve_panel_layout(
        selection,
        selection.x + selection.width - toolbar_w,
        toolbar_w,
        toolbar_h,
        viewport_w,
        viewport_h,
        VerticalSide::Below,
        occupied,
    )
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
    fn all_panel_layouts_stay_inside_viewport() {
        let selection = RectF::new(1180.0, 760.0, 80.0, 60.0);
        let toolbar = resolve_toolbar_layout(selection, TEST_ACTION_COUNT, 1200.0, 800.0, &[]);
        let resolution = resolve_resolution_tooltip_layout_with_occupied(selection, 1200.0, 800.0, &[]);
        let info = resolve_info_tooltip_layout(selection, 44.0, 1200.0, 800.0);

        for layout in [toolbar, resolution, info] {
            assert!(layout.x >= VIEWPORT_MARGIN);
            assert!(layout.y >= VIEWPORT_MARGIN);
            assert!(layout.x + layout.width <= 1200.0 - VIEWPORT_MARGIN + f64::EPSILON);
            assert!(layout.y + layout.height <= 800.0 - VIEWPORT_MARGIN + f64::EPSILON);
        }
    }
}

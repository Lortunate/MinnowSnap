use crate::services::geometry::RectF;

pub(crate) const VIEWPORT_MARGIN: f64 = 16.0;
pub(crate) const SELECTION_PANEL_GAP: f64 = 8.0;

const TOOLBAR_BUTTON_SIZE: f64 = 32.0;
const TOOLBAR_BUTTON_GAP: f64 = 2.0;
const TOOLBAR_PADDING_X: f64 = 8.0;
const TOOLBAR_PADDING_Y: f64 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PanelLayout {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl PanelLayout {
    pub(crate) fn as_rect(self) -> RectF {
        RectF::new(self.x, self.y, self.width, self.height)
    }
}

pub(crate) fn toolbar_size(action_count: usize) -> (f64, f64) {
    let button_count = action_count.max(1) as f64;
    let width = button_count * TOOLBAR_BUTTON_SIZE + (button_count - 1.0) * TOOLBAR_BUTTON_GAP + TOOLBAR_PADDING_X * 2.0;
    let height = TOOLBAR_BUTTON_SIZE + TOOLBAR_PADDING_Y * 2.0;
    (width, height)
}

pub(crate) fn resolve_toolbar_layout(
    selection: RectF,
    action_count: usize,
    viewport_w: f64,
    viewport_h: f64,
    occupied: &[PanelLayout],
) -> PanelLayout {
    let (width, height) = toolbar_size(action_count);
    let desired_x = selection.x + selection.width - width;

    let mut first_colliding = None;
    for candidate_y in [
        outside_position(selection, height, viewport_h, VerticalSide::Below),
        outside_position(selection, height, viewport_h, VerticalSide::Above),
        Some(inside_position(selection, height, preferred_inside_side(selection, viewport_h))),
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
            inside_position(selection, height, preferred_inside_side(selection, viewport_h)),
            width,
            height,
            viewport_w,
            viewport_h,
        )
    })
}

pub(crate) fn rects_overlap(a: RectF, b: RectF) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

pub(crate) fn clamp_layout(x: f64, y: f64, width: f64, height: f64, viewport_w: f64, viewport_h: f64) -> PanelLayout {
    let max_x = (viewport_w - width - VIEWPORT_MARGIN).max(VIEWPORT_MARGIN);
    let max_y = (viewport_h - height - VIEWPORT_MARGIN).max(VIEWPORT_MARGIN);
    PanelLayout {
        x: x.clamp(VIEWPORT_MARGIN, max_x),
        y: y.clamp(VIEWPORT_MARGIN, max_y),
        width,
        height,
    }
}

#[derive(Clone, Copy)]
enum VerticalSide {
    Above,
    Below,
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

fn overlaps_any(candidate: PanelLayout, occupied: &[PanelLayout]) -> bool {
    occupied.iter().copied().any(|other| rects_overlap(candidate.as_rect(), other.as_rect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTION_COUNT: usize = 5;

    #[test]
    fn toolbar_size_scales_with_actions() {
        let (width, height) = toolbar_size(ACTION_COUNT);
        assert!(width > height);
        assert_eq!(height, TOOLBAR_BUTTON_SIZE + TOOLBAR_PADDING_Y * 2.0);
    }

    #[test]
    fn toolbar_prefers_below_selection() {
        let selection = RectF::new(300.0, 200.0, 260.0, 140.0);
        let layout = resolve_toolbar_layout(selection, ACTION_COUNT, 1200.0, 800.0, &[]);
        assert_eq!(layout.y, selection.y + selection.height + SELECTION_PANEL_GAP);
    }

    #[test]
    fn toolbar_moves_above_when_bottom_has_no_space() {
        let selection = RectF::new(300.0, 700.0, 260.0, 80.0);
        let layout = resolve_toolbar_layout(selection, ACTION_COUNT, 1200.0, 800.0, &[]);
        assert_eq!(layout.y, selection.y - layout.height - SELECTION_PANEL_GAP);
    }

    #[test]
    fn toolbar_falls_back_inside_when_outside_sides_do_not_fit() {
        let selection = RectF::new(300.0, 24.0, 260.0, 760.0);
        let layout = resolve_toolbar_layout(selection, ACTION_COUNT, 1200.0, 800.0, &[]);
        assert_eq!(layout.y, selection.y + SELECTION_PANEL_GAP);
    }

    #[test]
    fn toolbar_avoids_occupied_slot() {
        let selection = RectF::new(300.0, 200.0, 260.0, 140.0);
        let (width, height) = toolbar_size(ACTION_COUNT);
        let occupied = [PanelLayout {
            x: selection.x + selection.width - width,
            y: selection.y + selection.height + SELECTION_PANEL_GAP,
            width,
            height,
        }];
        let layout = resolve_toolbar_layout(selection, ACTION_COUNT, 1200.0, 800.0, &occupied);
        assert!(!rects_overlap(layout.as_rect(), occupied[0].as_rect()));
    }
}

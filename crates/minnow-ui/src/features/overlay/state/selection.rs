use gpui::{Pixels, Point};

use crate::features::overlay::window_catalog::find_window_at;
use minnow_imaging::geometry::{RectF, clamp_rect_resize};

use super::{DragMode, OverlaySession, ResizeCorner};

const TARGET_CONFIRM_DRAG_THRESHOLD: f64 = 1.0;
// Resizing keeps the existing 10-point minimum, independently of the 8-point
// threshold for confirming a newly drawn selection.
const MIN_RESIZE_SIZE: f64 = 10.0;

fn resize_selection(start: RectF, corner: ResizeCorner, delta: (f64, f64), viewport: (f64, f64)) -> RectF {
    let mut x = start.x;
    let mut y = start.y;
    let mut width = start.width;
    let mut height = start.height;
    let left = matches!(corner, ResizeCorner::TopLeft | ResizeCorner::BottomLeft | ResizeCorner::Left);
    let right = matches!(corner, ResizeCorner::TopRight | ResizeCorner::BottomRight | ResizeCorner::Right);
    let top = matches!(corner, ResizeCorner::TopLeft | ResizeCorner::TopRight | ResizeCorner::Top);
    let bottom = matches!(corner, ResizeCorner::BottomLeft | ResizeCorner::BottomRight | ResizeCorner::Bottom);

    if right {
        width += delta.0;
    } else if left {
        x += delta.0;
        width -= delta.0;
    }
    if bottom {
        height += delta.1;
    } else if top {
        y += delta.1;
        height -= delta.1;
    }
    if width < MIN_RESIZE_SIZE {
        if left {
            x = start.x + start.width - MIN_RESIZE_SIZE;
        }
        width = MIN_RESIZE_SIZE;
    }
    if height < MIN_RESIZE_SIZE {
        if top {
            y = start.y + start.height - MIN_RESIZE_SIZE;
        }
        height = MIN_RESIZE_SIZE;
    }

    let (x, y, width, height) = clamp_rect_resize(x, y, width, height, viewport.0, viewport.1);
    RectF::new(x, y, width, height)
}

impl OverlaySession {
    pub(crate) fn start_selection(&mut self, point: Point<Pixels>) {
        self.viewport.selection_move_delta = None;
        self.viewport.selection_move_origin = None;
        let (x, y) = self.clamp_point_to_viewport(point);
        self.viewport.mode = DragMode::Selecting;
        self.viewport.drag_start = Some(point);
        self.viewport.drag_start_rect = None;
        self.viewport.confirm_target_on_release = self
            .viewport
            .target
            .map(|target| x >= target.x && x <= target.x + target.width && y >= target.y && y <= target.y + target.height)
            .unwrap_or(false);
        self.viewport.selection = Some(RectF::new(x, y, 0.0, 0.0));
    }

    pub(crate) fn update_selection(&mut self, point: Point<Pixels>) {
        if self.viewport.mode != DragMode::Selecting {
            return;
        }

        let Some(start) = self.viewport.drag_start else {
            return;
        };

        let start_x = start.x.to_f64();
        let start_y = start.y.to_f64();
        let (current_x, current_y) = self.clamp_point_to_viewport(point);

        let x = start_x.min(current_x);
        let y = start_y.min(current_y);
        let width = (current_x - start_x).abs();
        let height = (current_y - start_y).abs();
        if width > TARGET_CONFIRM_DRAG_THRESHOLD || height > TARGET_CONFIRM_DRAG_THRESHOLD {
            self.viewport.confirm_target_on_release = false;
            self.viewport.target = None;
            self.hovered_window = None;
        }

        self.viewport.selection = Some(self.clamp_rect_to_viewport(RectF::new(x, y, width, height)));
    }

    pub(crate) fn finish_selection(&mut self) {
        if self.viewport.mode != DragMode::Selecting {
            return;
        }

        self.viewport.drag_start = None;
        self.viewport.drag_start_rect = None;

        if self.viewport.confirm_target_on_release
            && let Some(target) = self.viewport.target
        {
            self.viewport.mode = DragMode::Idle;
            self.viewport.selection = Some(target);
            let center_x = target.x + target.width / 2.0;
            let center_y = target.y + target.height / 2.0;
            self.hovered_window = find_window_at(&self.windows, center_x, center_y).map(|idx| self.windows[idx].clone());
            self.viewport.target = None;
            self.viewport.confirm_target_on_release = false;
            return;
        }

        if let Some(selection) = self.viewport.selection
            && selection.width >= Self::MIN_SELECTION_SIZE
            && selection.height >= Self::MIN_SELECTION_SIZE
        {
            self.viewport.mode = DragMode::Idle;
            self.viewport.target = None;
            self.viewport.confirm_target_on_release = false;
            self.hovered_window = None;
            return;
        }

        self.viewport.confirm_target_on_release = false;
        self.clear();
    }

    pub(crate) fn start_resize(&mut self, corner: ResizeCorner, point: Point<Pixels>) {
        let Some(selection) = self.viewport.selection else {
            return;
        };

        self.viewport.selection_move_delta = None;
        self.viewport.selection_move_origin = None;
        self.viewport.mode = DragMode::Resizing(corner);
        self.viewport.drag_start = Some(point);
        self.viewport.drag_start_rect = Some(selection);
        self.viewport.confirm_target_on_release = false;
    }

    pub(crate) fn update_resize(&mut self, point: Point<Pixels>) {
        let DragMode::Resizing(corner) = self.viewport.mode else {
            return;
        };

        let Some(start) = self.viewport.drag_start else {
            return;
        };
        let Some(start_rect) = self.viewport.drag_start_rect else {
            return;
        };

        let (current_x, current_y) = self.clamp_point_to_viewport(point);
        let dx = current_x - start.x.to_f64();
        let dy = current_y - start.y.to_f64();
        let resized = resize_selection(start_rect, corner, (dx, dy), (self.viewport.viewport_w, self.viewport.viewport_h));
        self.viewport.selection = Some(self.clamp_rect_to_viewport(resized));
    }

    pub(crate) fn finish_resize(&mut self) {
        if matches!(self.viewport.mode, DragMode::Resizing(_)) {
            self.viewport.mode = DragMode::Idle;
            self.viewport.drag_start = None;
            self.viewport.drag_start_rect = None;
        }
    }

    pub(crate) fn start_move(&mut self, point: Point<Pixels>) {
        let Some(selection) = self.viewport.selection else {
            return;
        };

        self.viewport.mode = DragMode::Idle;
        self.viewport.drag_start = Some(point);
        self.viewport.drag_start_rect = Some(selection);
        self.viewport.selection_move_origin = Some(selection);
        self.viewport.selection_move_delta = Some((0.0, 0.0));
        self.viewport.confirm_target_on_release = false;
    }

    pub(crate) fn update_move(&mut self, point: Point<Pixels>) {
        if self.viewport.selection_move_origin.is_none() {
            return;
        }

        let Some(start) = self.viewport.drag_start else {
            return;
        };
        let Some(start_rect) = self.viewport.drag_start_rect else {
            return;
        };

        let (current_x, current_y) = self.clamp_point_to_viewport(point);
        let dx = current_x - start.x.to_f64();
        let dy = current_y - start.y.to_f64();

        let width = start_rect.width.max(0.0);
        let height = start_rect.height.max(0.0);
        let max_x = (self.viewport.viewport_w - width).max(0.0);
        let max_y = (self.viewport.viewport_h - height).max(0.0);
        let next_x = (start_rect.x + dx).clamp(0.0, max_x);
        let next_y = (start_rect.y + dy).clamp(0.0, max_y);
        let next = RectF::new(next_x, next_y, width, height);

        let origin = self.viewport.selection_move_origin.unwrap_or(start_rect);
        self.viewport.selection = Some(next);
        self.viewport.selection_move_delta = Some((next.x - origin.x, next.y - origin.y));
    }

    pub(crate) fn finish_move(&mut self) -> bool {
        let Some(origin) = self.viewport.selection_move_origin else {
            return false;
        };
        let Some(selection) = self.viewport.selection else {
            self.viewport.selection_move_origin = None;
            self.viewport.selection_move_delta = None;
            self.viewport.drag_start = None;
            self.viewport.drag_start_rect = None;
            return false;
        };

        // Delta is defined in selection-space, so compute from the stored origin to keep
        // behavior consistent even if clamping changed the effective drag distance.
        let dx = selection.x - origin.x;
        let dy = selection.y - origin.y;

        let changed = if dx.abs() <= f64::EPSILON && dy.abs() <= f64::EPSILON {
            false
        } else {
            self.annotation.translate_all_annotations(dx, dy)
        };

        self.viewport.selection_move_origin = None;
        self.viewport.selection_move_delta = None;
        self.viewport.drag_start = None;
        self.viewport.drag_start_rect = None;
        changed
    }

    pub(crate) fn clear(&mut self) {
        self.reset_interaction_state();
        self.clear_annotation_state();
        self.refresh_picker_sample();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::overlay::annotation::AnnotationTool;
    use crate::features::overlay::state::{LifecycleCommand, OverlayCommand, OverlaySession};

    fn session() -> OverlaySession {
        let mut session = OverlaySession::default();
        session.set_viewport_size(200.0, 100.0);
        session
    }

    #[test]
    fn selection_locks_after_drag() {
        let mut session = session();
        session.start_selection(Point::new(gpui::px(10.0), gpui::px(20.0)));
        session.update_selection(Point::new(gpui::px(80.0), gpui::px(60.0)));
        session.finish_selection();

        assert_eq!(session.mode(), DragMode::Idle);
        assert_eq!(session.selection(), Some(RectF::new(10.0, 20.0, 70.0, 40.0)));
    }

    #[test]
    fn resize_updates_selection() {
        let mut session = session();
        session.viewport.selection = Some(RectF::new(20.0, 20.0, 40.0, 20.0));
        session.start_resize(ResizeCorner::BottomRight, Point::new(gpui::px(60.0), gpui::px(40.0)));
        session.update_resize(Point::new(gpui::px(80.0), gpui::px(70.0)));
        session.finish_resize();

        assert_eq!(session.selection(), Some(RectF::new(20.0, 20.0, 60.0, 50.0)));
        assert_eq!(session.mode(), DragMode::Idle);
    }

    #[test]
    fn resize_handles_preserve_opposite_edges_and_clip_to_viewport() {
        let start = RectF::new(20.0, 20.0, 40.0, 20.0);
        let viewport = (200.0, 100.0);
        for (corner, delta, expected) in [
            (ResizeCorner::TopLeft, (100.0, 100.0), RectF::new(50.0, 30.0, 10.0, 10.0)),
            (ResizeCorner::TopRight, (-100.0, 100.0), RectF::new(20.0, 30.0, 10.0, 10.0)),
            (ResizeCorner::BottomLeft, (100.0, -100.0), RectF::new(50.0, 20.0, 10.0, 10.0)),
            (ResizeCorner::BottomRight, (-100.0, -100.0), RectF::new(20.0, 20.0, 10.0, 10.0)),
            (ResizeCorner::Left, (100.0, 100.0), RectF::new(50.0, 20.0, 10.0, 20.0)),
            (ResizeCorner::Right, (-100.0, 100.0), RectF::new(20.0, 20.0, 10.0, 20.0)),
            (ResizeCorner::Top, (100.0, 100.0), RectF::new(20.0, 30.0, 40.0, 10.0)),
            (ResizeCorner::Bottom, (100.0, -100.0), RectF::new(20.0, 20.0, 40.0, 10.0)),
            (ResizeCorner::TopLeft, (-100.0, -100.0), RectF::new(0.0, 0.0, 60.0, 40.0)),
            (ResizeCorner::BottomRight, (500.0, 500.0), RectF::new(20.0, 20.0, 180.0, 80.0)),
        ] {
            assert_eq!(resize_selection(start, corner, delta, viewport), expected, "{corner:?}");
        }
        // Viewport clipping still takes priority over the resize minimum.
        assert_eq!(
            resize_selection(RectF::new(195.0, 95.0, 5.0, 5.0), ResizeCorner::BottomRight, (0.0, 0.0), viewport),
            RectF::new(195.0, 95.0, 5.0, 5.0)
        );
    }

    #[test]
    fn confirming_and_resizing_keep_distinct_minimum_sizes() {
        let mut session = session();
        session.start_selection(Point::new(gpui::px(10.0), gpui::px(20.0)));
        session.update_selection(Point::new(gpui::px(18.0), gpui::px(28.0)));
        session.finish_selection();
        assert_eq!(session.selection(), Some(RectF::new(10.0, 20.0, 8.0, 8.0)));

        session.start_resize(ResizeCorner::BottomRight, Point::new(gpui::px(18.0), gpui::px(28.0)));
        session.update_resize(Point::new(gpui::px(18.0), gpui::px(28.0)));
        session.finish_resize();
        assert_eq!(session.selection(), Some(RectF::new(10.0, 20.0, 10.0, 10.0)));
    }

    #[test]
    fn selection_move_translates_annotations_on_release() {
        let mut session = session();
        session.viewport.selection = Some(RectF::new(20.0, 20.0, 80.0, 40.0));
        session.set_annotation_tool(AnnotationTool::Rectangle);
        assert!(session.start_annotation_draw(Point::new(gpui::px(30.0), gpui::px(30.0))));
        assert!(session.update_annotation_interaction(Point::new(gpui::px(60.0), gpui::px(60.0))));
        assert!(session.finish_annotation_interaction());

        let before = session.selected_annotation_item().unwrap().bounds();
        let dx: f64 = 30.0;
        let dy: f64 = 40.0;

        // Simulate a selection move drag followed by pointer release.
        session.start_move(Point::new(gpui::px(30.0), gpui::px(30.0)));
        session.update_move(Point::new(gpui::px((30.0 + dx) as f32), gpui::px((30.0 + dy) as f32)));
        session.apply(OverlayCommand::Lifecycle(LifecycleCommand::PointerReleased));

        let after = session.selected_annotation_item().unwrap().bounds();

        assert_eq!(after.x, before.x + dx);
        assert_eq!(after.y, before.y + dy);
        assert_eq!(after.width, before.width);
        assert_eq!(after.height, before.height);
    }

    #[test]
    fn selection_move_translation_undo_redo_preserves_annotation() {
        let mut session = session();
        session.viewport.selection = Some(RectF::new(20.0, 20.0, 80.0, 40.0));
        session.set_annotation_tool(AnnotationTool::Rectangle);
        assert!(session.start_annotation_draw(Point::new(gpui::px(30.0), gpui::px(30.0))));
        assert!(session.update_annotation_interaction(Point::new(gpui::px(60.0), gpui::px(60.0))));
        assert!(session.finish_annotation_interaction());
        assert!(session.start_annotation_draw(Point::new(gpui::px(65.0), gpui::px(35.0))));
        assert!(session.update_annotation_interaction(Point::new(gpui::px(90.0), gpui::px(55.0))));
        assert!(session.finish_annotation_interaction());

        let before = session
            .annotation_ui_state()
            .layer
            .outlines
            .iter()
            .map(|outline| (outline.id, outline.bounds))
            .collect::<Vec<_>>();

        session.start_move(Point::new(gpui::px(30.0), gpui::px(30.0)));
        session.update_move(Point::new(gpui::px(60.0), gpui::px(70.0)));
        session.apply(OverlayCommand::Lifecycle(LifecycleCommand::PointerReleased));

        let moved = session
            .annotation_ui_state()
            .layer
            .outlines
            .iter()
            .map(|outline| (outline.id, outline.bounds))
            .collect::<Vec<_>>();
        assert_ne!(moved, before);

        assert!(session.undo_annotation());
        let undone = session
            .annotation_ui_state()
            .layer
            .outlines
            .iter()
            .map(|outline| (outline.id, outline.bounds))
            .collect::<Vec<_>>();
        assert_eq!(undone, before);

        assert!(session.redo_annotation());
        let redone = session
            .annotation_ui_state()
            .layer
            .outlines
            .iter()
            .map(|outline| (outline.id, outline.bounds))
            .collect::<Vec<_>>();
        assert_eq!(redone, moved);
    }

    #[test]
    fn selection_move_clamps_to_viewport_edge_without_resizing() {
        let mut session = session();
        session.viewport.selection = Some(RectF::new(80.0, 30.0, 80.0, 50.0));

        session.start_move(Point::new(gpui::px(100.0), gpui::px(40.0)));
        session.update_move(Point::new(gpui::px(190.0), gpui::px(100.0)));

        assert_eq!(session.selection(), Some(RectF::new(120.0, 50.0, 80.0, 50.0)));
    }
}

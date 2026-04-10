use gpui::{Pixels, Point};

use crate::features::overlay::window_catalog::find_window_at;
use minnow_core::geometry::RectF;

use super::{DragMode, OverlaySession, ResizeCorner};

impl OverlaySession {
    pub(crate) fn start_selection(&mut self, point: Point<Pixels>) {
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
        if width > 1.0 || height > 1.0 {
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
        let (x, y, width, height) = minnow_core::geometry::calculate_resize(
            start_rect.x,
            start_rect.y,
            start_rect.width,
            start_rect.height,
            dx,
            dy,
            corner.geometry_key(),
            self.viewport.viewport_w,
            self.viewport.viewport_h,
        );

        self.viewport.selection = Some(self.clamp_rect_to_viewport(RectF::new(x, y, width, height)));
    }

    pub(crate) fn finish_resize(&mut self) {
        if matches!(self.viewport.mode, DragMode::Resizing(_)) {
            self.viewport.mode = DragMode::Idle;
            self.viewport.drag_start = None;
            self.viewport.drag_start_rect = None;
        }
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
    use crate::features::overlay::state::OverlaySession;

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
}

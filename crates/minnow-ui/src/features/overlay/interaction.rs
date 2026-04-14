use gpui::{MouseButton, Pixels, Point};

use crate::features::overlay::state::{AnnotationCommand, DragMode, LifecycleCommand, OverlayCommand, OverlaySession, ResizeCorner};
use minnow_core::geometry::RectF;

pub(crate) fn resolve_mouse_down_command(
    session: &OverlaySession,
    button: MouseButton,
    point: Point<Pixels>,
    click_count: usize,
) -> Option<OverlayCommand> {
    match button {
        MouseButton::Right => Some(resolve_right_click_command(session)),
        MouseButton::Left => resolve_left_click_command(session, point, click_count),
        _ => None,
    }
}

fn resolve_right_click_command(session: &OverlaySession) -> OverlayCommand {
    if session.has_selection() {
        OverlayCommand::Lifecycle(LifecycleCommand::ClearSelection)
    } else {
        OverlayCommand::Lifecycle(LifecycleCommand::CloseIntent)
    }
}

fn resolve_left_click_command(session: &OverlaySession, point: Point<Pixels>, click_count: usize) -> Option<OverlayCommand> {
    if let Some(selection) = session.selection() {
        if let Some(corner) = hit_resize_corner(selection, point) {
            return Some(OverlayCommand::Lifecycle(LifecycleCommand::StartResize { corner, point }));
        }

        if click_count >= 2 {
            return Some(OverlayCommand::Annotation(AnnotationCommand::StartTextEditAtPoint(point)));
        }

        if matches!(session.mode(), DragMode::Idle) {
            if let Some(id) = session.annotation_hit_test(point) {
                return Some(OverlayCommand::Annotation(AnnotationCommand::StartMove { id, point }));
            }

            if point_in_rect(point, selection) {
                if session.has_active_annotation_tool() {
                    return Some(OverlayCommand::Annotation(AnnotationCommand::StartDraw(point)));
                }
                return Some(OverlayCommand::Lifecycle(LifecycleCommand::StartMove(point)));
            }

            return Some(OverlayCommand::Annotation(AnnotationCommand::Select(None)));
        }
    }

    Some(OverlayCommand::Lifecycle(LifecycleCommand::StartSelection(point)))
}

fn point_in_rect(point: Point<Pixels>, rect: RectF) -> bool {
    let x = point.x.to_f64();
    let y = point.y.to_f64();
    rect.contains_point(x, y)
}

fn hit_resize_corner(selection: RectF, point: Point<Pixels>) -> Option<ResizeCorner> {
    let x = point.x.to_f64();
    let y = point.y.to_f64();
    let left = selection.x;
    let top = selection.y;
    let right = selection.x + selection.width;
    let bottom = selection.y + selection.height;
    let hit = 12.0;

    let near_left = (x - left).abs() <= hit;
    let near_right = (x - right).abs() <= hit;
    let near_top = (y - top).abs() <= hit;
    let near_bottom = (y - bottom).abs() <= hit;

    match (near_left, near_right, near_top, near_bottom) {
        (true, false, true, false) => Some(ResizeCorner::TopLeft),
        (false, true, true, false) => Some(ResizeCorner::TopRight),
        (true, false, false, true) => Some(ResizeCorner::BottomLeft),
        (false, true, false, true) => Some(ResizeCorner::BottomRight),
        (true, false, false, false) => Some(ResizeCorner::Left),
        (false, true, false, false) => Some(ResizeCorner::Right),
        (false, false, true, false) => Some(ResizeCorner::Top),
        (false, false, false, true) => Some(ResizeCorner::Bottom),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_mouse_down_command;
    use crate::features::overlay::state::{DragMode, LifecycleCommand, OverlayCommand, OverlaySession};
    use gpui::{MouseButton, Point, px};

    #[test]
    fn inside_selection_without_tool_starts_selection_move() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(300.0, 200.0);
        session.start_selection(Point::new(px(20.0), px(20.0)));
        session.update_selection(Point::new(px(80.0), px(70.0)));
        session.finish_selection();

        assert!(matches!(session.mode(), DragMode::Idle));

        let command = resolve_mouse_down_command(
            &session,
            MouseButton::Left,
            Point::new(px(40.0), px(40.0)),
            1,
        );

        assert_eq!(
            command,
            Some(OverlayCommand::Lifecycle(LifecycleCommand::StartMove(Point::new(
                px(40.0),
                px(40.0),
            ))))
        );
    }
}

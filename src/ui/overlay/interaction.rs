use gpui::{MouseButton, Pixels, Point};

use crate::core::geometry::RectF;
use crate::ui::overlay::session::{DragMode, OverlayCommand, OverlaySession, ResizeCorner};

pub(crate) fn resolve_mouse_down_command(session: &OverlaySession, button: MouseButton, point: Point<Pixels>) -> Option<OverlayCommand> {
    match button {
        MouseButton::Right => Some(resolve_right_click_command(session)),
        MouseButton::Left => Some(resolve_left_click_command(session, point)),
        _ => None,
    }
}

fn resolve_right_click_command(session: &OverlaySession) -> OverlayCommand {
    if session.has_selection() {
        OverlayCommand::ClearSelection
    } else {
        OverlayCommand::Close
    }
}

fn resolve_left_click_command(session: &OverlaySession, point: Point<Pixels>) -> OverlayCommand {
    if let Some(selection) = session.selection() {
        if let Some(corner) = hit_resize_corner(selection, point) {
            return OverlayCommand::StartResize { corner, point };
        }

        if point_in_rect(point, selection) && matches!(session.mode(), DragMode::Idle) {
            return OverlayCommand::StartMove(point);
        }
    }

    OverlayCommand::StartSelection(point)
}

fn point_in_rect(point: Point<Pixels>, rect: RectF) -> bool {
    let x = point.x.to_f64();
    let y = point.y.to_f64();
    x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height
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

use super::OverlayView;
use crate::features::overlay::interaction::resolve_mouse_down_command;
use crate::features::overlay::state::{AnnotationCommand, LifecycleCommand};
use gpui::{Context, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollWheelEvent, Window, px};

impl OverlayView {
    pub(super) fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.handle.session();
        let next_command = {
            let state = session.read(cx);
            resolve_mouse_down_command(state, event.button, event.position, event.click_count)
        };
        if let Some(command) = next_command {
            self.dispatch_command(command, window, cx);
        }
    }

    pub(super) fn on_mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_lifecycle(LifecycleCommand::PointerMoved(event.position), window, cx);
    }

    pub(super) fn on_mouse_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.button == MouseButton::Left {
            self.dispatch_lifecycle(LifecycleCommand::PointerReleased, window, cx);
        }
    }

    pub(super) fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.is_held {
            return;
        }

        let key = event.keystroke.key.as_str();
        if key == "enter" && event.keystroke.modifiers.shift {
            self.dispatch_annotation(AnnotationCommand::InsertTextNewline, window, cx);
            return;
        }

        if event.keystroke.modifiers.control
            || event.keystroke.modifiers.alt
            || event.keystroke.modifiers.platform
            || event.keystroke.modifiers.function
        {
            return;
        }

        let text = if key == "space" {
            Some(" ".to_string())
        } else if key.chars().count() == 1 {
            Some(key.to_string())
        } else {
            None
        };
        if let Some(text) = text {
            self.dispatch_annotation(AnnotationCommand::AppendText { text }, window, cx);
        }
    }

    pub(super) fn on_scroll_wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta_y = event.delta.pixel_delta(px(24.0)).y.to_f64();
        if delta_y.abs() <= f64::EPSILON {
            return;
        }
        let delta = if delta_y > 0.0 { 1.0 } else { -1.0 };
        self.dispatch_annotation(
            AnnotationCommand::AdjustByWheel {
                point: event.position,
                delta,
            },
            window,
            cx,
        );
    }
}

mod actions;
mod input;
mod render;

use crate::core::capture::action::CaptureAction;
use crate::ui::overlay::annotation::COLOR_PRESETS;
use crate::ui::overlay::session::{
    AnnotationCommand, AnnotationKindTag, AnnotationTool, CaptureCommand, DragMode, LifecycleCommand, OverlayCommand, OverlayHandle, PickerCommand,
};
use gpui::{AppContext, Context, Div, Entity, FocusHandle, Hsla, Rgba, Styled, Subscription, Window, div, rgba};
use gpui_component::color_picker::ColorPickerState;

pub(crate) struct OverlayView {
    handle: OverlayHandle,
    focus_handle: FocusHandle,
    _session_observer: Subscription,
    _picker_observer: Option<Subscription>,
    color_picker_state: Option<Entity<ColorPickerState>>,
    recent_custom_colors: Vec<u32>,
    last_picker_color: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectionHudVisibility {
    show_handles: bool,
    show_resolution_tooltip: bool,
    show_toolbar: bool,
}

impl From<DragMode> for SelectionHudVisibility {
    fn from(drag_mode: DragMode) -> Self {
        let locked = matches!(drag_mode, DragMode::Idle);
        Self {
            show_handles: locked,
            show_resolution_tooltip: true,
            show_toolbar: locked,
        }
    }
}

pub(super) fn should_show_property_panel(active_tool: Option<AnnotationTool>, selected_annotation_kind: Option<AnnotationKindTag>) -> bool {
    active_tool.is_some() || selected_annotation_kind.is_some()
}

impl OverlayView {
    fn overlay_layer() -> Div {
        div().absolute().left(gpui::px(0.0)).top(gpui::px(0.0)).size_full()
    }

    pub(crate) fn new(handle: OverlayHandle, focus_handle: FocusHandle, cx: &mut Context<Self>) -> Self {
        let session = handle.session();
        let observer = cx.observe(&session, |_, _, cx| {
            cx.notify();
        });
        Self {
            handle,
            focus_handle,
            _session_observer: observer,
            _picker_observer: None,
            color_picker_state: None,
            recent_custom_colors: Vec::new(),
            last_picker_color: None,
        }
    }

    fn hsla_to_u32(color: Hsla) -> u32 {
        u32::from(Rgba::from(color))
    }

    fn ensure_color_picker_state(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<ColorPickerState> {
        if let Some(state) = &self.color_picker_state {
            return state.clone();
        }
        let state = cx.new(|cx| ColorPickerState::new(window, cx).default_value(rgba(COLOR_PRESETS[0])));
        let observer = cx.observe(&state, |_, _, cx| cx.notify());
        self._picker_observer = Some(observer);
        self.color_picker_state = Some(state.clone());
        state
    }

    fn push_recent_custom_color(&mut self, color: u32) {
        if COLOR_PRESETS.contains(&color) {
            return;
        }
        self.recent_custom_colors.retain(|value| *value != color);
        self.recent_custom_colors.insert(0, color);
        self.recent_custom_colors.truncate(6);
    }

    fn apply_picker_color_if_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker_state) = self.color_picker_state.as_ref().cloned() else {
            return;
        };
        let picker_color = picker_state.read(cx).value().map(Self::hsla_to_u32);
        if picker_color == self.last_picker_color {
            return;
        }
        self.last_picker_color = picker_color;
        if let Some(color) = picker_color {
            self.push_recent_custom_color(color);
            self.dispatch_annotation(AnnotationCommand::SetColor { color }, window, cx);
        }
    }

    fn sync_picker_with_style(&mut self, color: u32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker_state) = self.color_picker_state.as_ref().cloned() else {
            return;
        };
        let state_color = picker_state.read(cx).value().map(Self::hsla_to_u32);
        if state_color == Some(color) {
            self.last_picker_color = Some(color);
            return;
        }
        let hsla: Hsla = rgba(color).into();
        picker_state.update(cx, |state, cx| {
            state.set_value(hsla, window, cx);
        });
        self.last_picker_color = Some(color);
    }

    fn dispatch_command(&mut self, command: OverlayCommand, window: &mut Window, cx: &mut Context<Self>) {
        self.handle.dispatch(command, window, cx);
    }

    fn dispatch_capture(&mut self, action: CaptureAction, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_command(OverlayCommand::Capture(CaptureCommand::Execute(action)), window, cx);
    }

    fn dispatch_annotation(&mut self, command: AnnotationCommand, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_command(OverlayCommand::Annotation(command), window, cx);
    }

    fn dispatch_lifecycle(&mut self, command: LifecycleCommand, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_command(OverlayCommand::Lifecycle(command), window, cx);
    }

    fn dispatch_picker(&mut self, command: PickerCommand, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_command(OverlayCommand::Picker(command), window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::overlay::session::ResizeCorner;

    #[test]
    fn locked_selection_shows_resolution_and_toolbar() {
        let visibility = SelectionHudVisibility::from(DragMode::Idle);

        assert_eq!(
            visibility,
            SelectionHudVisibility {
                show_handles: true,
                show_resolution_tooltip: true,
                show_toolbar: true,
            }
        );
    }

    #[test]
    fn drag_move_and_resize_only_keep_resolution_tooltip() {
        for drag_mode in [DragMode::Selecting, DragMode::Resizing(ResizeCorner::TopLeft)] {
            let visibility = SelectionHudVisibility::from(drag_mode);
            assert_eq!(
                visibility,
                SelectionHudVisibility {
                    show_handles: false,
                    show_resolution_tooltip: true,
                    show_toolbar: false,
                }
            );
        }
    }

    #[test]
    fn property_panel_hidden_without_tool_and_selection() {
        assert!(!should_show_property_panel(None, None));
    }

    #[test]
    fn property_panel_visible_with_active_tool_or_selected_annotation() {
        assert!(should_show_property_panel(Some(AnnotationTool::Arrow), None));
        assert!(should_show_property_panel(None, Some(AnnotationKindTag::Rectangle)));
    }
}

use crate::core::capture::action::CaptureAction;
use crate::core::geometry::RectF;
use crate::ui::overlay::actions::{
    CloseOverlay, CopyPixelColor, CopySelection, CyclePickerFormat, MovePickerDown, MovePickerLeft, MovePickerRight, MovePickerUp, OVERLAY_CONTEXT,
    PickColorSelection, PinSelection, QrSelection, ResetSelection, SaveSelection,
};
use crate::ui::overlay::interaction::resolve_mouse_down_command;
#[cfg(feature = "overlay-diagnostics")]
use crate::ui::overlay::render::diagnostics::overlay_diagnostics_hud;
use crate::ui::overlay::render::hud::{resolution_tooltip, window_info_tooltip, window_info_tooltip_content};
use crate::ui::overlay::render::layout::{resolve_info_tooltip_layout, resolve_resolution_tooltip_layout_with_occupied, resolve_toolbar_layout};
use crate::ui::overlay::render::picker::overlay_picker;
use crate::ui::overlay::render::selection::{overlay_mask, selection_frame, selection_handles};
use crate::ui::overlay::render::toolbar::{overlay_toolbar, overlay_toolbar_action_count};
use crate::ui::overlay::session::{DragMode, OverlayCommand, OverlayHandle, OverlayPickerFrame};
use gpui::InteractiveElement;
use gpui::{
    Context, Div, FocusHandle, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Styled, Subscription, Window,
    div, img,
};
use gpui_component::ActiveTheme;
use std::rc::Rc;

pub(crate) struct OverlayView {
    handle: OverlayHandle,
    focus_handle: FocusHandle,
    _session_observer: Subscription,
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

impl OverlayView {
    fn overlay_layer() -> Div {
        div().absolute().left(gpui::px(0.0)).top(gpui::px(0.0)).size_full()
    }

    pub fn new(handle: OverlayHandle, focus_handle: FocusHandle, cx: &mut Context<Self>) -> Self {
        let session = handle.session();
        let observer = cx.observe(&session, |_, _, cx| {
            cx.notify();
        });
        Self {
            handle,
            focus_handle,
            _session_observer: observer,
        }
    }

    fn dispatch_command(&mut self, command: OverlayCommand, window: &mut Window, cx: &mut Context<Self>) {
        self.handle.dispatch(command, window, cx);
    }

    fn dispatch_capture(&mut self, action: CaptureAction, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_command(OverlayCommand::Capture(action), window, cx);
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.handle.session();
        let next_command = {
            let state = session.read(cx);
            resolve_mouse_down_command(&state, event.button, event.position)
        };
        if let Some(command) = next_command {
            self.dispatch_command(command, window, cx);
        }
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_command(OverlayCommand::PointerMoved(event.position), window, cx);
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.button == MouseButton::Left {
            self.dispatch_command(OverlayCommand::PointerReleased, window, cx);
        }
    }

    fn on_action_copy_selection(&mut self, _: &CopySelection, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_capture(CaptureAction::Copy, window, cx);
    }

    fn on_action_save_selection(&mut self, _: &SaveSelection, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_capture(CaptureAction::Save, window, cx);
    }

    fn on_action_pin_selection(&mut self, _: &PinSelection, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_capture(CaptureAction::Pin, window, cx);
    }

    fn on_action_qr_selection(&mut self, _: &QrSelection, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_capture(CaptureAction::QrCode, window, cx);
    }

    fn on_action_pick_color_selection(&mut self, _: &PickColorSelection, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_capture(CaptureAction::PickColor, window, cx);
    }

    fn on_action_copy_pixel_color(&mut self, _: &CopyPixelColor, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_command(OverlayCommand::CopyPickerColor, window, cx);
    }

    fn on_action_cycle_picker_format(&mut self, _: &CyclePickerFormat, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_command(OverlayCommand::CyclePickerFormat, window, cx);
    }

    fn move_picker_by_pixel(&mut self, delta_x: i32, delta_y: i32, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_command(OverlayCommand::MovePickerByPixel { delta_x, delta_y }, window, cx);
    }

    fn on_action_move_picker_up(&mut self, _: &MovePickerUp, window: &mut Window, cx: &mut Context<Self>) {
        self.move_picker_by_pixel(0, -1, window, cx);
    }

    fn on_action_move_picker_down(&mut self, _: &MovePickerDown, window: &mut Window, cx: &mut Context<Self>) {
        self.move_picker_by_pixel(0, 1, window, cx);
    }

    fn on_action_move_picker_left(&mut self, _: &MovePickerLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.move_picker_by_pixel(-1, 0, window, cx);
    }

    fn on_action_move_picker_right(&mut self, _: &MovePickerRight, window: &mut Window, cx: &mut Context<Self>) {
        self.move_picker_by_pixel(1, 0, window, cx);
    }

    fn on_action_reset_selection(&mut self, _: &ResetSelection, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_command(OverlayCommand::ClearSelection, window, cx);
    }

    fn on_action_close_overlay(&mut self, _: &CloseOverlay, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_command(OverlayCommand::Close, window, cx);
    }

    fn background_layer(background_image: Option<std::sync::Arc<gpui::RenderImage>>, background: gpui::Hsla) -> Div {
        if let Some(image) = background_image {
            Self::overlay_layer().child(img(image).size_full())
        } else {
            Self::overlay_layer().bg(background)
        }
    }

    fn render_selection_layer(&self, cx: &mut Context<Self>, selection: RectF, drag_mode: DragMode, viewport_w: f64, viewport_h: f64) -> Div {
        let hud_visibility = SelectionHudVisibility::from(drag_mode);
        let toolbar_layout = hud_visibility
            .show_toolbar
            .then(|| resolve_toolbar_layout(selection, overlay_toolbar_action_count(), viewport_w, viewport_h, &[]));
        let resolution_layout = hud_visibility.show_resolution_tooltip.then(|| {
            let occupied = toolbar_layout.iter().copied().collect::<Vec<_>>();
            resolve_resolution_tooltip_layout_with_occupied(selection, viewport_w, viewport_h, &occupied)
        });

        let mut layer = Self::overlay_layer().child(selection_frame(cx, selection, hud_visibility.show_handles));
        if hud_visibility.show_handles {
            layer = layer.child(selection_handles(cx, selection));
        }
        if let Some(layout) = resolution_layout {
            layer = layer.child(resolution_tooltip(cx, selection, layout));
        }
        if let Some(layout) = toolbar_layout {
            let handle = self.handle.clone();
            let on_action = Rc::new(move |command: OverlayCommand, window: &mut Window, app: &mut gpui::App| {
                handle.dispatch(command, window, app);
            });
            layer = layer.child(overlay_toolbar(cx, layout, on_action));
        }
        layer
    }

    fn render_hover_layer(
        cx: &mut Context<Self>,
        target: RectF,
        hovered_window: Option<crate::core::window::WindowInfo>,
        viewport_w: f64,
        viewport_h: f64,
    ) -> Div {
        let mut layer = Self::overlay_layer().child(selection_frame(cx, target, false));
        if let Some(window_info) = hovered_window.as_ref()
            && let Some(content) = window_info_tooltip_content(window_info)
        {
            let info_layout = resolve_info_tooltip_layout(target, content.height, viewport_w, viewport_h);
            layer = layer.child(window_info_tooltip(cx, &content, info_layout));
        }
        layer
    }

    fn render_picker_layer(cx: &mut Context<Self>, picker: &OverlayPickerFrame, viewport_w: f64, viewport_h: f64) -> Div {
        Self::overlay_layer().child(overlay_picker(
            cx,
            picker.cursor,
            picker.sample.as_ref(),
            picker.neighborhood.as_ref(),
            picker.format,
            viewport_w,
            viewport_h,
        ))
    }
}

impl gpui::Render for OverlayView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let frame = self.handle.prepare_frame(window, cx);
        let viewport = window.viewport_size();
        let viewport_w = viewport.width.to_f64();
        let viewport_h = viewport.height.to_f64();
        let active_rect = frame.selection.or(frame.target);

        let theme = cx.theme();
        let mut root = div()
            .id("overlay-root")
            .track_focus(&self.focus_handle)
            .key_context(OVERLAY_CONTEXT)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_action(cx.listener(Self::on_action_copy_selection))
            .on_action(cx.listener(Self::on_action_save_selection))
            .on_action(cx.listener(Self::on_action_pin_selection))
            .on_action(cx.listener(Self::on_action_qr_selection))
            .on_action(cx.listener(Self::on_action_pick_color_selection))
            .on_action(cx.listener(Self::on_action_copy_pixel_color))
            .on_action(cx.listener(Self::on_action_cycle_picker_format))
            .on_action(cx.listener(Self::on_action_move_picker_up))
            .on_action(cx.listener(Self::on_action_move_picker_down))
            .on_action(cx.listener(Self::on_action_move_picker_left))
            .on_action(cx.listener(Self::on_action_move_picker_right))
            .on_action(cx.listener(Self::on_action_reset_selection))
            .on_action(cx.listener(Self::on_action_close_overlay))
            .size_full();

        root = root.child(Self::background_layer(frame.background_image.clone(), theme.background));
        root = root.child(overlay_mask(cx, active_rect, viewport_w, viewport_h));

        if let Some(selection) = frame.selection {
            root = root.child(self.render_selection_layer(cx, selection, frame.drag_mode, viewport_w, viewport_h));
        } else if let Some(target) = frame.target {
            root = root.child(Self::render_hover_layer(cx, target, frame.hovered_window.clone(), viewport_w, viewport_h));
        }

        if let Some(picker) = frame.picker.as_ref() {
            root = root.child(Self::render_picker_layer(cx, picker, viewport_w, viewport_h));
        }

        #[cfg(feature = "overlay-diagnostics")]
        {
            root = root.child(overlay_diagnostics_hud(cx, &frame.diagnostics, viewport_w, viewport_h));
        }

        root
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
        for drag_mode in [DragMode::Selecting, DragMode::Moving, DragMode::Resizing(ResizeCorner::TopLeft)] {
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
}

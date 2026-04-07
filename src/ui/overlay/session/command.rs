use super::{
    AnnotationCommand, AnnotationKind, CaptureCommand, DragMode, LifecycleCommand, OverlayCommand, OverlayEffect, OverlayOutcome, OverlaySession,
    PickerCommand, SessionTransition,
};
use crate::core::capture::action::{ActionContext, CaptureAction};
use crate::core::capture::active_monitor_scale;
use crate::core::geometry::RectF;
use crate::core::i18n;
use crate::core::notify::NotificationType;

impl OverlaySession {
    pub(super) fn apply(&mut self, command: OverlayCommand) -> OverlayOutcome {
        let transition = match command {
            OverlayCommand::Capture(command) => self.apply_capture(command),
            OverlayCommand::Picker(command) => self.apply_picker(command),
            OverlayCommand::Annotation(command) => self.apply_annotation(command),
            OverlayCommand::Lifecycle(command) => self.apply_lifecycle(command),
        };
        transition.into_outcome()
    }

    fn apply_capture(&mut self, command: CaptureCommand) -> SessionTransition {
        match command {
            CaptureCommand::Execute(action) => {
                if matches!(action, CaptureAction::Copy) && self.text_editing_id().is_some() {
                    return SessionTransition::from_changed(self.commit_text_edit());
                }
                self.capture_effect(action)
                    .map(SessionTransition::Effect)
                    .unwrap_or(SessionTransition::NoOp)
            }
            CaptureCommand::CopyPickerColor => self
                .picker_text()
                .map(|text| {
                    SessionTransition::Effect(OverlayEffect::CopyText {
                        message: i18n::notify::copied_qr(),
                        text,
                        title: i18n::app::capture_name(),
                        notification_type: NotificationType::Copy,
                        close_on_success: true,
                    })
                })
                .unwrap_or(SessionTransition::NoOp),
        }
    }

    fn apply_picker(&mut self, command: PickerCommand) -> SessionTransition {
        match command {
            PickerCommand::CycleFormat => SessionTransition::from_changed(self.cycle_picker_format()),
            PickerCommand::MoveByPixel { delta_x, delta_y } => SessionTransition::from_changed(self.move_picker_by_pixel(delta_x, delta_y)),
        }
    }

    fn apply_annotation(&mut self, command: AnnotationCommand) -> SessionTransition {
        match command {
            AnnotationCommand::SetTool(tool) => SessionTransition::from_changed(self.set_annotation_tool(tool)),
            AnnotationCommand::StartDraw(point) => SessionTransition::from_changed(self.start_annotation_draw(point)),
            AnnotationCommand::StartMove { id, point } => SessionTransition::from_changed(self.start_annotation_move(id, point)),
            AnnotationCommand::Select(id) => SessionTransition::from_changed(self.select_annotation(id)),
            AnnotationCommand::DeleteIntent => {
                if self.text_editing_id().is_some() {
                    SessionTransition::from_changed(self.backspace_text_edit())
                } else {
                    SessionTransition::from_changed(self.delete_selected_annotation())
                }
            }
            AnnotationCommand::Undo => SessionTransition::from_changed(self.undo_annotation()),
            AnnotationCommand::Redo => SessionTransition::from_changed(self.redo_annotation()),
            AnnotationCommand::CycleColor => SessionTransition::from_changed(self.cycle_annotation_color()),
            AnnotationCommand::SetColor { color } => SessionTransition::from_changed(self.set_annotation_color(color)),
            AnnotationCommand::ToggleFill => SessionTransition::from_changed(self.toggle_annotation_fill()),
            AnnotationCommand::AdjustStroke { delta } => SessionTransition::from_changed(self.adjust_annotation_stroke(delta)),
            AnnotationCommand::SetMosaicMode(mode) => SessionTransition::from_changed(self.set_annotation_mosaic_mode(mode)),
            AnnotationCommand::AdjustMosaicIntensity { delta } => SessionTransition::from_changed(self.adjust_annotation_mosaic_intensity(delta)),
            AnnotationCommand::AdjustByWheel { point, delta } => {
                SessionTransition::from_changed(self.adjust_selected_annotation_by_wheel(point, delta))
            }
            AnnotationCommand::StartTextEdit => SessionTransition::from_changed(self.begin_text_edit_selected()),
            AnnotationCommand::StartTextEditAtPoint(point) => {
                let next = self
                    .annotation_hit_test(point)
                    .and_then(|id| matches!(self.annotation_kind_for(id), Some(AnnotationKind::Text { .. })).then_some(id));
                if let Some(id) = next {
                    self.select_annotation(Some(id));
                    SessionTransition::from_changed(self.begin_text_edit_selected())
                } else {
                    SessionTransition::NoOp
                }
            }
            AnnotationCommand::AppendText { text } => SessionTransition::from_changed(self.append_text_edit(&text)),
            AnnotationCommand::InsertTextNewline => SessionTransition::from_changed(self.insert_newline_text_edit()),
        }
    }

    fn apply_lifecycle(&mut self, command: LifecycleCommand) -> SessionTransition {
        match command {
            LifecycleCommand::StartSelection(point) => {
                self.start_selection(point);
                SessionTransition::Refresh
            }
            LifecycleCommand::StartResize { corner, point } => {
                self.start_resize(corner, point);
                SessionTransition::Refresh
            }
            LifecycleCommand::PointerMoved(point) => {
                let queued_first = self.queue_pointer(point);
                let refresh = match self.mode() {
                    DragMode::Idle => queued_first,
                    DragMode::Selecting | DragMode::Resizing(_) => self.apply_pending_pointer(),
                };
                SessionTransition::from_changed(refresh)
            }
            LifecycleCommand::PointerReleased => {
                if self.has_active_annotation_interaction() {
                    self.finish_annotation_interaction();
                } else {
                    match self.mode() {
                        DragMode::Selecting => self.finish_selection(),
                        DragMode::Resizing(_) => self.finish_resize(),
                        DragMode::Idle => {}
                    }
                }
                SessionTransition::Refresh
            }
            LifecycleCommand::ClearSelection => {
                self.clear();
                SessionTransition::Refresh
            }
            LifecycleCommand::CloseIntent => {
                if self.text_editing_id().is_some() {
                    SessionTransition::from_changed(self.cancel_text_edit())
                } else {
                    SessionTransition::Effect(OverlayEffect::Close)
                }
            }
        }
    }

    fn capture_effect(&self, action: CaptureAction) -> Option<OverlayEffect> {
        if matches!(action, CaptureAction::Scroll) {
            let selection_rect = self.selection_rect()?;
            if !selection_rect.has_area() {
                return None;
            }

            let viewport_rect = RectF::new(0.0, 0.0, self.viewport.viewport_w, self.viewport.viewport_h);
            let viewport_scale = f64::from(active_monitor_scale()).max(1.0);
            return Some(OverlayEffect::StartLongCapture {
                selection_rect,
                viewport_rect,
                viewport_scale,
            });
        }

        let background_path = self.composed_background_path()?;
        let selection = self.selection_rect()?;
        Some(OverlayEffect::Capture {
            action,
            context: ActionContext::crop_selection(background_path, selection),
        })
    }
}

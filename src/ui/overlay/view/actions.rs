use super::OverlayView;
use crate::core::capture::action::CaptureAction;
use crate::ui::overlay::actions::{
    CloseOverlay, CopyPixelColor, CopySelection, CycleAnnotationColorAction, CyclePickerFormat, DecreaseAnnotationStroke, DeleteAnnotation,
    IncreaseAnnotationStroke, MovePickerDown, MovePickerLeft, MovePickerRight, MovePickerUp, PickColorSelection, PinSelection, QrSelection,
    RedoAnnotationAction, ResetSelection, SaveSelection, SelectArrowTool, SelectCircleTool, SelectCounterTool, SelectMosaicTool, SelectRectangleTool,
    SelectTextTool, StartTextEditAction, ToggleAnnotationFillAction, UndoAnnotationAction,
};
use crate::ui::overlay::session::{AnnotationCommand, CaptureCommand, LifecycleCommand, PickerCommand};
use gpui::{Context, Window};

macro_rules! annotation_action_handler {
    ($name:ident, $action:ty, $command:expr) => {
        pub(super) fn $name(&mut self, _: &$action, window: &mut Window, cx: &mut Context<Self>) {
            self.dispatch_annotation($command, window, cx);
        }
    };
}

impl OverlayView {
    pub(super) fn on_action_copy_selection(&mut self, _: &CopySelection, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_capture(CaptureAction::Copy, window, cx);
    }

    pub(super) fn on_action_save_selection(&mut self, _: &SaveSelection, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_capture(CaptureAction::Save, window, cx);
    }

    pub(super) fn on_action_pin_selection(&mut self, _: &PinSelection, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_capture(CaptureAction::Pin, window, cx);
    }

    pub(super) fn on_action_qr_selection(&mut self, _: &QrSelection, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_capture(CaptureAction::QrCode, window, cx);
    }

    pub(super) fn on_action_pick_color_selection(&mut self, _: &PickColorSelection, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_capture(CaptureAction::PickColor, window, cx);
    }

    pub(super) fn on_action_copy_pixel_color(&mut self, _: &CopyPixelColor, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_command(
            crate::ui::overlay::session::OverlayCommand::Capture(CaptureCommand::CopyPickerColor),
            window,
            cx,
        );
    }

    pub(super) fn on_action_cycle_picker_format(&mut self, _: &CyclePickerFormat, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_picker(PickerCommand::CycleFormat, window, cx);
    }

    pub(super) fn move_picker_by_pixel(&mut self, delta_x: i32, delta_y: i32, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_picker(PickerCommand::MoveByPixel { delta_x, delta_y }, window, cx);
    }

    pub(super) fn on_action_move_picker_up(&mut self, _: &MovePickerUp, window: &mut Window, cx: &mut Context<Self>) {
        self.move_picker_by_pixel(0, -1, window, cx);
    }

    pub(super) fn on_action_move_picker_down(&mut self, _: &MovePickerDown, window: &mut Window, cx: &mut Context<Self>) {
        self.move_picker_by_pixel(0, 1, window, cx);
    }

    pub(super) fn on_action_move_picker_left(&mut self, _: &MovePickerLeft, window: &mut Window, cx: &mut Context<Self>) {
        self.move_picker_by_pixel(-1, 0, window, cx);
    }

    pub(super) fn on_action_move_picker_right(&mut self, _: &MovePickerRight, window: &mut Window, cx: &mut Context<Self>) {
        self.move_picker_by_pixel(1, 0, window, cx);
    }

    annotation_action_handler!(
        on_action_select_arrow_tool,
        SelectArrowTool,
        AnnotationCommand::SetTool(crate::ui::overlay::session::AnnotationTool::Arrow)
    );
    annotation_action_handler!(
        on_action_select_rectangle_tool,
        SelectRectangleTool,
        AnnotationCommand::SetTool(crate::ui::overlay::session::AnnotationTool::Rectangle)
    );
    annotation_action_handler!(
        on_action_select_circle_tool,
        SelectCircleTool,
        AnnotationCommand::SetTool(crate::ui::overlay::session::AnnotationTool::Circle)
    );
    annotation_action_handler!(
        on_action_select_counter_tool,
        SelectCounterTool,
        AnnotationCommand::SetTool(crate::ui::overlay::session::AnnotationTool::Counter)
    );
    annotation_action_handler!(
        on_action_select_text_tool,
        SelectTextTool,
        AnnotationCommand::SetTool(crate::ui::overlay::session::AnnotationTool::Text)
    );
    annotation_action_handler!(
        on_action_select_mosaic_tool,
        SelectMosaicTool,
        AnnotationCommand::SetTool(crate::ui::overlay::session::AnnotationTool::Mosaic)
    );
    annotation_action_handler!(on_action_undo_annotation, UndoAnnotationAction, AnnotationCommand::Undo);
    annotation_action_handler!(on_action_redo_annotation, RedoAnnotationAction, AnnotationCommand::Redo);
    annotation_action_handler!(on_action_delete_annotation, DeleteAnnotation, AnnotationCommand::DeleteIntent);
    annotation_action_handler!(
        on_action_cycle_annotation_color,
        CycleAnnotationColorAction,
        AnnotationCommand::CycleColor
    );
    annotation_action_handler!(
        on_action_toggle_annotation_fill,
        ToggleAnnotationFillAction,
        AnnotationCommand::ToggleFill
    );
    annotation_action_handler!(
        on_action_increase_annotation_stroke,
        IncreaseAnnotationStroke,
        AnnotationCommand::AdjustStroke { delta: 1.0 }
    );
    annotation_action_handler!(
        on_action_decrease_annotation_stroke,
        DecreaseAnnotationStroke,
        AnnotationCommand::AdjustStroke { delta: -1.0 }
    );
    annotation_action_handler!(on_action_start_text_edit, StartTextEditAction, AnnotationCommand::StartTextEdit);

    pub(super) fn on_action_reset_selection(&mut self, _: &ResetSelection, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_lifecycle(LifecycleCommand::ClearSelection, window, cx);
    }

    pub(super) fn on_action_close_overlay(&mut self, _: &CloseOverlay, window: &mut Window, cx: &mut Context<Self>) {
        self.dispatch_lifecycle(LifecycleCommand::CloseIntent, window, cx);
    }
}

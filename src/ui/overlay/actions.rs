use gpui::{App, KeyBinding, actions};

pub(crate) const OVERLAY_CONTEXT: &str = "MinnowSnapOverlay";

actions!(
    overlay,
    [
        CopySelection,
        SaveSelection,
        PinSelection,
        QrSelection,
        PickColorSelection,
        CopyPixelColor,
        CyclePickerFormat,
        MovePickerUp,
        MovePickerDown,
        MovePickerLeft,
        MovePickerRight,
        SelectArrowTool,
        SelectRectangleTool,
        SelectCircleTool,
        SelectCounterTool,
        SelectTextTool,
        SelectMosaicTool,
        UndoAnnotationAction,
        RedoAnnotationAction,
        DeleteAnnotation,
        CycleAnnotationColorAction,
        ToggleAnnotationFillAction,
        IncreaseAnnotationStroke,
        DecreaseAnnotationStroke,
        StartTextEditAction,
        ResetSelection,
        CloseOverlay
    ]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", CopySelection, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("ctrl-s", SaveSelection, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("ctrl-p", PinSelection, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("ctrl-o", QrSelection, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("ctrl-shift-c", PickColorSelection, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("c", CopyPixelColor, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("shift", CyclePickerFormat, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("up", MovePickerUp, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("down", MovePickerDown, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("left", MovePickerLeft, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("right", MovePickerRight, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("1", SelectArrowTool, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("2", SelectRectangleTool, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("3", SelectCircleTool, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("4", SelectCounterTool, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("5", SelectTextTool, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("6", SelectMosaicTool, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("ctrl-z", UndoAnnotationAction, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("ctrl-shift-z", RedoAnnotationAction, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("backspace", DeleteAnnotation, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("delete", DeleteAnnotation, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("x", CycleAnnotationColorAction, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("f", ToggleAnnotationFillAction, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("]", IncreaseAnnotationStroke, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("[", DecreaseAnnotationStroke, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("ctrl-e", StartTextEditAction, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("r", ResetSelection, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("escape", CloseOverlay, Some(OVERLAY_CONTEXT)),
    ]);
}

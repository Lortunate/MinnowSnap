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
        KeyBinding::new("r", ResetSelection, Some(OVERLAY_CONTEXT)),
        KeyBinding::new("escape", CloseOverlay, Some(OVERLAY_CONTEXT)),
    ]);
}

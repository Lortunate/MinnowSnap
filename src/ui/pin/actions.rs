use gpui::{App, KeyBinding, actions};

pub(crate) const PIN_CONTEXT: &str = "MinnowSnapPin";

actions!(pin, [ClosePin, CloseAllPins]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", ClosePin, Some(PIN_CONTEXT)),
        KeyBinding::new("ctrl-w", ClosePin, Some(PIN_CONTEXT)),
        KeyBinding::new("ctrl-shift-w", CloseAllPins, Some(PIN_CONTEXT)),
    ]);
}

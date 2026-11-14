use crate::presentation::hotkeys::{HotkeyAction, shortcuts_conflict};
use crate::presentation::i18n;
use minnow_application::{preferences::NativePreferencesSnapshot, settings::ShortcutSettings};

pub(crate) fn conflicts_with_desired(desired: &ShortcutSettings, target_action: HotkeyAction, formatted: &str) -> bool {
    match target_action {
        HotkeyAction::Capture => shortcuts_conflict(formatted, &desired.quick_capture),
        HotkeyAction::QuickCapture => shortcuts_conflict(&desired.capture, formatted),
    }
}

pub(super) fn retry_needed(desired: &ShortcutSettings, native: &NativePreferencesSnapshot) -> bool {
    desired.capture != native.shortcuts.capture.as_deref().unwrap_or_default()
        || desired.quick_capture != native.shortcuts.quick_capture.as_deref().unwrap_or_default()
        || native.shortcut_failure.is_some()
        || !native.shortcuts.unrouted.is_empty()
        || !native.shortcuts.uncertain.is_empty()
}

pub(super) fn effective_label(shortcut: &str) -> String {
    i18n::preferences::shortcut_effective(if shortcut.is_empty() {
        i18n::preferences::shortcut_not_registered()
    } else {
        shortcut.to_owned()
    })
}

use super::{MutationResult, store};
use crate::core::{
    hotkey::{HotkeyAction, HotkeyService, ShortcutBindings},
    i18n,
};
use crate::ui::preferences::view::PreferencesView;
use gpui::{App, BorrowAppContext, Context, SharedString};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShortcutsSnapshot {
    pub(crate) bindings: ShortcutBindings,
    pub(crate) conflict_message: Option<SharedString>,
}

pub(crate) fn snapshot(cx: &App) -> ShortcutsSnapshot {
    let bindings = current_shortcut_bindings(cx);

    ShortcutsSnapshot {
        conflict_message: shortcut_conflict_message(&bindings),
        bindings,
    }
}

pub(crate) fn next_shortcut_bindings(current: &ShortcutBindings, target_action: HotkeyAction, formatted: &str) -> ShortcutBindings {
    match target_action {
        HotkeyAction::Capture => current.with_capture(formatted),
        HotkeyAction::QuickCapture => current.with_quick_capture(formatted),
    }
}

pub(crate) fn persist_shortcut_bindings(bindings: ShortcutBindings, cx: &mut Context<PreferencesView>) -> Result<MutationResult, SharedString> {
    if bindings.has_conflict() {
        return Err(SharedString::from(i18n::preferences::shortcuts_conflict()));
    }

    if cx.has_global::<HotkeyService>() {
        let update_result = cx.update_global::<HotkeyService, _>(|service: &mut HotkeyService, _| service.update_bindings(bindings.clone()));
        if update_result.is_err() {
            return Err(SharedString::from(i18n::preferences::shortcuts_conflict()));
        }
    } else {
        store::with_settings(|settings| {
            settings.set_capture_shortcut(bindings.capture.clone());
            settings.set_quick_capture_shortcut(bindings.quick_capture.clone());
        });
    }

    Ok(MutationResult::refresh_windows().clear_notice())
}

fn current_shortcut_bindings(cx: &App) -> ShortcutBindings {
    if cx.has_global::<HotkeyService>() {
        cx.global::<HotkeyService>().current_bindings()
    } else {
        let settings = store::snapshot();
        ShortcutBindings::from_settings(&settings.shortcuts)
    }
}

fn shortcut_conflict_message(bindings: &ShortcutBindings) -> Option<SharedString> {
    bindings
        .has_conflict()
        .then(|| SharedString::from(i18n::preferences::shortcuts_conflict()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_conflict_message_only_shows_for_conflicts() {
        let conflicting = ShortcutBindings {
            capture: "Ctrl+Shift+A".to_string(),
            quick_capture: "ctrl+shift+a".to_string(),
        };

        let snapshot = ShortcutsSnapshot {
            conflict_message: shortcut_conflict_message(&conflicting),
            bindings: conflicting,
        };

        assert_eq!(
            snapshot.conflict_message,
            Some(SharedString::from(i18n::preferences::shortcuts_conflict()))
        );
        assert_eq!(shortcut_conflict_message(&ShortcutBindings::default()), None);
    }

    #[test]
    fn next_shortcut_bindings_updates_only_target_action() {
        let current = ShortcutBindings::default();

        let capture = next_shortcut_bindings(&current, HotkeyAction::Capture, "Ctrl+Shift+1");
        assert_eq!(capture.capture, "Ctrl+Shift+1");
        assert_eq!(capture.quick_capture, current.quick_capture);

        let quick = next_shortcut_bindings(&current, HotkeyAction::QuickCapture, "Ctrl+Shift+2");
        assert_eq!(quick.quick_capture, "Ctrl+Shift+2");
        assert_eq!(quick.capture, current.capture);
    }
}

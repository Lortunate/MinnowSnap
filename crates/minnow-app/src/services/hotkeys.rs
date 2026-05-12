use crate::services::settings::ShortcutSettings;
use gpui::Keystroke;
use std::str::FromStr;

pub const DEFAULT_CAPTURE_SHORTCUT: &str = "F1";
pub const DEFAULT_QUICK_CAPTURE_SHORTCUT: &str = "F2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotkeyAction {
    Capture,
    QuickCapture,
}

impl HotkeyAction {
    fn default_shortcut(self) -> &'static str {
        match self {
            Self::Capture => DEFAULT_CAPTURE_SHORTCUT,
            Self::QuickCapture => DEFAULT_QUICK_CAPTURE_SHORTCUT,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutBindings {
    pub capture: String,
    pub quick_capture: String,
}

impl Default for ShortcutBindings {
    fn default() -> Self {
        Self {
            capture: DEFAULT_CAPTURE_SHORTCUT.to_string(),
            quick_capture: DEFAULT_QUICK_CAPTURE_SHORTCUT.to_string(),
        }
    }
}

impl ShortcutBindings {
    pub fn from_settings(settings: &ShortcutSettings) -> Self {
        Self {
            capture: resolve_shortcut(&settings.capture, HotkeyAction::Capture),
            quick_capture: resolve_shortcut(&settings.quick_capture, HotkeyAction::QuickCapture),
        }
    }

    pub fn with_capture(&self, shortcut: &str) -> Self {
        Self {
            capture: resolve_shortcut(shortcut, HotkeyAction::Capture),
            quick_capture: self.quick_capture.clone(),
        }
    }

    pub fn with_quick_capture(&self, shortcut: &str) -> Self {
        Self {
            capture: self.capture.clone(),
            quick_capture: resolve_shortcut(shortcut, HotkeyAction::QuickCapture),
        }
    }

    pub fn has_conflict(&self) -> bool {
        shortcuts_conflict(&self.capture, &self.quick_capture)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotkeyUpdateError {
    Conflict,
}

pub fn parse_hotkey(shortcut: &str) -> Option<global_hotkey::hotkey::HotKey> {
    if shortcut.is_empty() {
        return None;
    }

    match global_hotkey::hotkey::HotKey::from_str(shortcut) {
        Ok(hotkey) => Some(hotkey),
        Err(err) => {
            tracing::error!("Failed to parse hotkey '{shortcut}': {err}");
            None
        }
    }
}

pub fn resolve_shortcut(shortcut: &str, action: HotkeyAction) -> String {
    let trimmed = shortcut.trim();
    if trimmed.is_empty() {
        action.default_shortcut().to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn shortcuts_conflict(capture: &str, quick_capture: &str) -> bool {
    normalize_shortcut_for_compare(capture, HotkeyAction::Capture) == normalize_shortcut_for_compare(quick_capture, HotkeyAction::QuickCapture)
}

pub fn format_keystroke(keystroke: &Keystroke) -> Option<String> {
    if is_modifier_only_key(&keystroke.key) {
        return None;
    }

    let mut tokens = Vec::new();
    if keystroke.modifiers.control {
        tokens.push("Ctrl".to_string());
    }
    if keystroke.modifiers.alt {
        tokens.push("Alt".to_string());
    }
    if keystroke.modifiers.shift {
        tokens.push("Shift".to_string());
    }
    if keystroke.modifiers.platform {
        #[cfg(target_os = "macos")]
        tokens.push("Cmd".to_string());

        #[cfg(not(target_os = "macos"))]
        tokens.push("Win".to_string());
    }
    if keystroke.modifiers.function {
        tokens.push("Fn".to_string());
    }

    tokens.push(display_key_token(&keystroke.key));
    Some(tokens.join("+"))
}

fn normalize_shortcut_for_compare(shortcut: &str, action: HotkeyAction) -> String {
    resolve_shortcut(shortcut, action)
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn is_modifier_only_key(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "shift" | "control" | "ctrl" | "alt" | "command" | "cmd" | "super" | "platform" | "function" | "fn"
    )
}

fn display_key_token(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.len() == 1 {
        return lower.to_ascii_uppercase();
    }

    if lower.starts_with('f') && lower.chars().skip(1).all(|ch| ch.is_ascii_digit()) {
        return lower.to_ascii_uppercase();
    }

    match lower.as_str() {
        "escape" => "Escape".to_string(),
        "space" => "Space".to_string(),
        "tab" => "Tab".to_string(),
        "enter" => "Enter".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" => "Delete".to_string(),
        "up" => "Up".to_string(),
        "down" => "Down".to_string(),
        "left" => "Left".to_string(),
        "right" => "Right".to_string(),
        other => other.to_ascii_uppercase(),
    }
}

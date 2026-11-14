use gpui::Keystroke;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotkeyAction {
    Capture,
    QuickCapture,
}

pub fn shortcuts_conflict(capture: &str, quick_capture: &str) -> bool {
    !capture.trim().is_empty()
        && !quick_capture.trim().is_empty()
        && normalize_shortcut_for_compare(capture) == normalize_shortcut_for_compare(quick_capture)
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

fn normalize_shortcut_for_compare(shortcut: &str) -> String {
    shortcut.chars().filter(|ch| !ch.is_whitespace()).collect::<String>().to_ascii_lowercase()
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

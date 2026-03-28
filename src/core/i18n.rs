pub const SYSTEM_LOCALE: &str = "System";
pub const SUPPORTED_LOCALES: [&str; 3] = [SYSTEM_LOCALE, "en-US", "zh-CN"];

pub fn init(language: &str) -> String {
    let locale = resolve_locale(language);
    rust_i18n::set_locale(&locale);
    gpui_component::set_locale(&locale);
    locale
}

pub fn resolve_locale(language: &str) -> String {
    if language.trim().is_empty() || language == SYSTEM_LOCALE {
        return detect_system_locale();
    }

    normalize_locale_tag(language)
}

pub fn normalize_locale_tag(locale: &str) -> String {
    match locale.trim() {
        "" | "System" => detect_system_locale(),
        "zh_CN" | "zh-CN" | "zh" => "zh-CN".to_string(),
        "en_US" | "en-US" | "en" => "en".to_string(),
        other => other.replace('_', "-"),
    }
}

pub fn detect_system_locale() -> String {
    let system = sys_locale::get_locale().unwrap_or_else(|| "en-US".to_string());
    match system.to_ascii_lowercase().as_str() {
        value if value.starts_with("zh") => "zh-CN".to_string(),
        value if value.starts_with("en") => "en".to_string(),
        _ => "en".to_string(),
    }
}

pub fn bool_label(value: bool) -> String {
    if value { common::enabled() } else { common::disabled() }
}

pub fn default_path_label() -> String {
    preferences::default_path()
}

pub mod app {
    use rust_i18n::t;

    pub fn name() -> String {
        t!("app.name").into_owned()
    }

    pub fn capture_name() -> String {
        t!("app.capture_name").into_owned()
    }
}

pub mod common {
    use rust_i18n::t;

    pub fn close() -> String {
        t!("common.actions.close").into_owned()
    }

    pub fn cancel() -> String {
        t!("common.actions.cancel").into_owned()
    }

    pub fn copy() -> String {
        t!("common.actions.copy").into_owned()
    }

    pub fn save() -> String {
        t!("common.actions.save").into_owned()
    }

    pub fn pin() -> String {
        t!("common.actions.pin").into_owned()
    }

    pub fn scan_qr() -> String {
        t!("common.actions.scan_qr").into_owned()
    }

    pub fn enabled() -> String {
        t!("common.states.enabled").into_owned()
    }

    pub fn disabled() -> String {
        t!("common.states.disabled").into_owned()
    }
}

pub mod overlay {
    use rust_i18n::t;

    pub fn preparing_surface() -> String {
        t!("overlay.status.preparing_surface").into_owned()
    }

    pub fn ready_to_select() -> String {
        t!("overlay.status.ready_to_select").into_owned()
    }

    pub fn selecting_area() -> String {
        t!("overlay.status.selecting_area").into_owned()
    }

    pub fn selection_size(width: impl ToString, height: impl ToString) -> String {
        t!("overlay.status.selection_size", width = width.to_string(), height = height.to_string()).into_owned()
    }

    pub fn selection_locked(width: impl ToString, height: impl ToString) -> String {
        t!("overlay.status.selection_locked", width = width.to_string(), height = height.to_string()).into_owned()
    }

    pub fn moving_selection() -> String {
        t!("overlay.status.moving_selection").into_owned()
    }

    pub fn resizing_selection() -> String {
        t!("overlay.status.resizing_selection").into_owned()
    }

    pub fn missing_surface() -> String {
        t!("overlay.status.missing_surface").into_owned()
    }

    pub fn missing_selection() -> String {
        t!("overlay.status.missing_selection").into_owned()
    }

    pub fn copied_selection() -> String {
        t!("overlay.status.copied_selection").into_owned()
    }

    pub fn saved_selection() -> String {
        t!("overlay.status.saved_selection").into_owned()
    }

    pub fn pin_ready(path: impl ToString, auto_ocr: impl ToString) -> String {
        t!("overlay.status.pin_ready", path = path.to_string(), auto_ocr = auto_ocr.to_string()).into_owned()
    }

    pub fn qr_ready(content: impl ToString) -> String {
        t!("overlay.status.qr_ready", content = content.to_string()).into_owned()
    }

    pub fn action_unavailable() -> String {
        t!("overlay.status.action_unavailable").into_owned()
    }

    pub fn qr_copy_failed() -> String {
        t!("overlay.status.qr_copy_failed").into_owned()
    }

    pub fn qr_not_found() -> String {
        t!("overlay.notify.qr_not_found").into_owned()
    }

    pub fn picker_value_and_format(value: impl ToString, format: impl ToString) -> String {
        t!("overlay.picker.value_and_format", value = value.to_string(), format = format.to_string()).into_owned()
    }

    pub fn picker_coordinates(x: impl ToString, y: impl ToString) -> String {
        t!("overlay.picker.coordinates", x = x.to_string(), y = y.to_string()).into_owned()
    }

    pub fn picker_shortcuts(copy_key: impl ToString, cycle_key: impl ToString) -> String {
        t!(
            "overlay.picker.shortcuts",
            copy_key = copy_key.to_string(),
            cycle_key = cycle_key.to_string()
        )
        .into_owned()
    }
}

pub mod notify {
    use rust_i18n::t;

    pub fn copied_image() -> String {
        t!("notify.capture.copied_image").into_owned()
    }

    pub fn saved_image(path: impl ToString) -> String {
        t!("notify.capture.saved_image", path = path.to_string()).into_owned()
    }

    pub fn copied_qr() -> String {
        t!("notify.capture.copied_qr").into_owned()
    }

    pub fn quick_capture_copied() -> String {
        t!("notify.capture.quick_capture_copied").into_owned()
    }

    pub fn quick_capture_failed() -> String {
        t!("notify.capture.quick_capture_failed").into_owned()
    }

    pub fn pin_reissued(auto_ocr: impl ToString) -> String {
        t!("notify.capture.pin_reissued", auto_ocr = auto_ocr.to_string()).into_owned()
    }
}

pub mod capture {
    use rust_i18n::t;

    pub fn copy_failed() -> String {
        t!("capture.errors.copy_failed").into_owned()
    }

    pub fn pin_failed() -> String {
        t!("capture.errors.pin_failed").into_owned()
    }
}

pub mod preferences {
    use rust_i18n::t;

    pub fn title() -> String {
        t!("preferences.window.title").into_owned()
    }

    pub fn subtitle() -> String {
        t!("preferences.window.subtitle").into_owned()
    }

    pub fn auto_start() -> String {
        t!("preferences.fields.auto_start").into_owned()
    }

    pub fn ocr() -> String {
        t!("preferences.fields.ocr").into_owned()
    }

    pub fn notifications() -> String {
        t!("preferences.fields.notifications").into_owned()
    }

    pub fn save_path() -> String {
        t!("preferences.fields.save_path").into_owned()
    }

    pub fn default_path() -> String {
        t!("preferences.fields.default_path").into_owned()
    }

    pub fn intro() -> String {
        t!("preferences.description.intro").into_owned()
    }
}

pub mod pin {
    use rust_i18n::t;

    pub fn badge_auto_ocr() -> String {
        t!("pin.badge.auto_ocr").into_owned()
    }

    pub fn badge_pinned_image() -> String {
        t!("pin.badge.pinned_image").into_owned()
    }

    pub fn shortcuts_hint() -> String {
        t!("pin.hint.shortcuts").into_owned()
    }

    pub fn close_all() -> String {
        t!("pin.menu.close_all").into_owned()
    }
}

pub mod tray {
    use rust_i18n::t;

    pub fn capture_overlay() -> String {
        t!("tray.actions.capture_overlay").into_owned()
    }

    pub fn quick_capture() -> String {
        t!("tray.actions.quick_capture").into_owned()
    }

    pub fn preferences() -> String {
        t!("tray.actions.preferences").into_owned()
    }

    pub fn exit() -> String {
        t!("tray.actions.exit").into_owned()
    }
}

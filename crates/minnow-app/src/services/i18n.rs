pub const SYSTEM_LOCALE: &str = "System";
pub const SUPPORTED_LOCALES: [&str; 3] = [SYSTEM_LOCALE, "en-US", "zh-CN"];

pub fn init(language: &str) -> String {
    let locale = resolve_locale(language);
    rust_i18n::set_locale(&locale);
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

    pub fn ocr() -> String {
        t!("common.actions.ocr").into_owned()
    }

    pub fn scan_qr() -> String {
        t!("common.actions.scan_qr").into_owned()
    }

    pub fn scroll() -> String {
        t!("common.actions.scroll").into_owned()
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

    pub fn annotation_tool_arrow() -> String {
        t!("overlay.annotation.tool.arrow").into_owned()
    }

    pub fn annotation_tool_rectangle() -> String {
        t!("overlay.annotation.tool.rectangle").into_owned()
    }

    pub fn annotation_tool_circle() -> String {
        t!("overlay.annotation.tool.circle").into_owned()
    }

    pub fn annotation_tool_counter() -> String {
        t!("overlay.annotation.tool.counter").into_owned()
    }

    pub fn annotation_tool_text() -> String {
        t!("overlay.annotation.tool.text").into_owned()
    }

    pub fn annotation_tool_mosaic() -> String {
        t!("overlay.annotation.tool.mosaic").into_owned()
    }

    pub fn annotation_undo() -> String {
        t!("overlay.annotation.actions.undo").into_owned()
    }

    pub fn annotation_redo() -> String {
        t!("overlay.annotation.actions.redo").into_owned()
    }

    pub fn annotation_cycle_color() -> String {
        t!("overlay.annotation.actions.cycle_color").into_owned()
    }

    pub fn annotation_set_color() -> String {
        t!("overlay.annotation.actions.set_color").into_owned()
    }

    pub fn annotation_custom_color() -> String {
        t!("overlay.annotation.actions.custom_color").into_owned()
    }

    pub fn annotation_toggle_fill() -> String {
        t!("overlay.annotation.actions.toggle_fill").into_owned()
    }

    pub fn annotation_stroke_up() -> String {
        t!("overlay.annotation.actions.stroke_up").into_owned()
    }

    pub fn annotation_stroke_down() -> String {
        t!("overlay.annotation.actions.stroke_down").into_owned()
    }

    pub fn annotation_edit_text() -> String {
        t!("overlay.annotation.actions.edit_text").into_owned()
    }

    pub fn annotation_mosaic_mode_pixelate() -> String {
        t!("overlay.annotation.actions.mosaic_mode_pixelate").into_owned()
    }

    pub fn annotation_mosaic_mode_blur() -> String {
        t!("overlay.annotation.actions.mosaic_mode_blur").into_owned()
    }

    pub fn annotation_mosaic_intensity_up() -> String {
        t!("overlay.annotation.actions.mosaic_intensity_up").into_owned()
    }

    pub fn annotation_mosaic_intensity_down() -> String {
        t!("overlay.annotation.actions.mosaic_intensity_down").into_owned()
    }

    pub fn qr_not_found() -> String {
        t!("overlay.notify.qr_not_found").into_owned()
    }

    pub fn long_capture_processing() -> String {
        t!("overlay.long_capture.processing").into_owned()
    }

    pub fn long_capture_scroll_hint() -> String {
        t!("overlay.long_capture.scroll_hint").into_owned()
    }

    pub fn long_capture_empty() -> String {
        t!("overlay.long_capture.empty").into_owned()
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

    pub fn copied_text() -> String {
        t!("notify.capture.copied_text").into_owned()
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

    pub fn page_general() -> String {
        t!("preferences.pages.general.title").into_owned()
    }

    pub fn page_general_description() -> String {
        t!("preferences.pages.general.description").into_owned()
    }

    pub fn page_shortcuts() -> String {
        t!("preferences.pages.shortcuts.title").into_owned()
    }

    pub fn page_shortcuts_description() -> String {
        t!("preferences.pages.shortcuts.description").into_owned()
    }

    pub fn page_notifications() -> String {
        t!("preferences.pages.notifications.title").into_owned()
    }

    pub fn page_notifications_description() -> String {
        t!("preferences.pages.notifications.description").into_owned()
    }

    pub fn page_ocr() -> String {
        t!("preferences.pages.ocr.title").into_owned()
    }

    pub fn page_ocr_description() -> String {
        t!("preferences.pages.ocr.description").into_owned()
    }

    pub fn page_about() -> String {
        t!("preferences.pages.about.title").into_owned()
    }

    pub fn page_about_description() -> String {
        t!("preferences.pages.about.description").into_owned()
    }

    pub fn auto_start() -> String {
        t!("preferences.fields.auto_start").into_owned()
    }

    pub fn auto_start_description() -> String {
        t!("preferences.fields.auto_start_description").into_owned()
    }

    pub fn language() -> String {
        t!("preferences.fields.language").into_owned()
    }

    pub fn language_description() -> String {
        t!("preferences.fields.language_description").into_owned()
    }

    pub fn theme() -> String {
        t!("preferences.fields.theme").into_owned()
    }

    pub fn theme_description() -> String {
        t!("preferences.fields.theme_description").into_owned()
    }

    pub fn font_family() -> String {
        t!("preferences.fields.font_family").into_owned()
    }

    pub fn font_family_description() -> String {
        t!("preferences.fields.font_family_description").into_owned()
    }

    pub fn ocr() -> String {
        t!("preferences.fields.ocr").into_owned()
    }

    pub fn ocr_model() -> String {
        t!("preferences.fields.ocr_model").into_owned()
    }

    pub fn notifications() -> String {
        t!("preferences.fields.notifications").into_owned()
    }

    pub fn notifications_enabled() -> String {
        t!("preferences.fields.notifications_enabled").into_owned()
    }

    pub fn notifications_enabled_description() -> String {
        t!("preferences.fields.notifications_enabled_description").into_owned()
    }

    pub fn save_notification() -> String {
        t!("preferences.fields.save_notification").into_owned()
    }

    pub fn save_notification_description() -> String {
        t!("preferences.fields.save_notification_description").into_owned()
    }

    pub fn copy_notification() -> String {
        t!("preferences.fields.copy_notification").into_owned()
    }

    pub fn copy_notification_description() -> String {
        t!("preferences.fields.copy_notification_description").into_owned()
    }

    pub fn qr_code_notification() -> String {
        t!("preferences.fields.qr_code_notification").into_owned()
    }

    pub fn qr_code_notification_description() -> String {
        t!("preferences.fields.qr_code_notification_description").into_owned()
    }

    pub fn shutter_sound() -> String {
        t!("preferences.fields.shutter_sound").into_owned()
    }

    pub fn shutter_sound_description() -> String {
        t!("preferences.fields.shutter_sound_description").into_owned()
    }

    pub fn save_path() -> String {
        t!("preferences.fields.save_path").into_owned()
    }

    pub fn save_directory() -> String {
        t!("preferences.fields.save_directory").into_owned()
    }

    pub fn image_compression() -> String {
        t!("preferences.fields.image_compression").into_owned()
    }

    pub fn image_compression_description() -> String {
        t!("preferences.fields.image_compression_description").into_owned()
    }

    pub fn capture_shortcut() -> String {
        t!("preferences.fields.capture_shortcut").into_owned()
    }

    pub fn quick_capture_shortcut() -> String {
        t!("preferences.fields.quick_capture_shortcut").into_owned()
    }

    pub fn capture_shortcut_description() -> String {
        t!("preferences.fields.capture_shortcut_description").into_owned()
    }

    pub fn quick_capture_shortcut_description() -> String {
        t!("preferences.fields.quick_capture_shortcut_description").into_owned()
    }

    pub fn ocr_enabled_description() -> String {
        t!("preferences.fields.ocr_enabled_description").into_owned()
    }

    pub fn ocr_model_type() -> String {
        t!("preferences.fields.ocr_model_type").into_owned()
    }

    pub fn ocr_model_type_mobile() -> String {
        t!("preferences.fields.ocr_model_type_mobile").into_owned()
    }

    pub fn default_path() -> String {
        t!("preferences.fields.default_path").into_owned()
    }

    pub fn default_path_with_value(path: impl ToString) -> String {
        t!("preferences.fields.default_path_with_value", path = path.to_string()).into_owned()
    }

    pub fn intro() -> String {
        t!("preferences.description.intro").into_owned()
    }

    pub fn select_save_directory() -> String {
        t!("preferences.actions.select_save_directory").into_owned()
    }

    pub fn browse() -> String {
        t!("preferences.actions.browse").into_owned()
    }

    pub fn open() -> String {
        t!("preferences.actions.open").into_owned()
    }

    pub fn shortcuts_recording() -> String {
        t!("preferences.actions.shortcuts_recording").into_owned()
    }

    pub fn shortcuts_recording_hint() -> String {
        t!("preferences.actions.shortcuts_recording_hint").into_owned()
    }

    pub fn shortcuts_restore_defaults() -> String {
        t!("preferences.actions.shortcuts_restore_defaults").into_owned()
    }

    pub fn shortcuts_restore_defaults_description() -> String {
        t!("preferences.actions.shortcuts_restore_defaults_description").into_owned()
    }

    pub fn ocr_download_action() -> String {
        t!("preferences.actions.ocr_download").into_owned()
    }

    pub fn ocr_redownload_action() -> String {
        t!("preferences.actions.ocr_redownload").into_owned()
    }

    pub fn ocr_download_in_progress() -> String {
        t!("preferences.actions.ocr_download_in_progress").into_owned()
    }

    pub fn follow_system() -> String {
        t!("preferences.options.follow_system").into_owned()
    }

    pub fn theme_light() -> String {
        t!("preferences.options.light").into_owned()
    }

    pub fn theme_dark() -> String {
        t!("preferences.options.dark").into_owned()
    }

    pub fn language_zh_cn() -> String {
        t!("preferences.options.zh_cn").into_owned()
    }

    pub fn language_en_us() -> String {
        t!("preferences.options.en_us").into_owned()
    }

    pub fn about_summary() -> String {
        t!("preferences.about.summary").into_owned()
    }

    pub fn version_label() -> String {
        t!("preferences.about.version").into_owned()
    }

    pub fn github_repository() -> String {
        t!("preferences.about.github_repository").into_owned()
    }

    pub fn github_repository_description() -> String {
        t!("preferences.about.github_repository_description").into_owned()
    }

    pub fn report_issue() -> String {
        t!("preferences.about.report_issue").into_owned()
    }

    pub fn report_issue_description() -> String {
        t!("preferences.about.report_issue_description").into_owned()
    }

    pub fn open_log_folder() -> String {
        t!("preferences.about.open_log_folder").into_owned()
    }

    pub fn shortcuts_title() -> String {
        t!("preferences.shortcuts.title").into_owned()
    }

    pub fn shortcuts_conflict() -> String {
        t!("preferences.shortcuts.conflict").into_owned()
    }

    pub fn ocr_status_missing() -> String {
        t!("preferences.ocr.status_missing").into_owned()
    }

    pub fn ocr_status_downloading(progress: u8) -> String {
        t!("preferences.ocr.status_downloading", progress = progress).into_owned()
    }

    pub fn ocr_status_ready() -> String {
        t!("preferences.ocr.status_ready").into_owned()
    }

    pub fn ocr_status_failed(message: impl ToString) -> String {
        t!("preferences.ocr.status_failed", message = message.to_string()).into_owned()
    }

    pub fn ocr_download_completed() -> String {
        t!("preferences.ocr.download_completed").into_owned()
    }

    pub fn ocr_download_failed() -> String {
        t!("preferences.ocr.download_failed").into_owned()
    }

    pub fn ocr_note() -> String {
        t!("preferences.ocr.note").into_owned()
    }

    pub fn folder_picker_failed() -> String {
        t!("preferences.errors.folder_picker_failed").into_owned()
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

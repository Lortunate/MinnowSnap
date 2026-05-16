use crate::services::{hotkeys, paths::ensure_parent_dir};
use config::{Config, File};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use tracing::{error, info};

static SETTINGS: LazyLock<Mutex<SettingsStore>> = LazyLock::new(|| Mutex::new(SettingsStore::new()));

pub const THEME_SYSTEM: &str = "System";
pub const THEME_LIGHT: &str = "Light";
pub const THEME_DARK: &str = "Dark";

pub fn snapshot() -> AppSettings {
    SETTINGS.lock().map(|guard| guard.get()).unwrap_or_default()
}

pub fn general_settings() -> GeneralSettings {
    snapshot().general
}

pub fn output_settings() -> OutputSettings {
    snapshot().output
}

pub fn shortcut_settings() -> ShortcutSettings {
    snapshot().shortcuts
}

pub fn ocr_settings() -> OcrSettings {
    snapshot().ocr
}

pub fn notification_settings() -> NotificationSettings {
    snapshot().notification
}

pub fn language() -> String {
    general_settings().language
}

pub fn auto_start_enabled() -> bool {
    general_settings().auto_start
}

pub fn apply(action: SettingsAction) {
    SETTINGS.lock().unwrap().apply(action);
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SettingsAction {
    SetSavePath(String),
    SetOxipngEnabled(bool),
    SetFontFamily(String),
    SetTheme(String),
    SetLanguage(String),
    SetAutoStart(bool),
    SetShortcuts { capture: String, quick_capture: String },
    SetOcrEnabled(bool),
    SetNotificationEnabled(bool),
    SetSaveNotification(bool),
    SetCopyNotification(bool),
    SetQrCodeNotification(bool),
    SetShutterSound(bool),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct GeneralSettings {
    pub theme: String,
    pub language: String,
    pub font_family: Option<String>,
    pub auto_start: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            theme: THEME_SYSTEM.to_string(),
            language: "System".to_string(),
            font_family: None,
            auto_start: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct ShortcutSettings {
    pub capture: String,
    pub quick_capture: String,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            capture: hotkeys::DEFAULT_CAPTURE_SHORTCUT.to_string(),
            quick_capture: hotkeys::DEFAULT_QUICK_CAPTURE_SHORTCUT.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct OutputSettings {
    pub save_path: Option<String>,
    pub oxipng_enabled: bool,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            save_path: None,
            oxipng_enabled: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct OcrSettings {
    pub enabled: bool,
    pub model_type: String,
}

impl Default for OcrSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            model_type: "Mobile".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct NotificationSettings {
    pub enabled: bool,
    pub save_notification: bool,
    pub copy_notification: bool,
    pub qr_code_notification: bool,
    pub shutter_sound: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            save_notification: true,
            copy_notification: true,
            qr_code_notification: true,
            shutter_sound: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct AppSettings {
    pub general: GeneralSettings,
    pub shortcuts: ShortcutSettings,
    pub output: OutputSettings,
    pub ocr: OcrSettings,
    pub notification: NotificationSettings,
}

pub struct SettingsStore {
    config: AppSettings,
    config_path: PathBuf,
    #[cfg(test)]
    save_count: usize,
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsStore {
    fn new() -> Self {
        let (config, config_path) = Self::load_config();
        Self {
            config,
            config_path,
            #[cfg(test)]
            save_count: 0,
        }
    }

    fn get_config_path() -> PathBuf {
        crate::services::paths::app_paths().config_file().to_path_buf()
    }

    fn load_config() -> (AppSettings, PathBuf) {
        let config_path = Self::get_config_path();

        if !config_path.exists() {
            if let Err(e) = ensure_parent_dir(&config_path) {
                error!("Failed to create config directory: {}", e);
            }
            return (AppSettings::default(), config_path);
        }

        let s = Config::builder().add_source(File::from(config_path.clone())).build();

        match s {
            Ok(s) => match s.try_deserialize() {
                Ok(c) => {
                    info!("Config loaded successfully from {:?}", config_path);
                    (c, config_path)
                }
                Err(e) => {
                    error!("Failed to parse config file: {}", e);
                    (AppSettings::default(), config_path)
                }
            },
            Err(e) => {
                error!("Failed to load config: {}", e);
                (AppSettings::default(), config_path)
            }
        }
    }

    fn get(&self) -> AppSettings {
        self.config.clone()
    }

    fn update<F: FnOnce(&mut AppSettings)>(&mut self, f: F) {
        f(&mut self.config);
        self.save();
    }

    pub fn apply(&mut self, action: SettingsAction) {
        match action {
            SettingsAction::SetSavePath(path) => self.set_save_path(path),
            SettingsAction::SetOxipngEnabled(enabled) => self.set_oxipng_enabled(enabled),
            SettingsAction::SetFontFamily(font_family) => self.set_font_family(font_family),
            SettingsAction::SetTheme(theme) => self.set_theme(theme),
            SettingsAction::SetLanguage(language) => self.set_language(language),
            SettingsAction::SetAutoStart(enabled) => self.set_auto_start(enabled),
            SettingsAction::SetShortcuts { capture, quick_capture } => self.set_shortcuts(capture, quick_capture),
            SettingsAction::SetOcrEnabled(enabled) => self.set_ocr_enabled(enabled),
            SettingsAction::SetNotificationEnabled(enabled) => self.set_notification_enabled(enabled),
            SettingsAction::SetSaveNotification(enabled) => self.set_save_notification(enabled),
            SettingsAction::SetCopyNotification(enabled) => self.set_copy_notification(enabled),
            SettingsAction::SetQrCodeNotification(enabled) => self.set_qr_code_notification(enabled),
            SettingsAction::SetShutterSound(enabled) => self.set_shutter_sound(enabled),
        }
    }

    fn set_save_path(&mut self, path: String) {
        self.update(|c| c.output.save_path = if path.is_empty() { None } else { Some(path) });
    }

    fn set_oxipng_enabled(&mut self, enabled: bool) {
        self.update(|c| c.output.oxipng_enabled = enabled);
    }

    fn set_font_family(&mut self, font_family: String) {
        self.update(|c| c.general.font_family = if font_family.is_empty() { None } else { Some(font_family) });
    }

    fn set_theme(&mut self, theme: String) {
        self.update(|c| c.general.theme = theme);
    }

    fn set_language(&mut self, language: String) {
        self.update(|c| c.general.language = language);
    }

    fn set_auto_start(&mut self, enabled: bool) {
        self.update(|c| c.general.auto_start = enabled);
    }

    fn set_shortcuts(&mut self, capture: String, quick_capture: String) {
        self.update(|c| {
            c.shortcuts.capture = capture;
            c.shortcuts.quick_capture = quick_capture;
        });
    }

    fn set_ocr_enabled(&mut self, enabled: bool) {
        self.update(|c| c.ocr.enabled = enabled);
    }

    fn set_notification_enabled(&mut self, enabled: bool) {
        self.update(|c| c.notification.enabled = enabled);
    }

    fn set_save_notification(&mut self, enabled: bool) {
        self.update(|c| c.notification.save_notification = enabled);
    }

    fn set_copy_notification(&mut self, enabled: bool) {
        self.update(|c| c.notification.copy_notification = enabled);
    }

    fn set_qr_code_notification(&mut self, enabled: bool) {
        self.update(|c| c.notification.qr_code_notification = enabled);
    }

    fn set_shutter_sound(&mut self, enabled: bool) {
        self.update(|c| c.notification.shutter_sound = enabled);
    }

    fn save(&mut self) {
        #[cfg(test)]
        {
            self.save_count += 1;
        }

        let config = self.config.clone();
        let path = self.config_path.clone();

        crate::RUNTIME.spawn_blocking(move || match toml::to_string_pretty(&config) {
            Ok(toml_string) => {
                if let Err(e) = ensure_parent_dir(&path) {
                    error!("Failed to create config directory for {:?}: {}", path, e);
                    return;
                }
                if let Err(e) = fs::write(&path, toml_string) {
                    error!("Failed to write config file to {:?}: {}", path, e);
                } else {
                    info!("Settings saved to {:?}", path);
                }
            }
            Err(e) => error!("Failed to serialize config: {}", e),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> SettingsStore {
        SettingsStore {
            config: AppSettings::default(),
            config_path: std::env::temp_dir().join("minnowsnap-settings-test.toml"),
            save_count: 0,
        }
    }

    #[test]
    fn set_shortcuts_updates_both_bindings_with_one_save() {
        let mut store = test_store();

        store.apply(SettingsAction::SetShortcuts {
            capture: "Ctrl+Shift+1".to_string(),
            quick_capture: "Ctrl+Shift+2".to_string(),
        });

        let settings = store.get();
        assert_eq!(settings.shortcuts.capture, "Ctrl+Shift+1");
        assert_eq!(settings.shortcuts.quick_capture, "Ctrl+Shift+2");
        assert_eq!(store.save_count, 1);
    }

    #[test]
    fn settings_action_updates_general_and_output_settings() {
        let mut store = test_store();

        store.apply(SettingsAction::SetTheme("Dark".to_string()));
        store.apply(SettingsAction::SetLanguage("en_US".to_string()));
        store.apply(SettingsAction::SetFontFamily("JetBrains Mono".to_string()));
        store.apply(SettingsAction::SetSavePath("D:/captures".to_string()));
        store.apply(SettingsAction::SetOxipngEnabled(false));

        let settings = store.get();
        assert_eq!(settings.general.theme, "Dark");
        assert_eq!(settings.general.language, "en_US");
        assert_eq!(settings.general.font_family.as_deref(), Some("JetBrains Mono"));
        assert_eq!(settings.output.save_path.as_deref(), Some("D:/captures"));
        assert!(!settings.output.oxipng_enabled);
        assert_eq!(store.save_count, 5);
    }

    #[test]
    fn settings_action_reset_empty_optional_values() {
        let mut store = test_store();

        store.apply(SettingsAction::SetFontFamily(String::new()));
        store.apply(SettingsAction::SetSavePath(String::new()));

        let settings = store.get();
        assert_eq!(settings.general.font_family, None);
        assert_eq!(settings.output.save_path, None);
    }

    #[test]
    fn default_shortcuts_stay_aligned_with_hotkeys_constants() {
        let settings = ShortcutSettings::default();

        assert_eq!(settings.capture, hotkeys::DEFAULT_CAPTURE_SHORTCUT);
        assert_eq!(settings.quick_capture, hotkeys::DEFAULT_QUICK_CAPTURE_SHORTCUT);
    }
}

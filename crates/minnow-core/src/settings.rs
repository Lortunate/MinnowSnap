use crate::paths::ensure_parent_dir;
use config::{Config, File};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use tracing::{error, info};

pub static SETTINGS: LazyLock<Mutex<SettingsManager>> = LazyLock::new(|| Mutex::new(SettingsManager::new()));

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
            theme: "System".to_string(),
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
            capture: "F1".to_string(),
            quick_capture: "F2".to_string(),
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

pub struct SettingsManager {
    config: AppSettings,
    config_path: PathBuf,
}

impl Default for SettingsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsManager {
    pub fn new() -> Self {
        let (config, config_path) = Self::load_config();
        Self { config, config_path }
    }

    fn get_config_path() -> PathBuf {
        minnow_paths::app_paths().config_file().to_path_buf()
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

    pub fn get(&self) -> AppSettings {
        self.config.clone()
    }

    fn update<F: FnOnce(&mut AppSettings)>(&mut self, f: F) {
        f(&mut self.config);
        self.save();
    }

    pub fn set_save_path(&mut self, path: String) {
        self.update(|c| c.output.save_path = if path.is_empty() { None } else { Some(path) });
    }

    pub fn set_oxipng_enabled(&mut self, enabled: bool) {
        self.update(|c| c.output.oxipng_enabled = enabled);
    }

    pub fn set_font_family(&mut self, font_family: String) {
        self.update(|c| c.general.font_family = if font_family.is_empty() { None } else { Some(font_family) });
    }

    pub fn set_theme(&mut self, theme: String) {
        self.update(|c| c.general.theme = theme);
    }

    pub fn set_language(&mut self, language: String) {
        self.update(|c| c.general.language = language);
    }

    pub fn set_auto_start(&mut self, enabled: bool) {
        self.update(|c| c.general.auto_start = enabled);
    }

    pub fn set_capture_shortcut(&mut self, shortcut: String) {
        self.update(|c| c.shortcuts.capture = shortcut);
    }

    pub fn set_quick_capture_shortcut(&mut self, shortcut: String) {
        self.update(|c| c.shortcuts.quick_capture = shortcut);
    }

    pub fn set_ocr_enabled(&mut self, enabled: bool) {
        self.update(|c| c.ocr.enabled = enabled);
    }

    pub fn set_notification_enabled(&mut self, enabled: bool) {
        self.update(|c| c.notification.enabled = enabled);
    }

    pub fn set_save_notification(&mut self, enabled: bool) {
        self.update(|c| c.notification.save_notification = enabled);
    }

    pub fn set_copy_notification(&mut self, enabled: bool) {
        self.update(|c| c.notification.copy_notification = enabled);
    }

    pub fn set_qr_code_notification(&mut self, enabled: bool) {
        self.update(|c| c.notification.qr_code_notification = enabled);
    }

    pub fn set_shutter_sound(&mut self, enabled: bool) {
        self.update(|c| c.notification.shutter_sound = enabled);
    }

    fn save(&self) {
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

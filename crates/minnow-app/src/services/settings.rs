mod persistence;

use crate::services::{hotkeys, paths::ensure_parent_dir};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, MutexGuard};
use tracing::{error, info};

use persistence::SettingsPersistence;

static SETTINGS: LazyLock<Mutex<SettingsStore>> = LazyLock::new(|| Mutex::new(SettingsStore::new()));

pub const THEME_SYSTEM: &str = "System";
pub const THEME_LIGHT: &str = "Light";
pub const THEME_DARK: &str = "Dark";

fn settings_guard() -> MutexGuard<'static, SettingsStore> {
    match SETTINGS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            error!("Settings lock was poisoned; recovering the latest in-memory state");
            let guard = poisoned.into_inner();
            SETTINGS.clear_poison();
            guard
        }
    }
}

pub fn snapshot() -> AppSettings {
    settings_guard().get()
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
    settings_guard().apply(action);
}

pub fn flush() {
    if let Err(err) = settings_guard().flush() {
        error!("Failed to flush the latest settings snapshot: {err}");
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SettingsAction {
    SavePath(String),
    OxipngEnabled(bool),
    FontFamily(String),
    Theme(String),
    Language(String),
    AutoStart(bool),
    Shortcuts { capture: String, quick_capture: String },
    OcrEnabled(bool),
    NotificationEnabled(bool),
    SaveNotification(bool),
    CopyNotification(bool),
    QrCodeNotification(bool),
    ShutterSound(bool),
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
    persistence: SettingsPersistence,
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
            persistence: SettingsPersistence::new(),
            #[cfg(test)]
            save_count: 0,
        }
    }

    fn get_config_path() -> PathBuf {
        crate::services::paths::app_paths().config_file().to_path_buf()
    }

    fn load_config() -> (AppSettings, PathBuf) {
        let config_path = Self::get_config_path();
        let config = Self::load_config_from(&config_path);
        (config, config_path)
    }

    fn load_config_from(config_path: &Path) -> AppSettings {
        let contents = match fs::read_to_string(config_path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                if let Err(err) = ensure_parent_dir(config_path) {
                    error!("Failed to create config directory: {err}");
                }
                return AppSettings::default();
            }
            Err(err) => {
                error!("Failed to read config file {}: {err}", config_path.display());
                return AppSettings::default();
            }
        };

        match toml::from_str(&contents) {
            Ok(config) => {
                info!("Config loaded successfully from {:?}", config_path);
                config
            }
            Err(err) => {
                error!("Failed to parse config file {}: {err}", config_path.display());
                AppSettings::default()
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
            SettingsAction::SavePath(path) => {
                self.update(|c| c.output.save_path = if path.is_empty() { None } else { Some(path) });
            }
            SettingsAction::OxipngEnabled(enabled) => {
                self.update(|c| c.output.oxipng_enabled = enabled);
            }
            SettingsAction::FontFamily(font_family) => {
                self.update(|c| {
                    c.general.font_family = if font_family.is_empty() { None } else { Some(font_family) };
                });
            }
            SettingsAction::Theme(theme) => {
                self.update(|c| c.general.theme = theme);
            }
            SettingsAction::Language(language) => {
                self.update(|c| c.general.language = language);
            }
            SettingsAction::AutoStart(enabled) => {
                self.update(|c| c.general.auto_start = enabled);
            }
            SettingsAction::Shortcuts { capture, quick_capture } => {
                self.update(|c| {
                    c.shortcuts.capture = capture;
                    c.shortcuts.quick_capture = quick_capture;
                });
            }
            SettingsAction::OcrEnabled(enabled) => {
                self.update(|c| c.ocr.enabled = enabled);
            }
            SettingsAction::NotificationEnabled(enabled) => {
                self.update(|c| c.notification.enabled = enabled);
            }
            SettingsAction::SaveNotification(enabled) => {
                self.update(|c| c.notification.save_notification = enabled);
            }
            SettingsAction::CopyNotification(enabled) => {
                self.update(|c| c.notification.copy_notification = enabled);
            }
            SettingsAction::QrCodeNotification(enabled) => {
                self.update(|c| c.notification.qr_code_notification = enabled);
            }
            SettingsAction::ShutterSound(enabled) => {
                self.update(|c| c.notification.shutter_sound = enabled);
            }
        }
    }

    fn save(&mut self) {
        #[cfg(test)]
        {
            self.save_count += 1;
        }

        self.persistence.enqueue(self.config.clone(), self.config_path.clone());
    }

    fn flush(&self) -> Result<(), String> {
        self.persistence.flush_latest(&self.config, &self.config_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_STORE_ID: AtomicU64 = AtomicU64::new(0);

    fn test_store() -> SettingsStore {
        let id = TEST_STORE_ID.fetch_add(1, Ordering::Relaxed);
        SettingsStore {
            config: AppSettings::default(),
            config_path: std::env::temp_dir().join(format!("minnowsnap-settings-test-{}-{id}.toml", std::process::id())),
            persistence: SettingsPersistence::new(),
            save_count: 0,
        }
    }

    fn cleanup_store(store: SettingsStore) {
        let path = store.config_path.clone();
        drop(store);
        let _ = std::fs::remove_file(path);
    }

    fn test_config_path(label: &str) -> PathBuf {
        let id = TEST_STORE_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!("minnowsnap-settings-load-{label}-{}-{id}", std::process::id()))
            .join("settings.toml")
    }

    fn cleanup_config_path(path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn missing_config_returns_defaults_and_prepares_parent_directory() {
        let path = test_config_path("missing");

        let settings = SettingsStore::load_config_from(&path);

        assert_eq!(settings.general.theme, THEME_SYSTEM);
        assert_eq!(settings.shortcuts.capture, hotkeys::DEFAULT_CAPTURE_SHORTCUT);
        assert!(path.parent().is_some_and(Path::exists));
        cleanup_config_path(&path);
    }

    #[test]
    fn malformed_config_returns_defaults() {
        let path = test_config_path("malformed");
        ensure_parent_dir(&path).expect("create config test directory");
        std::fs::write(&path, "general = [").expect("write malformed config");

        let settings = SettingsStore::load_config_from(&path);

        assert_eq!(settings.general.theme, THEME_SYSTEM);
        assert_eq!(settings.general.language, "System");
        assert!(settings.notification.enabled);
        cleanup_config_path(&path);
    }

    #[test]
    fn partial_config_uses_section_defaults() {
        let path = test_config_path("partial");
        ensure_parent_dir(&path).expect("create config test directory");
        std::fs::write(&path, "[general]\ntheme = \"Dark\"\n").expect("write partial config");

        let settings = SettingsStore::load_config_from(&path);

        assert_eq!(settings.general.theme, THEME_DARK);
        assert_eq!(settings.general.language, "System");
        assert_eq!(settings.shortcuts.capture, hotkeys::DEFAULT_CAPTURE_SHORTCUT);
        assert!(settings.output.oxipng_enabled);
        assert!(settings.notification.enabled);
        cleanup_config_path(&path);
    }

    #[test]
    fn set_shortcuts_updates_both_bindings_with_one_save() {
        let mut store = test_store();

        store.apply(SettingsAction::Shortcuts {
            capture: "Ctrl+Shift+1".to_string(),
            quick_capture: "Ctrl+Shift+2".to_string(),
        });

        let settings = store.get();
        assert_eq!(settings.shortcuts.capture, "Ctrl+Shift+1");
        assert_eq!(settings.shortcuts.quick_capture, "Ctrl+Shift+2");
        assert_eq!(store.save_count, 1);
        cleanup_store(store);
    }

    #[test]
    fn settings_action_updates_general_and_output_settings() {
        let mut store = test_store();

        store.apply(SettingsAction::Theme("Dark".to_string()));
        store.apply(SettingsAction::Language("en_US".to_string()));
        store.apply(SettingsAction::FontFamily("JetBrains Mono".to_string()));
        store.apply(SettingsAction::SavePath("D:/captures".to_string()));
        store.apply(SettingsAction::OxipngEnabled(false));

        let settings = store.get();
        assert_eq!(settings.general.theme, "Dark");
        assert_eq!(settings.general.language, "en_US");
        assert_eq!(settings.general.font_family.as_deref(), Some("JetBrains Mono"));
        assert_eq!(settings.output.save_path.as_deref(), Some("D:/captures"));
        assert!(!settings.output.oxipng_enabled);
        assert_eq!(store.save_count, 5);
        cleanup_store(store);
    }

    #[test]
    fn settings_action_reset_empty_optional_values() {
        let mut store = test_store();

        store.apply(SettingsAction::FontFamily(String::new()));
        store.apply(SettingsAction::SavePath(String::new()));

        let settings = store.get();
        assert_eq!(settings.general.font_family, None);
        assert_eq!(settings.output.save_path, None);
        cleanup_store(store);
    }

    #[test]
    fn default_shortcuts_stay_aligned_with_hotkeys_constants() {
        let settings = ShortcutSettings::default();

        assert_eq!(settings.capture, hotkeys::DEFAULT_CAPTURE_SHORTCUT);
        assert_eq!(settings.quick_capture, hotkeys::DEFAULT_QUICK_CAPTURE_SHORTCUT);
    }
}

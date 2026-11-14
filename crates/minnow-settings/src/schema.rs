use crate::SaveDirectory;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

pub const THEME_SYSTEM: &str = "System";
pub const THEME_LIGHT: &str = "Light";
pub const THEME_DARK: &str = "Dark";
pub const DEFAULT_CAPTURE_SHORTCUT: &str = "F1";
pub const DEFAULT_QUICK_CAPTURE_SHORTCUT: &str = "F2";

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SettingsAction {
    SavePath(SaveDirectory),
    OxipngEnabled(bool),
    FontFamily(String),
    Theme(String),
    Language(String),
    AutoStart(bool),
    CaptureShortcut(String),
    QuickCaptureShortcut(String),
    Shortcuts { capture: String, quick_capture: String },
    OcrEnabled(bool),
    NotificationEnabled(bool),
    SaveNotification(bool),
    CopyNotification(bool),
    QrCodeNotification(bool),
    ShutterSound(bool),
}

/// The supported OCR model families. Persisted values retain their historical names.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub enum OcrModelType {
    #[default]
    Mobile,
    Server,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("unknown OCR model type {0:?}; expected Mobile or Server")]
pub struct ParseOcrModelTypeError(String);

impl FromStr for OcrModelType {
    type Err = ParseOcrModelTypeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "mobile" => Ok(Self::Mobile),
            "server" => Ok(Self::Server),
            _ => Err(ParseOcrModelTypeError(value.to_owned())),
        }
    }
}

impl fmt::Display for OcrModelType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Mobile => "Mobile",
            Self::Server => "Server",
        })
    }
}

impl<'de> Deserialize<'de> for OcrModelType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
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

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(default)]
pub struct ShortcutSettings {
    pub capture: String,
    pub quick_capture: String,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            capture: DEFAULT_CAPTURE_SHORTCUT.to_string(),
            quick_capture: DEFAULT_QUICK_CAPTURE_SHORTCUT.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(default)]
pub struct OutputSettings {
    #[serde(skip_serializing_if = "SaveDirectory::is_default")]
    pub save_path: SaveDirectory,
    pub oxipng_enabled: bool,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            save_path: SaveDirectory::default(),
            oxipng_enabled: true,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(default)]
pub struct OcrSettings {
    pub enabled: bool,
    pub model_type: OcrModelType,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
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

#[derive(Debug, Serialize, Deserialize, Clone, Default, Eq, PartialEq)]
#[serde(default)]
pub struct AppSettings {
    pub general: GeneralSettings,
    pub shortcuts: ShortcutSettings,
    pub output: OutputSettings,
    pub ocr: OcrSettings,
    pub notification: NotificationSettings,
}

impl AppSettings {
    /// Apply a desired preference to this value, without performing I/O or OS operations.
    pub fn apply(&mut self, action: SettingsAction) {
        match action {
            SettingsAction::SavePath(directory) => self.output.save_path = directory,
            SettingsAction::OxipngEnabled(enabled) => self.output.oxipng_enabled = enabled,
            SettingsAction::FontFamily(font_family) => {
                self.general.font_family = if font_family.is_empty() { None } else { Some(font_family) };
            }
            SettingsAction::Theme(theme) => self.general.theme = theme,
            SettingsAction::Language(language) => self.general.language = language,
            SettingsAction::AutoStart(enabled) => self.general.auto_start = enabled,
            SettingsAction::CaptureShortcut(shortcut) => self.shortcuts.capture = shortcut,
            SettingsAction::QuickCaptureShortcut(shortcut) => self.shortcuts.quick_capture = shortcut,
            SettingsAction::Shortcuts { capture, quick_capture } => {
                self.shortcuts.capture = capture;
                self.shortcuts.quick_capture = quick_capture;
            }
            SettingsAction::OcrEnabled(enabled) => self.ocr.enabled = enabled,
            SettingsAction::NotificationEnabled(enabled) => self.notification.enabled = enabled,
            SettingsAction::SaveNotification(enabled) => self.notification.save_notification = enabled,
            SettingsAction::CopyNotification(enabled) => self.notification.copy_notification = enabled,
            SettingsAction::QrCodeNotification(enabled) => self.notification.qr_code_notification = enabled,
            SettingsAction::ShutterSound(enabled) => self.notification.shutter_sound = enabled,
        }
    }
}

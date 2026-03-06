use crate::core::app::{APP_ID, APP_NAME};
use crate::core::settings::SETTINGS;
use tracing::error;

#[cfg(not(target_os = "windows"))]
use notify_rust::Notification;
#[cfg(target_os = "windows")]
use tauri_winrt_notification::{IconCrop, Toast};

#[cfg(target_os = "windows")]
use std::{fs, path::PathBuf};
#[cfg(target_os = "windows")]
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

#[cfg(target_os = "windows")]
const WINDOWS_TOAST_ICON_FILE: &str = "minnowsnap-toast-icon.png";
#[cfg(target_os = "windows")]
const WINDOWS_TOAST_ICON_BYTES: &[u8] = include_bytes!("../../resources/logo.png");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationType {
    Save,
    Copy,
    QrCode,
    Info,
}

pub fn play_shutter() {
    let settings = match SETTINGS.lock() {
        Ok(guard) => guard.get(),
        Err(_) => return,
    };

    if !settings.notification.shutter_sound {
        return;
    }

    crate::core::RUNTIME.spawn_blocking(|| {
        let sound_data = include_bytes!("../../resources/raw/capture.mp3");
        let cursor = std::io::Cursor::new(&sound_data[..]);
        match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(handle) => {
                let player = rodio::Player::connect_new(handle.mixer());
                match rodio::Decoder::new(cursor) {
                    Ok(source) => {
                        player.append(source);
                        player.sleep_until_end();
                    }
                    Err(e) => tracing::error!("Failed to decode audio stream: {}", e),
                }
            }
            Err(e) => tracing::error!("Failed to open default audio stream: {}", e),
        }
    });
}

#[cfg(target_os = "windows")]
fn ensure_windows_toast_icon_file() -> Option<PathBuf> {
    let path = std::env::temp_dir().join(WINDOWS_TOAST_ICON_FILE);
    
    let needs_update = fs::metadata(&path)
        .map(|m| m.len() != WINDOWS_TOAST_ICON_BYTES.len() as u64)
        .unwrap_or(true);

    if needs_update {
        if let Err(e) = fs::write(&path, WINDOWS_TOAST_ICON_BYTES) {
            error!("Failed to write toast icon file: {}", e);
            return None;
        }
    }
    
    Some(path)
}

#[cfg(target_os = "windows")]
pub fn init_windows_notification_app_id() {
    let key_path = format!(r"Software\Classes\AppUserModelId\{APP_ID}");
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    
    let Ok((key, _)) = hkcu.create_subkey(&key_path) else {
        error!("Failed to create AppUserModelId registry key: {}", key_path);
        return;
    };
    
    if let Err(e) = key.set_value("DisplayName", &APP_NAME) {
        error!("Failed to set DisplayName for AppUserModelId: {}", e);
    }
    if let Err(e) = key.set_value("IconBackgroundColor", &"0") {
        error!("Failed to set IconBackgroundColor for AppUserModelId: {}", e);
    }

    if let Some(path) = ensure_windows_toast_icon_file() {
        let icon_uri = path.to_string_lossy();
        if let Err(e) = key.set_value("IconUri", &icon_uri.as_ref()) {
            error!("Failed to set IconUri for AppUserModelId: {}", e);
        }
    } else {
        error!("Failed to prepare toast icon file for IconUri");
    }
}

pub fn show(title: &str, message: &str, type_: NotificationType) {
    let settings = match SETTINGS.lock() {
        Ok(guard) => guard.get(),
        Err(_) => return,
    };

    if !settings.notification.enabled {
        return;
    }

    let allowed = match type_ {
        NotificationType::Copy => settings.notification.copy_notification,
        NotificationType::Save => settings.notification.save_notification,
        NotificationType::QrCode => settings.notification.qr_code_notification,
        _ => true,
    };
    if !allowed {
        return;
    }

    #[cfg(target_os = "windows")]
    {
        let toast = if let Some(icon_path) = ensure_windows_toast_icon_file() {
            Toast::new(APP_ID)
                .icon(icon_path.as_path(), IconCrop::Circular, APP_NAME)
                .title(title)
                .text1(message)
        } else {
            Toast::new(APP_ID).title(title).text1(message)
        };

        if let Err(e) = toast.show() {
            error!("Failed to send notification: {}", e);
        }
    }

    #[cfg(not(target_os = "windows"))]
    if let Err(e) = Notification::new().summary(title).body(message).appname(APP_NAME).show() {
        error!("Failed to send notification: {}", e);
    }
}

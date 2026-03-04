use auto_launch::{AutoLaunchBuilder, MacOSLaunchMode};
use log::{error, info};
use single_instance::SingleInstance;
use std::env;
#[cfg(target_os = "windows")]
use std::{fs, path::PathBuf};

pub const APP_ID: &str = "com.lortunate.minnow";
pub const APP_NAME: &str = "MinnowSnap";
pub const APP_LOCK_ID: &str = "com.lortunate.minnow.lock";
pub const QML_MAIN: &str = "qrc:/qt/qml/com/lortunate/minnow/qml/main.qml";
#[cfg(target_os = "windows")]
const WINDOWS_TOAST_ICON_FILE: &str = "minnowsnap-toast-icon.png";
#[cfg(target_os = "windows")]
const WINDOWS_TOAST_ICON_BYTES: &[u8] = include_bytes!("../../resources/logo.png");

pub fn init_logger() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
}

#[cfg(target_os = "macos")]
pub fn hide_dock_icon() {
    use log::{debug, error};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        if app.setActivationPolicy(NSApplicationActivationPolicy::Accessory) {
            debug!("Successfully set activation policy to Accessory (hidden dock icon)");
        } else {
            error!("Failed to set activation policy");
        }
    } else {
        error!("Failed to get MainThreadMarker");
    }
}

pub fn ensure_single_instance(uniq_id: &str) -> bool {
    let instance = Box::new(SingleInstance::new(uniq_id).expect("Failed to create SingleInstance"));
    if instance.is_single() {
        Box::leak(instance);
        true
    } else {
        false
    }
}

pub fn get_instance_id() -> String {
    if cfg!(target_os = "macos") {
        env::temp_dir().join(APP_LOCK_ID).to_string_lossy().to_string()
    } else {
        APP_LOCK_ID.to_string()
    }
}

pub fn set_auto_start(enabled: bool) {
    let app_name = "MinnowSnap";
    let app_path = env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();

    if app_path.is_empty() {
        error!("Failed to get current executable path for auto-start");
        return;
    }

    let auto = AutoLaunchBuilder::new()
        .set_app_name(app_name)
        .set_app_path(&app_path)
        .set_macos_launch_mode(MacOSLaunchMode::LaunchAgent)
        .build()
        .expect("Failed to build AutoLaunch");

    if enabled {
        if let Err(e) = auto.enable() {
            error!("Failed to enable auto-start: {}", e);
        } else {
            info!("Auto-start enabled");
        }
    } else if let Err(e) = auto.disable() {
        error!("Failed to disable auto-start: {}", e);
    } else {
        info!("Auto-start disabled");
    }
}

#[cfg(target_os = "windows")]
fn windows_toast_icon_path() -> PathBuf {
    env::temp_dir().join(WINDOWS_TOAST_ICON_FILE)
}

#[cfg(target_os = "windows")]
fn ensure_windows_toast_icon_file() -> Option<PathBuf> {
    let icon_path = windows_toast_icon_path();
    let up_to_date = fs::metadata(&icon_path)
        .map(|m| m.len() == WINDOWS_TOAST_ICON_BYTES.len() as u64)
        .unwrap_or(false);
    if up_to_date {
        return Some(icon_path);
    }
    match fs::write(&icon_path, WINDOWS_TOAST_ICON_BYTES) {
        Ok(_) => Some(icon_path),
        Err(e) => {
            error!("Failed to write toast icon file: {e}");
            None
        }
    }
}

#[cfg(target_os = "windows")]
pub fn init_windows_notification_app_id() {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key_path = format!(r"Software\Classes\AppUserModelId\{APP_ID}");
    let Ok((key, _)) = hkcu.create_subkey(&key_path) else {
        error!("Failed to create AppUserModelId registry key: {key_path}");
        return;
    };

    if let Err(e) = key.set_value("DisplayName", &APP_NAME) {
        error!("Failed to set DisplayName for AppUserModelId: {e}");
    }

    if let Err(e) = key.set_value("IconBackgroundColor", &"0") {
        error!("Failed to set IconBackgroundColor for AppUserModelId: {e}");
    }

    match ensure_windows_toast_icon_file() {
        Some(icon_path) => {
            let icon_uri = icon_path.to_string_lossy().to_string();
            if let Err(e) = key.set_value("IconUri", &icon_uri) {
                error!("Failed to set IconUri for AppUserModelId: {e}");
            }
        }
        None => error!("Failed to prepare toast icon file for IconUri"),
    }
}

#[cfg(target_os = "windows")]
pub fn windows_notification_icon_path() -> Option<PathBuf> {
    ensure_windows_toast_icon_file()
}

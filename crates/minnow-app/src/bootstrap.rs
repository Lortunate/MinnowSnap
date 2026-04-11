use auto_launch::{AutoLaunchBuilder, MacOSLaunchMode};
use minnow_core::app_meta::{APP_LOCK_ID, APP_NAME};
#[cfg(target_os = "macos")]
use minnow_core::paths;
use single_instance::SingleInstance;
use std::env;
use tracing::{error, info};
use tracing_appender::non_blocking::WorkerGuard;

pub fn init_logger() -> Option<WorkerGuard> {
    minnow_core::logging::init_logger()
}

#[cfg(target_os = "macos")]
pub fn hide_dock_icon() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        if app.setActivationPolicy(NSApplicationActivationPolicy::Accessory) {
            tracing::debug!("Dock icon hidden");
        } else {
            error!("Failed to hide dock icon");
        }
    } else {
        error!("MainThreadMarker unavailable");
    }
}

pub fn ensure_single_instance(uniq_id: &str) -> bool {
    if let Ok(instance) = SingleInstance::new(uniq_id)
        && instance.is_single()
    {
        Box::leak(Box::new(instance));
        return true;
    }
    false
}

pub fn get_instance_id() -> String {
    #[cfg(target_os = "macos")]
    {
        let path = paths::lock_file();
        if let Err(err) = paths::ensure_parent_dir(&path) {
            error!("Failed to create single-instance lock directory for {:?}: {}", path, err);
        }
        return path.to_string_lossy().into();
    }
    #[cfg(not(target_os = "macos"))]
    return APP_LOCK_ID.to_string();
}

pub fn set_auto_start(enabled: bool) {
    let Ok(current_exe) = env::current_exe() else {
        error!("Failed to get current executable path for auto-start");
        return;
    };
    let Some(app_path) = current_exe.to_str() else {
        error!("Failed to convert executable path to string");
        return;
    };

    let Ok(auto) = AutoLaunchBuilder::new()
        .set_app_name(APP_NAME)
        .set_app_path(app_path)
        .set_macos_launch_mode(MacOSLaunchMode::LaunchAgent)
        .build()
    else {
        error!("Failed to build AutoLaunch");
        return;
    };

    let result = if enabled { auto.enable() } else { auto.disable() };
    if let Err(e) = result {
        error!("Auto-start error: {}", e);
    } else {
        info!("Auto-start set to {}", enabled);
    }
}

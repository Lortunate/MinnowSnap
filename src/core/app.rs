use auto_launch::{AutoLaunchBuilder, MacOSLaunchMode};
use single_instance::SingleInstance;
use std::env;
use tracing::{error, info};
use tracing_appender::non_blocking::WorkerGuard;

pub const APP_ID: &str = "com.lortunate.minnow";
pub const APP_NAME: &str = "MinnowSnap";
pub const APP_LOCK_ID: &str = "com.lortunate.minnow.lock";

pub fn init_logger() -> Option<WorkerGuard> {
    crate::core::logging::init_logger(APP_NAME)
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
    return env::temp_dir().join(APP_LOCK_ID).to_string_lossy().into();
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

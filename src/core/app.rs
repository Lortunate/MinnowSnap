use single_instance::SingleInstance;
use std::env;

pub const APP_ID: &str = "com.lortunate.minnow.lock";
pub const QML_MAIN: &str = "qrc:/qt/qml/com/lortunate/minnow/qml/main.qml";

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
        env::temp_dir().join(APP_ID).to_string_lossy().to_string()
    } else {
        APP_ID.to_string()
    }
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod bridge;
pub mod core;
pub mod interop;

#[cfg(target_os = "macos")]
use crate::core::app::APP_ID;
use crate::core::app::{QML_MAIN, ensure_single_instance, get_instance_id, init_logger};
use cxx::UniquePtr;
use cxx_qt::casting::Upcast;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQmlEngine, QUrl};
#[cfg(not(feature = "dhat-heap"))]
use mimalloc::MiMalloc;
use std::pin::Pin;
#[cfg(target_os = "macos")]
use tracing::error;
use tracing::info;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static GLOBAL: dhat::Alloc = dhat::Alloc;

#[cfg(not(feature = "dhat-heap"))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

struct MinnowApp {
    #[allow(dead_code)]
    engine: UniquePtr<QQmlApplicationEngine>,
    app: UniquePtr<QGuiApplication>,
}

impl MinnowApp {
    fn new() -> Self {
        let app = QGuiApplication::new();
        bridge::app::set_window_icon();
        bridge::app::set_quit_on_last_window_closed();

        let settings = core::settings::SETTINGS.lock().unwrap().get();
        bridge::app::install_translator(&settings.general.language);
        core::app::set_auto_start(settings.general.auto_start);
        core::io::fonts::preload_fonts();

        #[cfg(target_os = "macos")]
        core::app::hide_dock_icon();

        let mut engine = QQmlApplicationEngine::new();
        if let Some(mut pinned_engine) = engine.as_mut() {
            bridge::provider::register_image_provider(pinned_engine.as_mut());
            pinned_engine.as_mut().load(&QUrl::from(QML_MAIN));

            let base_engine: Pin<&mut QQmlEngine> = pinned_engine.upcast_pin();
            base_engine.on_quit(|_| bridge::app::quit_app()).release();
        }

        Self { engine, app }
    }

    fn run(&mut self) {
        if let Some(mut gui_pin) = self.app.as_mut() {
            gui_pin.as_mut().exec();
        }
    }
}

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _dhat_profiler = dhat::Profiler::new_heap();

    let _guard = init_logger();
    bridge::app::init_qt_logging();
    info!("Starting MinnowSnap...");

    if !ensure_single_instance(&get_instance_id()) {
        info!("Another instance is running, exiting.");
        return;
    }

    #[cfg(target_os = "windows")]
    core::notify::init_windows_notification_app_id();

    #[cfg(target_os = "macos")]
    if let Err(e) = notify_rust::set_application(APP_ID) {
        error!("Failed to set application: {}", e);
    }

    MinnowApp::new().run();
}

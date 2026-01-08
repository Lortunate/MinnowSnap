pub mod bridge;
pub mod core;

use crate::core::app::{ensure_single_instance, get_instance_id, init_logger, QML_MAIN};
use cxx::UniquePtr;
use cxx_qt::casting::Upcast;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQmlEngine, QUrl};
use log::info;
use mimalloc::MiMalloc;
use std::pin::Pin;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

struct MinnowApp {
    app: UniquePtr<QGuiApplication>,
    engine: UniquePtr<QQmlApplicationEngine>,
}

impl MinnowApp {
    fn new() -> Self {
        let app = QGuiApplication::new();
        bridge::app::set_quit_on_last_window_closed();

        let language = core::settings::SETTINGS.lock().unwrap().get().general.language;
        bridge::app::install_translator(&language);

        #[cfg(target_os = "macos")]
        core::app::hide_dock_icon();

        let mut engine = QQmlApplicationEngine::new();
        if let Some(mut engine_pin) = engine.as_mut() {
            bridge::provider::register_image_provider(engine_pin.as_mut());
            engine_pin.as_mut().load(&QUrl::from(QML_MAIN));

            let untyped: Pin<&mut QQmlEngine> = engine_pin.upcast_pin();
            untyped.on_quit(|_| std::process::exit(0)).release();
        }

        Self { app, engine }
    }

    fn run(&mut self) {
        if let Some(mut gui_pin) = self.app.as_mut() {
            gui_pin.as_mut().exec();
        }
    }
}

fn main() {
    init_logger();
    info!("Starting MinnowSnap...");

    if !ensure_single_instance(&get_instance_id()) {
        info!("Another instance is running, exiting.");
        return;
    }

    MinnowApp::new().run();
}

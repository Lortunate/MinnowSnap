pub mod assets;
pub mod background_host;

use crate::core::app::{ensure_single_instance, get_instance_id, init_logger, set_auto_start};
use crate::core::notify::init_windows_notification_app_id;
use crate::ui::{overlay, preferences};
use gpui::Application;
use tracing::info;

pub fn run() {
    let _guard = init_logger();
    info!("Starting MinnowSnap...");

    if !ensure_single_instance(&get_instance_id()) {
        info!("Another instance is running, exiting.");
        return;
    }

    #[cfg(target_os = "windows")]
    init_windows_notification_app_id();

    #[cfg(target_os = "macos")]
    if let Err(err) = notify_rust::set_application(APP_ID) {
        tracing::error!("Failed to set application: {err}");
    }

    #[cfg(target_os = "macos")]
    crate::core::app::hide_dock_icon();

    let app = Application::new().with_assets(assets::AppAssets);

    app.run(move |cx| {
        let locale = crate::core::settings::SETTINGS
            .lock()
            .map(|settings| settings.get().general.language)
            .unwrap_or_else(|_| crate::core::i18n::SYSTEM_LOCALE.to_string());
        crate::core::i18n::init(&locale);
        gpui_component::init(cx);
        crate::ui::overlay::bind_keys(cx);
        crate::ui::pin::bind_keys(cx);
        set_auto_start(crate::core::settings::SETTINGS.lock().unwrap().get().general.auto_start);
        let overlay_handle = overlay::OverlayHandle::new(cx);
        cx.set_global(overlay_handle);

        if let Err(err) = crate::ui::tray_icon::SystemTray::install(cx) {
            tracing::error!("Failed to install system tray: {err}");
            cx.quit();
            return;
        }

        if let Err(err) = background_host::install(cx) {
            tracing::error!("Failed to install background host window: {err}");
            cx.quit();
        }
    });
}

pub fn prepare_overlay_session(cx: &mut gpui::App) {
    let overlay_handle = cx.global::<overlay::OverlayHandle>().clone();
    overlay_handle.prepare(cx);
}

pub fn open_preferences_window(cx: &mut gpui::App) {
    preferences::open_window(cx);
}

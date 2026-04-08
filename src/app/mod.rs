pub(crate) mod asset_bytes;
pub(crate) mod asset_paths;
pub mod assets;
pub mod background_host;

use crate::core::app::{ensure_single_instance, get_instance_id, init_logger, set_auto_start};
use crate::core::capture::service::CaptureService;
use crate::core::geometry::Rect;
use crate::core::notify::init_windows_notification_app_id;
use crate::core::shutdown;
use crate::core::{i18n, notify};
use crate::ui::{overlay, pin, preferences};
use gpui::{App, Application};
use tokio::sync::broadcast;
use tracing::info;

pub fn run() {
    let _guard = init_logger();
    info!("Starting MinnowSnap...");

    if !ensure_single_instance(&get_instance_id()) {
        info!("Another instance is running, exiting.");
        return;
    }

    shutdown::init_control_plane();
    #[cfg(target_os = "windows")]
    {
        shutdown::install_ctrl_c_handler();
        shutdown::start_control_pipe_server();
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
        install_shutdown_listener(cx);

        let locale = crate::core::settings::SETTINGS
            .lock()
            .map(|settings| settings.get().general.language)
            .unwrap_or_else(|_| crate::core::i18n::SYSTEM_LOCALE.to_string());
        crate::core::i18n::init(&locale);
        gpui_component::init(cx);
        crate::core::appearance::apply_saved_preferences(None, cx);
        crate::ui::overlay::bind_keys(cx);
        crate::ui::pin::bind_keys(cx);
        pin::install(cx);
        set_auto_start(crate::core::settings::SETTINGS.lock().unwrap().get().general.auto_start);
        crate::core::hotkey::install_hotkey_service(cx);
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

    shutdown::clear_control_plane();
}

fn install_shutdown_listener(cx: &mut App) {
    let Some(mut shutdown_rx) = shutdown::subscribe() else {
        tracing::warn!("Shutdown control plane is not initialized; skip shutdown listener.");
        return;
    };
    let Some(shutdown_token) = shutdown::cancellation_token() else {
        tracing::warn!("Shutdown cancellation token is unavailable; skip shutdown listener.");
        return;
    };

    cx.spawn(async move |cx| {
        tokio::select! {
            _ = shutdown_token.cancelled() => {
                request_app_quit(cx);
            }
            trigger = shutdown_rx.recv() => {
                match trigger {
                    Ok(trigger) => {
                        info!("Applying shutdown trigger: {trigger:?}");
                        request_app_quit(cx);
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        request_app_quit(cx);
                    }
                    Err(broadcast::error::RecvError::Closed) => {}
                }
            }
        }
    })
    .detach();
}

fn request_app_quit(cx: &mut gpui::AsyncApp) {
    let _ = cx.update(|app| {
        app.quit();
    });
}

pub fn prepare_overlay_session(cx: &mut gpui::App) {
    let overlay_handle = cx.global::<overlay::OverlayHandle>().clone();
    overlay_handle.prepare(cx);
}

pub fn open_capture_overlay(cx: &mut gpui::App) {
    prepare_overlay_session(cx);
    overlay::open_window(cx);
}

pub fn run_quick_capture_with_notification() {
    let ok = CaptureService::run_quick_capture_workflow(Rect::empty());
    if ok {
        notify::show(
            i18n::app::capture_name().as_str(),
            i18n::notify::quick_capture_copied().as_str(),
            notify::NotificationType::Copy,
        );
    } else {
        notify::show(
            i18n::app::name().as_str(),
            i18n::notify::quick_capture_failed().as_str(),
            notify::NotificationType::Info,
        );
    }
}

pub fn open_preferences_window(cx: &mut gpui::App) {
    preferences::open_window(cx);
}

pub fn shutdown_running_instance() -> u8 {
    #[cfg(target_os = "windows")]
    {
        match shutdown::shutdown_running_instance() {
            Ok(()) => 0,
            Err(shutdown::ShutdownClientError::NotRunning) => 2,
            Err(err) => {
                eprintln!("Failed to request graceful shutdown: {err}");
                3
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("Shutdown command is only supported on Windows.");
        3
    }
}

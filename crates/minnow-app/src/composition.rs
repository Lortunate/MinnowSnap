use gpui::{App, Application};
use minnow_assets::AppAssets;
use minnow_core::capture::service::CaptureService;
use minnow_core::geometry::Rect;
use minnow_core::notify;
use minnow_core::notify::init_windows_notification_app_id;
use minnow_core::shutdown;
use minnow_core::{i18n, settings};
use minnow_ui::features::{overlay, pin, preferences};
use minnow_ui::shell::hotkey::HotkeyActionSink;
use minnow_ui::shell::system::install_ui_system_actions;
use minnow_ui::shell::tray::TrayActions;
use minnow_ui::support::{appearance, locale};
use tokio::sync::broadcast;
use tracing::info;

#[cfg(target_os = "macos")]
use minnow_core::app_meta::APP_ID;

pub(crate) fn run_application(set_auto_start: fn(bool), _hide_dock_icon: fn()) {
    #[cfg(target_os = "windows")]
    init_windows_notification_app_id();

    #[cfg(target_os = "macos")]
    if let Err(err) = notify_rust::set_application(APP_ID) {
        tracing::error!("Failed to set application: {err}");
    }

    #[cfg(target_os = "macos")]
    _hide_dock_icon();

    let app = Application::new().with_assets(AppAssets);

    app.run(move |cx| {
        install_shutdown_listener(cx);

        let locale_choice = settings::SETTINGS
            .lock()
            .map(|settings| settings.get().general.language)
            .unwrap_or_else(|_| i18n::SYSTEM_LOCALE.to_string());
        locale::apply(&locale_choice);
        gpui_component::init(cx);
        appearance::apply_saved_preferences(None, cx);
        install_ui_system_actions(cx, set_auto_start);
        overlay::bind_keys(cx);
        pin::bind_keys(cx);
        pin::install(cx);
        set_auto_start(settings::SETTINGS.lock().unwrap().get().general.auto_start);
        minnow_ui::shell::hotkey::install_hotkey_service(cx, HotkeyActionSink::new(open_capture_overlay, run_quick_capture_with_notification));
        let overlay_handle = overlay::OverlayHandle::new(cx);
        cx.set_global(overlay_handle);

        if let Err(err) = minnow_ui::shell::tray::SystemTray::install(
            cx,
            TrayActions::new(open_capture_overlay, run_quick_capture_with_notification, open_preferences_window),
        ) {
            tracing::error!("Failed to install system tray: {err}");
            cx.quit();
            return;
        }

        if let Err(err) = minnow_ui::shell::background_host::install(cx) {
            tracing::error!("Failed to install background host window: {err}");
            cx.quit();
        }
    });
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

pub(crate) fn prepare_overlay_session(cx: &mut gpui::App) {
    let overlay_handle = cx.global::<overlay::OverlayHandle>().clone();
    overlay_handle.prepare(cx);
}

pub(crate) fn open_capture_overlay(cx: &mut gpui::App) {
    prepare_overlay_session(cx);
    overlay::open_window(cx);
}

pub(crate) fn run_quick_capture_with_notification() {
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

pub(crate) fn open_preferences_window(cx: &mut gpui::App) {
    preferences::open_window(cx);
}

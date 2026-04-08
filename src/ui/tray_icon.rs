use crate::app::asset_bytes;
use crate::core::app::APP_NAME;
use crate::core::async_ui::{app_ready, update_app};
use crate::core::i18n;
use crate::core::shutdown::{self, ShutdownTrigger};
use gpui::{App, Global};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use tokio_util::sync::CancellationToken;
use tracing::info;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
};

#[derive(Clone)]
pub struct TrayMenuIds {
    capture_overlay: MenuId,
    quick_capture: MenuId,
    preferences: MenuId,
    exit: MenuId,
}

impl TrayMenuIds {
    fn new() -> Self {
        Self {
            capture_overlay: MenuId::new("tray.capture_overlay"),
            quick_capture: MenuId::new("tray.quick_capture"),
            preferences: MenuId::new("tray.preferences"),
            exit: MenuId::new("tray.exit"),
        }
    }
}

pub struct SystemTray {
    tray_icon: TrayIcon,
    menu_ids: TrayMenuIds,
}

enum TrayDispatchEvent {
    Menu(MenuEvent),
    Tray(TrayIconEvent),
}

impl Global for SystemTray {}

impl SystemTray {
    pub fn install(cx: &mut App) -> Result<(), String> {
        let tray = Self::build()?;
        let menu_ids = tray.menu_ids.clone();
        cx.set_global(tray);

        let (event_tx, event_rx) = unbounded_channel();
        Self::install_event_handlers(event_tx);
        let shutdown_token = shutdown::cancellation_token().unwrap_or_default();

        cx.spawn(async move |cx| {
            Self::event_loop(menu_ids, event_rx, shutdown_token, cx).await;
            Self::clear_event_handlers();
        })
        .detach();

        info!("System tray installed");
        Ok(())
    }

    fn build() -> Result<Self, String> {
        let menu_ids = TrayMenuIds::new();

        let capture_overlay = MenuItem::with_id(menu_ids.capture_overlay.clone(), i18n::tray::capture_overlay(), true, None);
        let quick_capture = MenuItem::with_id(menu_ids.quick_capture.clone(), i18n::tray::quick_capture(), true, None);
        let preferences = MenuItem::with_id(menu_ids.preferences.clone(), i18n::tray::preferences(), true, None);
        let exit = MenuItem::with_id(menu_ids.exit.clone(), i18n::tray::exit(), true, None);
        let separator = PredefinedMenuItem::separator();

        let menu = Menu::new();
        menu.append_items(&[&capture_overlay, &quick_capture, &separator, &preferences, &separator, &exit])
            .map_err(|err| format!("failed to build tray menu: {err}"))?;

        let icon = load_icon()?;
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(APP_NAME)
            .with_icon(icon)
            .build()
            .map_err(|err| format!("failed to create tray icon: {err}"))?;

        Ok(Self { tray_icon, menu_ids })
    }

    fn install_event_handlers(event_tx: tokio::sync::mpsc::UnboundedSender<TrayDispatchEvent>) {
        let menu_tx = event_tx.clone();
        MenuEvent::set_event_handler(Some(move |event| {
            let _ = menu_tx.send(TrayDispatchEvent::Menu(event));
        }));

        let tray_tx = event_tx;
        TrayIconEvent::set_event_handler(Some(move |event| {
            let _ = tray_tx.send(TrayDispatchEvent::Tray(event));
        }));
    }

    fn clear_event_handlers() {
        MenuEvent::set_event_handler::<fn(MenuEvent)>(None);
        TrayIconEvent::set_event_handler::<fn(TrayIconEvent)>(None);
    }

    async fn event_loop(
        menu_ids: TrayMenuIds,
        mut event_rx: UnboundedReceiver<TrayDispatchEvent>,
        shutdown_token: CancellationToken,
        cx: &mut gpui::AsyncApp,
    ) {
        loop {
            tokio::select! {
                _ = shutdown_token.cancelled() => return,
                event = event_rx.recv() => {
                    let Some(event) = event else {
                        return;
                    };

                    let should_exit = match event {
                        TrayDispatchEvent::Menu(event) => Self::handle_menu_event(&menu_ids, event, cx),
                        TrayDispatchEvent::Tray(event) => Self::handle_tray_event(event, cx),
                    };

                    if should_exit {
                        return;
                    }
                }
            }
        }
    }

    fn handle_menu_event(menu_ids: &TrayMenuIds, event: MenuEvent, cx: &mut gpui::AsyncApp) -> bool {
        if event.id == menu_ids.capture_overlay {
            return !update_app(cx, |app| {
                crate::app::open_capture_overlay(app);
            });
        }

        if event.id == menu_ids.quick_capture {
            if !app_ready(cx) {
                return true;
            }
            crate::app::run_quick_capture_with_notification();
            return false;
        }

        if event.id == menu_ids.preferences {
            return !update_app(cx, |app| {
                crate::app::open_preferences_window(app);
            });
        }

        if event.id == menu_ids.exit {
            shutdown::request_shutdown(ShutdownTrigger::TrayMenu);
            return true;
        }

        false
    }

    fn handle_tray_event(event: TrayIconEvent, cx: &mut gpui::AsyncApp) -> bool {
        if let TrayIconEvent::DoubleClick { .. } = event {
            return !update_app(cx, |app| {
                crate::app::open_capture_overlay(app);
            });
        }

        false
    }
}

impl Drop for SystemTray {
    fn drop(&mut self) {
        let _ = &self.tray_icon;
    }
}

fn load_icon() -> Result<Icon, String> {
    let image = image::load_from_memory(asset_bytes::LOGO_BYTES).map_err(|err| format!("failed to decode tray icon image: {err}"))?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), width, height).map_err(|err| format!("failed to build tray icon: {err}"))
}

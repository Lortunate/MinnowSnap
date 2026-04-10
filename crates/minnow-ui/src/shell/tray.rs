use crate::shell::async_ui::{app_ready, update_app};
use gpui::{App, Global};
use minnow_assets::asset_bytes;
use minnow_core::app_meta::APP_NAME;
use minnow_core::i18n;
use minnow_core::platform::shutdown::{self, ShutdownTrigger};
use std::sync::Arc;
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

#[derive(Clone)]
pub struct TrayActions {
    open_capture_overlay: Arc<dyn Fn(&mut App) + Send + Sync>,
    run_quick_capture: Arc<dyn Fn() + Send + Sync>,
    open_preferences: Arc<dyn Fn(&mut App) + Send + Sync>,
}

impl TrayActions {
    pub fn new<F1, F2, F3>(open_capture_overlay: F1, run_quick_capture: F2, open_preferences: F3) -> Self
    where
        F1: Fn(&mut App) + Send + Sync + 'static,
        F2: Fn() + Send + Sync + 'static,
        F3: Fn(&mut App) + Send + Sync + 'static,
    {
        Self {
            open_capture_overlay: Arc::new(open_capture_overlay),
            run_quick_capture: Arc::new(run_quick_capture),
            open_preferences: Arc::new(open_preferences),
        }
    }

    fn open_capture_overlay(&self, app: &mut App) {
        (self.open_capture_overlay)(app);
    }

    fn run_quick_capture(&self) {
        (self.run_quick_capture)();
    }

    fn open_preferences(&self, app: &mut App) {
        (self.open_preferences)(app);
    }
}

enum TrayDispatchEvent {
    Menu(MenuEvent),
    Tray(TrayIconEvent),
}

impl Global for SystemTray {}

impl SystemTray {
    pub fn install(cx: &mut App, actions: TrayActions) -> Result<(), String> {
        let tray = Self::build()?;
        let menu_ids = tray.menu_ids.clone();
        cx.set_global(tray);

        let (event_tx, event_rx) = unbounded_channel();
        Self::install_event_handlers(event_tx);
        let shutdown_token = shutdown::cancellation_token().unwrap_or_default();

        cx.spawn(async move |cx| {
            Self::event_loop(menu_ids, actions, event_rx, shutdown_token, cx).await;
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
        actions: TrayActions,
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
                        TrayDispatchEvent::Menu(event) => Self::handle_menu_event(&menu_ids, &actions, event, cx),
                        TrayDispatchEvent::Tray(event) => Self::handle_tray_event(&actions, event, cx),
                    };

                    if should_exit {
                        return;
                    }
                }
            }
        }
    }

    fn handle_menu_event(menu_ids: &TrayMenuIds, actions: &TrayActions, event: MenuEvent, cx: &mut gpui::AsyncApp) -> bool {
        if event.id == menu_ids.capture_overlay {
            return !update_app(cx, |app| {
                actions.open_capture_overlay(app);
            });
        }

        if event.id == menu_ids.quick_capture {
            if !app_ready(cx) {
                return true;
            }
            actions.run_quick_capture();
            return false;
        }

        if event.id == menu_ids.preferences {
            return !update_app(cx, |app| {
                actions.open_preferences(app);
            });
        }

        if event.id == menu_ids.exit {
            shutdown::request_shutdown(ShutdownTrigger::TrayMenu);
            return true;
        }

        false
    }

    fn handle_tray_event(actions: &TrayActions, event: TrayIconEvent, cx: &mut gpui::AsyncApp) -> bool {
        if let TrayIconEvent::DoubleClick { .. } = event {
            return !update_app(cx, |app| {
                actions.open_capture_overlay(app);
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

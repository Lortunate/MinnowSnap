use crate::app::asset_bytes;
use crate::core::app::APP_NAME;
use crate::core::i18n;
use gpui::{App, Global};
use std::time::Duration;
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

impl Global for SystemTray {}

impl SystemTray {
    pub fn install(cx: &mut App) -> Result<(), String> {
        let tray = Self::build()?;
        let menu_ids = tray.menu_ids.clone();
        cx.set_global(tray);

        let menu_rx = MenuEvent::receiver().clone();
        let tray_rx = TrayIconEvent::receiver().clone();

        cx.spawn(async move |cx| {
            Self::event_loop(menu_ids, tray_rx, menu_rx, cx).await;
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

    async fn event_loop(
        menu_ids: TrayMenuIds,
        tray_rx: tray_icon::TrayIconEventReceiver,
        menu_rx: tray_icon::menu::MenuEventReceiver,
        cx: &mut gpui::AsyncApp,
    ) {
        loop {
            let mut processed = false;

            while let Ok(event) = menu_rx.try_recv() {
                processed = true;
                if Self::handle_menu_event(&menu_ids, event, cx) {
                    return;
                }
            }

            while let Ok(event) = tray_rx.try_recv() {
                processed = true;
                if Self::handle_tray_event(event, cx) {
                    return;
                }
            }

            if !processed {
                cx.background_executor().timer(Duration::from_millis(50)).await;
            }
        }
    }

    fn handle_menu_event(menu_ids: &TrayMenuIds, event: MenuEvent, cx: &mut gpui::AsyncApp) -> bool {
        if event.id == menu_ids.capture_overlay {
            let _ = cx.update(|app| {
                crate::app::open_capture_overlay(app);
            });
            return false;
        }

        if event.id == menu_ids.quick_capture {
            crate::app::run_quick_capture_with_notification();
            return false;
        }

        if event.id == menu_ids.preferences {
            let _ = cx.update(|app| {
                crate::app::open_preferences_window(app);
            });
            return false;
        }

        if event.id == menu_ids.exit {
            let _ = cx.update(|app| {
                app.quit();
            });
            return true;
        }

        false
    }

    fn handle_tray_event(event: TrayIconEvent, cx: &mut gpui::AsyncApp) -> bool {
        if let TrayIconEvent::DoubleClick { .. } = event {
            let _ = cx.update(|app| {
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

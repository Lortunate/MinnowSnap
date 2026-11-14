use super::{
    APP_NAME, HostEvent,
    assets::LOGO_BYTES,
    shutdown::{ShutdownRequest, ShutdownTrigger},
};
use minnow_ui::i18n;
use tokio::sync::mpsc::Sender;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
};

/// The host owns the native tray for exactly the desktop application's lifetime.
pub(crate) struct SystemTray {
    _icon: TrayIcon,
}

impl SystemTray {
    pub(crate) fn new(events: Sender<HostEvent>, shutdown: ShutdownRequest) -> anyhow::Result<Self> {
        let capture = MenuId::new("tray.capture_overlay");
        let quick = MenuId::new("tray.quick_capture");
        let preferences = MenuId::new("tray.preferences");
        let exit = MenuId::new("tray.exit");
        let menu = Menu::new();
        menu.append_items(&[
            &MenuItem::with_id(capture.clone(), i18n::tray::capture_overlay(), true, None),
            &MenuItem::with_id(quick.clone(), i18n::tray::quick_capture(), true, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id(preferences.clone(), i18n::tray::preferences(), true, None),
            &PredefinedMenuItem::separator(),
            &MenuItem::with_id(exit.clone(), i18n::tray::exit(), true, None),
        ])?;
        let rgba = image::load_from_memory(LOGO_BYTES)?.to_rgba8();
        let (width, height) = rgba.dimensions();
        let icon = Icon::from_rgba(rgba.into_raw(), width, height)?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(APP_NAME)
            .with_icon(icon)
            .build()?;

        let menu_events = events.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == exit {
                shutdown.request(ShutdownTrigger::Tray);
                return;
            }
            let action = if event.id == capture {
                Some(HostEvent::Capture)
            } else if event.id == quick {
                Some(HostEvent::QuickCapture)
            } else if event.id == preferences {
                Some(HostEvent::Preferences)
            } else {
                None
            };
            if let Some(action) = action {
                let _ = menu_events.try_send(action);
            }
        }));
        TrayIconEvent::set_event_handler(Some(move |event| {
            if matches!(event, TrayIconEvent::DoubleClick { .. }) {
                let _ = events.try_send(HostEvent::Capture);
            }
        }));
        tracing::info!("System tray installed");
        Ok(Self { _icon: icon })
    }
}

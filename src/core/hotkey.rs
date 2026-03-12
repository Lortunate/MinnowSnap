use crate::core::settings::ShortcutSettings;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tracing::{error, info};

#[derive(Default)]
pub struct HotkeyIds {
    pub screen_capture: Option<u32>,
    pub quick_capture: Option<u32>,
}

pub struct HotkeyManager {
    pub manager: Option<GlobalHotKeyManager>,
    pub ids: Arc<Mutex<HotkeyIds>>,
    pub screen_hotkey: Option<HotKey>,
    pub quick_hotkey: Option<HotKey>,
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self {
            manager: None,
            ids: Arc::new(Mutex::new(HotkeyIds::default())),
            screen_hotkey: None,
            quick_hotkey: None,
        }
    }
}

pub fn parse_hotkey(shortcut: &str) -> Option<HotKey> {
    if shortcut.is_empty() {
        return None;
    }
    match HotKey::from_str(shortcut) {
        Ok(hk) => Some(hk),
        Err(e) => {
            error!("Failed to parse hotkey '{shortcut}': {e}");
            None
        }
    }
}

impl HotkeyManager {
    pub fn register_global_hotkeys<F1, F2>(&mut self, screen_shortcut: &str, quick_shortcut: &str, screen_callback: F1, quick_callback: F2)
    where
        F1: Fn() + Send + 'static,
        F2: Fn() + Send + 'static,
    {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to create hotkey manager: {e}");
                return;
            }
        };

        self.manager = Some(manager);
        let screen_hotkey = parse_hotkey(screen_shortcut);
        let quick_hotkey = parse_hotkey(quick_shortcut);

        if let Some(ref m) = self.manager {
            if let Some(hk) = screen_hotkey {
                if let Err(e) = m.register(hk) {
                    error!("Failed to register screen hotkey: {e}");
                } else {
                    self.ids.lock().unwrap().screen_capture = Some(hk.id());
                    self.screen_hotkey = Some(hk);
                    info!("Screen capture hotkey registered: {screen_shortcut}");
                }
            }

            if let Some(hk) = quick_hotkey {
                if let Err(e) = m.register(hk) {
                    error!("Failed to register quick hotkey: {e}");
                } else {
                    self.ids.lock().unwrap().quick_capture = Some(hk.id());
                    self.quick_hotkey = Some(hk);
                    info!("Quick capture hotkey registered: {quick_shortcut}");
                }
            }
        }

        let ids_clone = self.ids.clone();
        crate::core::RUNTIME.spawn_blocking(move || {
            let receiver = GlobalHotKeyEvent::receiver();
            while let Ok(event) = receiver.recv() {
                if event.state == HotKeyState::Pressed {
                    let ids = ids_clone.lock().unwrap();

                    if let Some(id) = ids.screen_capture
                        && event.id == id
                    {
                        info!("Screen capture hotkey triggered (id: {id})");
                        screen_callback();
                    }

                    if let Some(id) = ids.quick_capture
                        && event.id == id
                    {
                        info!("Quick capture hotkey triggered (id: {id})");
                        quick_callback();
                    }
                }
            }
        });

        info!("Global hotkeys registered");
    }

    pub fn update_shortcut(&mut self, shortcut: &str, is_screen: bool) {
        let mut shortcut_str = shortcut.to_string();
        if shortcut_str.is_empty() {
            let defaults = ShortcutSettings::default();
            shortcut_str = if is_screen { defaults.capture } else { defaults.quick_capture };
        }

        let Some(manager) = &self.manager else {
            return;
        };
        let new_hotkey = parse_hotkey(&shortcut_str);

        let current_hotkey = if is_screen { &mut self.screen_hotkey } else { &mut self.quick_hotkey };

        if let Some(old) = current_hotkey
            && let Err(e) = manager.unregister(*old)
        {
            error!("Failed to unregister hotkey: {e}");
        }

        let mut next_hotkey = None;

        if let Some(hotkey) = new_hotkey {
            if let Err(e) = manager.register(hotkey) {
                error!("Failed to register hotkey: {e}");
            } else {
                next_hotkey = Some(hotkey);
                let label = if is_screen { "Screen capture" } else { "Quick capture" };
                info!("{label} hotkey updated to: {shortcut_str}");
            }
        } else {
            let label = if is_screen { "Screen capture" } else { "Quick capture" };
            info!("{label} hotkey cleared");
        }

        *current_hotkey = next_hotkey;

        let mut ids = self.ids.lock().unwrap();
        if is_screen {
            ids.screen_capture = next_hotkey.map(|hk| hk.id());
        } else {
            ids.quick_capture = next_hotkey.map(|hk| hk.id());
        }
    }
}

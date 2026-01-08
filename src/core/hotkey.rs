use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use log::{error, info};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct HotkeyIds {
    pub screen_capture: Option<u32>,
    pub quick_capture: Option<u32>,
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

pub struct HotkeyService;

impl HotkeyService {
    pub fn register_global_hotkeys<F1, F2>(
        screen_shortcut: &str,
        quick_shortcut: &str,
        screen_callback: F1,
        quick_callback: F2,
        hotkey_ids: Arc<Mutex<HotkeyIds>>,
    ) -> Option<GlobalHotKeyManager>
    where
        F1: Fn() + Send + 'static,
        F2: Fn() + Send + 'static,
    {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to create hotkey manager: {e}");
                return None;
            }
        };

        let screen_hotkey = parse_hotkey(screen_shortcut);
        let quick_hotkey = parse_hotkey(quick_shortcut);

        if let Some(hk) = screen_hotkey {
            if let Err(e) = manager.register(hk) {
                error!("Failed to register screen hotkey: {e}");
            } else {
                hotkey_ids.lock().unwrap().screen_capture = Some(hk.id());
                info!("Screen capture hotkey registered: {screen_shortcut}");
            }
        }

        if let Some(hk) = quick_hotkey {
            if let Err(e) = manager.register(hk) {
                error!("Failed to register quick hotkey: {e}");
            } else {
                hotkey_ids.lock().unwrap().quick_capture = Some(hk.id());
                info!("Quick capture hotkey registered: {quick_shortcut}");
            }
        }

        std::thread::spawn(move || {
            let receiver = GlobalHotKeyEvent::receiver();
            while let Ok(event) = receiver.recv() {
                if event.state == HotKeyState::Pressed {
                    let ids = hotkey_ids.lock().unwrap();

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
        Some(manager)
    }

    pub fn update_hotkey_registration(
        manager: &GlobalHotKeyManager,
        current_hotkey: &mut Option<HotKey>,
        new_shortcut: &str,
        hotkey_ids: &Arc<Mutex<HotkeyIds>>,
        is_screen: bool,
    ) {
        let new_hotkey = parse_hotkey(new_shortcut);

        if let Some(old) = current_hotkey
            && let Err(e) = manager.unregister(*old)
        {
            error!("Failed to unregister hotkey: {e}");
        }

        if let Some(hotkey) = new_hotkey {
            if let Err(e) = manager.register(hotkey) {
                error!("Failed to register hotkey: {e}");
                *current_hotkey = None;
                let mut ids = hotkey_ids.lock().unwrap();
                if is_screen {
                    ids.screen_capture = None;
                } else {
                    ids.quick_capture = None;
                }
            } else {
                *current_hotkey = Some(hotkey);
                let mut ids = hotkey_ids.lock().unwrap();
                if is_screen {
                    ids.screen_capture = Some(hotkey.id());
                } else {
                    ids.quick_capture = Some(hotkey.id());
                }
                info!("Hotkey updated to: {new_shortcut}");
            }
        } else {
            *current_hotkey = None;
            let mut ids = hotkey_ids.lock().unwrap();
            if is_screen {
                ids.screen_capture = None;
            } else {
                ids.quick_capture = None;
            }
        }
    }
}

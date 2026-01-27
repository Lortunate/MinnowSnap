use crate::core::hotkey::{HotkeyIds, HotkeyService};
use crate::core::settings::ShortcutSettings;
use cxx_qt_lib::QString;
use global_hotkey::{hotkey::HotKey, GlobalHotKeyManager};
use std::sync::{Arc, Mutex};

pub struct HotkeyState {
    pub manager: Option<GlobalHotKeyManager>,
    pub ids: Arc<Mutex<HotkeyIds>>,
    pub screen: Option<HotKey>,
    pub quick: Option<HotKey>,
}

impl Default for HotkeyState {
    fn default() -> Self {
        Self {
            manager: None,
            ids: Arc::new(Mutex::new(HotkeyIds::default())),
            screen: None,
            quick: None,
        }
    }
}

pub fn update_hotkey(state: &mut HotkeyState, shortcut: QString, is_screen: bool) {
    let mut shortcut_str = shortcut.to_string();
    if shortcut_str.is_empty() {
        let defaults = ShortcutSettings::default();
        shortcut_str = if is_screen { defaults.capture } else { defaults.quick_capture };
    }

    if let Some(manager) = &state.manager {
        let current_hotkey = if is_screen { &mut state.screen } else { &mut state.quick };
        HotkeyService::update_hotkey_registration(manager, current_hotkey, &shortcut_str, &state.ids, is_screen);
    }
}

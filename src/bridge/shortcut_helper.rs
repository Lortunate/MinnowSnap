use cxx_qt_lib::QString;
use global_hotkey::{GlobalHotKeyManager, hotkey::HotKey};
use log::{error, info};
use std::str::FromStr;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cpp/shortcut_utils.hpp");
        #[namespace = "ShortcutUtils"]
        #[cxx_name = "getKeySequence"]
        fn get_key_sequence_cpp(key: i32, modifiers: i32) -> QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type ShortcutHelper = super::ShortcutHelperRust;

        #[qinvokable]
        #[cxx_name = "getKeySequence"]
        fn get_key_sequence(self: &Self, key: i32, modifiers: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "registerGlobalHotkey"]
        fn register_global_hotkey(self: &Self, seq: QString);
    }
}

pub struct ShortcutHelperRust;

impl Default for ShortcutHelperRust {
    fn default() -> Self {
        Self
    }
}

impl qobject::ShortcutHelper {
    pub fn get_key_sequence(&self, key: i32, modifiers: i32) -> QString {
        qobject::get_key_sequence_cpp(key, modifiers)
    }

    pub fn register_global_hotkey(&self, seq: QString) {
        let seq_str = seq.to_string();

        let Ok(hotkey) = HotKey::from_str(&seq_str) else {
            return error!("Failed to parse hotkey string: {seq_str}");
        };

        let Ok(manager) = GlobalHotKeyManager::new() else {
            return error!("Failed to create GlobalHotKeyManager");
        };

        if let Err(e) = manager.register(hotkey) {
            error!("Failed to register global hotkey: {e:?}");
        } else {
            info!("Successfully registered global hotkey: {seq_str}");
        }
    }
}

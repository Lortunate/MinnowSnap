use cxx_qt_lib::QString;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type ShortcutHelper = super::ShortcutHelperRust;

        #[qinvokable]
        #[cxx_name = "getKeySequence"]
        fn get_key_sequence(self: &Self, key: i32, modifiers: i32, text: QString) -> QString;
    }
}

pub struct ShortcutHelperRust;

impl Default for ShortcutHelperRust {
    fn default() -> Self {
        Self
    }
}

impl qobject::ShortcutHelper {
    pub fn get_key_sequence(&self, key: i32, modifiers: i32, text: QString) -> QString {
        const SHIFT_MODIFIER: i32 = 0x02000000;
        const CONTROL_MODIFIER: i32 = 0x04000000;
        const ALT_MODIFIER: i32 = 0x08000000;
        const META_MODIFIER: i32 = 0x10000000;

        const KEY_SPACE: i32 = 0x20;
        const KEY_BACKSPACE: i32 = 0x01000003;
        const KEY_DELETE: i32 = 0x01000007;
        const KEY_ESCAPE: i32 = 0x01000000;
        const KEY_F1: i32 = 0x01000030;
        const KEY_F12: i32 = 0x0100003b;

        if [0x01000020, 0x01000021, 0x01000022, 0x01000023].contains(&key) {
            return QString::from("");
        }

        if key == KEY_BACKSPACE || key == KEY_DELETE {
            return QString::from("DELETE_Request");
        }

        if key == KEY_ESCAPE {
            return QString::from("");
        }

        let mut parts = Vec::new();

        if (modifiers & CONTROL_MODIFIER) != 0 {
            parts.push("Ctrl");
        }
        if (modifiers & ALT_MODIFIER) != 0 {
            parts.push("Alt");
        }
        if (modifiers & SHIFT_MODIFIER) != 0 {
            parts.push("Shift");
        }
        if (modifiers & META_MODIFIER) != 0 {
            if cfg!(target_os = "macos") {
                parts.push("Cmd");
            } else {
                parts.push("Meta");
            }
        }

        let key_name = if (KEY_F1..=KEY_F12).contains(&key) {
            format!("F{}", key - KEY_F1 + 1)
        } else if key == KEY_SPACE {
            "Space".to_string()
        } else {
            let t = text.to_string().to_uppercase();
            if !t.is_empty() { t } else { "".to_string() }
        };

        if key_name.is_empty() {
            return QString::from("");
        }

        parts.push(&key_name);
        QString::from(&parts.join("+"))
    }
}

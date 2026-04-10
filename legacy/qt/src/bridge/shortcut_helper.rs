use cxx_qt_lib::QString;

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
}

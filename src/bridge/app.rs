#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cpp/app_utils.hpp");
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        unsafe fn quit_app();
        unsafe fn set_quit_on_last_window_closed();
        unsafe fn set_window_icon();
        unsafe fn install_translator(locale: &str);
        unsafe fn retranslate_all();
        unsafe fn cursor_x() -> i32;
        unsafe fn cursor_y() -> i32;
        fn translate(context: &str, source_text: &str) -> QString;
    }
}

pub fn quit_app() {
    unsafe { ffi::quit_app() }
}

pub fn set_quit_on_last_window_closed() {
    unsafe { ffi::set_quit_on_last_window_closed() }
}

pub fn set_window_icon() {
    unsafe { ffi::set_window_icon() }
}

pub fn install_translator(locale: &str) {
    unsafe { ffi::install_translator(locale) }
}

pub fn retranslate() {
    unsafe { ffi::retranslate_all() }
}

pub fn tr(context: &str, source_text: &str) -> cxx_qt_lib::QString {
    ffi::translate(context, source_text)
}

pub fn cursor_position() -> (i32, i32) {
    unsafe { (ffi::cursor_x(), ffi::cursor_y()) }
}

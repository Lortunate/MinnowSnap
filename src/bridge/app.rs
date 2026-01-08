#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cpp/app_utils.hpp");
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        unsafe fn set_quit_on_last_window_closed();
        unsafe fn install_translator(locale: &str);
        unsafe fn retranslate_all();
        fn translate(context: &str, source_text: &str) -> QString;
    }
}

pub fn set_quit_on_last_window_closed() {
    unsafe { ffi::set_quit_on_last_window_closed() }
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

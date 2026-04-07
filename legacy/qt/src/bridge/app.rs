use std::sync::Once;

#[cxx::bridge]
mod ffi {
    extern "Rust" {
        fn push_qt_log(level: i32, category: &str, message: &str, file: &str, line: i32);
    }

    unsafe extern "C++" {
        include!("cpp/app_utils.hpp");
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        unsafe fn quit_app();
        unsafe fn set_quit_on_last_window_closed();
        unsafe fn set_window_icon();
        unsafe fn install_qt_message_handler();
        unsafe fn install_translator(locale: &str);
        unsafe fn retranslate_all();
        unsafe fn cursor_x() -> i32;
        unsafe fn cursor_y() -> i32;
        unsafe fn cursor_screen_x_at(x: i32, y: i32) -> i32;
        unsafe fn cursor_screen_y_at(x: i32, y: i32) -> i32;
        unsafe fn cursor_screen_width_at(x: i32, y: i32) -> i32;
        unsafe fn cursor_screen_height_at(x: i32, y: i32) -> i32;
        unsafe fn cursor_screen_scale_at(x: i32, y: i32) -> f64;
        fn translate(context: &str, source_text: &str) -> QString;
    }
}

static QT_LOGGING_INIT: Once = Once::new();

#[derive(Debug, Clone, Copy)]
pub struct CursorScreen {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
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

pub fn init_qt_logging() {
    QT_LOGGING_INIT.call_once(|| unsafe {
        ffi::install_qt_message_handler();
    });
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

fn cursor_screen_at(point_x: i32, point_y: i32) -> Option<CursorScreen> {
    let (x, y, width, height) = unsafe {
        (
            ffi::cursor_screen_x_at(point_x, point_y),
            ffi::cursor_screen_y_at(point_x, point_y),
            ffi::cursor_screen_width_at(point_x, point_y),
            ffi::cursor_screen_height_at(point_x, point_y),
        )
    };
    if width <= 0 || height <= 0 {
        return None;
    }
    let scale = unsafe { ffi::cursor_screen_scale_at(point_x, point_y) };
    Some(CursorScreen {
        x: f64::from(x),
        y: f64::from(y),
        width: f64::from(width),
        height: f64::from(height),
        scale: if scale > 0.0 { scale } else { 1.0 },
    })
}

pub fn screen_at(point_x: i32, point_y: i32) -> Option<CursorScreen> {
    cursor_screen_at(point_x, point_y)
}

pub fn cursor_screen() -> Option<CursorScreen> {
    let (x, y) = cursor_position();
    screen_at(x, y)
}

pub fn push_qt_log(level: i32, category: &str, message: &str, file: &str, line: i32) {
    crate::core::logging::log_qt_message(level, category, message, file, line);
}

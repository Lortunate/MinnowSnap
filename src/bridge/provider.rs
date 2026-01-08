use crate::core::capture::{LAST_CAPTURE, SCROLL_CAPTURE};
use cxx_qt_lib::{QImage, QQmlApplicationEngine, QString};
use log::info;
use std::pin::Pin;

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qimage.h");
        type QImage = cxx_qt_lib::QImage;
        include!("cxx-qt-lib/qqmlapplicationengine.h");
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cpp/image_provider.hpp");

        unsafe fn register_provider(engine: Pin<&mut QQmlApplicationEngine>);
        unsafe fn create_from_rgba(data: *const u8, width: i32, height: i32) -> QImage;
    }

    extern "Rust" {
        fn get_capture_qimage(id: QString) -> QImage;
    }
}

pub fn register_image_provider(engine: Pin<&mut QQmlApplicationEngine>) {
    unsafe { ffi::register_provider(engine) }
}

static DUMMY_PIXEL: [u8; 4] = [0, 0, 0, 0];

fn get_capture_qimage(id: QString) -> QImage {
    let id_str = id.to_string();
    info!("ImageProvider request: {id_str}");

    let make_qimage = |img: &image::RgbaImage| {
        let width = img.width().try_into().unwrap_or(0);
        let height = img.height().try_into().unwrap_or(0);
        let raw_data = img.as_raw();
        info!("Providing image: {width}x{height}");
        unsafe { ffi::create_from_rgba(raw_data.as_ptr(), width, height) }
    };

    if id_str.starts_with("scroll") {
        if let Ok(guard) = SCROLL_CAPTURE.lock()
            && let Some(img) = &*guard
        {
            return make_qimage(img);
        } else {
            info!("SCROLL_CAPTURE is None or lock failed");
        }
    } else if let Ok(guard) = LAST_CAPTURE.lock()
        && let Some(img) = &*guard
    {
        return make_qimage(img);
    }

    info!("Provider: No image in cache, providing empty QImage");
    unsafe { ffi::create_from_rgba(DUMMY_PIXEL.as_ptr(), 1, 1) }
}

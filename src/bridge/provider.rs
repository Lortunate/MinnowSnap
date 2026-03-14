use crate::core::capture::datasource::{self, VirtualCaptureSource};
use crate::core::capture::{LAST_CAPTURE, SCROLL_CAPTURE};
use cxx_qt_lib::{QImage, QImageFormat, QQmlApplicationEngine, QString};
use image::RgbaImage;
use std::cell::RefCell;
use std::pin::Pin;
use tracing::{info, warn};

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
    }

    extern "Rust" {
        fn get_capture_qimage(id: QString) -> QImage;
    }
}

pub fn register_image_provider(engine: Pin<&mut QQmlApplicationEngine>) {
    unsafe { ffi::register_provider(engine) }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ImageKey {
    width: u32,
    height: u32,
    len: usize,
    ptr: usize,
}

struct CachedQImage {
    key: ImageKey,
    image: QImage,
}

thread_local! {
    static PREVIEW_QIMAGE_CACHE: RefCell<Option<CachedQImage>> = const { RefCell::new(None) };
    static SCROLL_QIMAGE_CACHE: RefCell<Option<CachedQImage>> = const { RefCell::new(None) };
}

fn make_image_key(img: &RgbaImage) -> ImageKey {
    ImageKey {
        width: img.width(),
        height: img.height(),
        len: img.as_raw().len(),
        ptr: img.as_raw().as_ptr() as usize,
    }
}

fn empty_qimage() -> QImage {
    unsafe { QImage::from_raw_bytes(vec![0, 0, 0, 0], 1, 1, QImageFormat::Format_RGBA8888) }
}

fn make_qimage(img: &image::RgbaImage) -> QImage {
    let width = img.width().try_into().unwrap_or(0);
    let height = img.height().try_into().unwrap_or(0);
    info!("Providing image: {width}x{height}");
    unsafe { QImage::from_raw_bytes(img.as_raw().clone(), width, height, QImageFormat::Format_RGBA8888) }
}

fn get_cached_qimage(img: &RgbaImage, is_preview: bool) -> QImage {
    let key = make_image_key(img);
    let cache = if is_preview { &PREVIEW_QIMAGE_CACHE } else { &SCROLL_QIMAGE_CACHE };

    cache.with(|slot| {
        if let Some(cached) = slot.borrow().as_ref()
            && cached.key == key
        {
            return cached.image.clone();
        }

        let image = make_qimage(img);
        let image_for_cache = image.clone();
        *slot.borrow_mut() = Some(CachedQImage { key, image: image_for_cache });
        image
    })
}

pub fn clear_cached_qimages() {
    PREVIEW_QIMAGE_CACHE.with(|slot| *slot.borrow_mut() = None);
    SCROLL_QIMAGE_CACHE.with(|slot| *slot.borrow_mut() = None);
}

fn get_capture_qimage(id: QString) -> QImage {
    let id_str = id.to_string();
    info!("ImageProvider request: {}", datasource::normalize_provider_id(&id_str));

    match datasource::parse_provider_source(&id_str) {
        Some(VirtualCaptureSource::Preview) => {
            if let Ok(guard) = LAST_CAPTURE.lock()
                && let Some(img) = &*guard
            {
                return get_cached_qimage(img, true);
            }
        }
        Some(VirtualCaptureSource::Scroll) => {
            if let Ok(guard) = SCROLL_CAPTURE.lock()
                && let Some(img) = &*guard
            {
                return get_cached_qimage(img, false);
            }
        }
        None => {
            warn!("Unknown image provider id: {}", id_str);
        }
    }

    info!("Provider: No image in cache, providing empty QImage");
    empty_qimage()
}

use crate::core::capture::source::{self, VirtualCaptureSource};
use crate::core::capture::get_cached_capture;
use cxx_qt_lib::{QImage, QImageFormat, QQmlApplicationEngine, QString};
use image::RgbaImage;
use std::cell::RefCell;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{debug, warn};

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
    generation: u64,
}

#[derive(Default)]
struct ProviderImageCache {
    preview: Option<CachedQImage>,
    scroll: Option<CachedQImage>,
}

impl ProviderImageCache {
    fn get(&self, source: VirtualCaptureSource) -> Option<&CachedQImage> {
        match source {
            VirtualCaptureSource::Preview => self.preview.as_ref(),
            VirtualCaptureSource::Scroll => self.scroll.as_ref(),
        }
    }

    fn set(&mut self, source: VirtualCaptureSource, cached: CachedQImage) {
        match source {
            VirtualCaptureSource::Preview => self.preview = Some(cached),
            VirtualCaptureSource::Scroll => self.scroll = Some(cached),
        }
    }

    fn clear_slot(&mut self, source: VirtualCaptureSource) {
        match source {
            VirtualCaptureSource::Preview => self.preview = None,
            VirtualCaptureSource::Scroll => self.scroll = None,
        }
    }

    fn clear_all(&mut self) {
        self.preview = None;
        self.scroll = None;
    }
}

thread_local! {
    static QIMAGE_CACHE: RefCell<ProviderImageCache> = RefCell::new(ProviderImageCache::default());
}

static QIMAGE_CACHE_GENERATION: AtomicU64 = AtomicU64::new(1);

#[inline]
fn current_cache_generation() -> u64 {
    QIMAGE_CACHE_GENERATION.load(Ordering::Acquire)
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
    debug!("Providing image: {width}x{height}");
    unsafe { QImage::from_raw_bytes(img.as_raw().clone(), width, height, QImageFormat::Format_RGBA8888) }
}

fn with_source_image<T>(source: VirtualCaptureSource, mut f: impl FnMut(&RgbaImage) -> T) -> Option<T> {
    let shared = get_cached_capture(source)?;
    Some(f(shared.as_ref()))
}

fn get_cached_qimage(img: &RgbaImage, source: VirtualCaptureSource) -> QImage {
    let key = make_image_key(img);
    let generation = current_cache_generation();

    QIMAGE_CACHE.with(|cache| {
        if let Some(cached) = cache.borrow().get(source)
            && cached.key == key
            && cached.generation == generation
        {
            return cached.image.clone();
        }

        let image = make_qimage(img);
        let image_for_cache = image.clone();
        cache.borrow_mut().set(
            source,
            CachedQImage {
                key,
                image: image_for_cache,
                generation,
            },
        );
        image
    })
}

fn clear_cached_qimage_slot(source: VirtualCaptureSource) {
    QIMAGE_CACHE.with(|cache| cache.borrow_mut().clear_slot(source));
}

pub fn clear_cached_qimages() {
    let next = QIMAGE_CACHE_GENERATION.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
    if next == 0 {
        QIMAGE_CACHE_GENERATION.store(1, Ordering::Release);
    }
    QIMAGE_CACHE.with(|cache| cache.borrow_mut().clear_all());
}

fn get_capture_qimage(id: QString) -> QImage {
    let id_str = id.to_string();
    debug!("ImageProvider request: {}", source::normalize_provider_id(&id_str));

    let Some(source) = source::parse_provider_source(&id_str) else {
        warn!("Unknown image provider id: {}", id_str);
        return empty_qimage();
    };

    if let Some(image) = with_source_image(source, |img| get_cached_qimage(img, source)) {
        return image;
    }
    clear_cached_qimage_slot(source);

    debug!("Provider: No image in cache, providing empty QImage");
    empty_qimage()
}

pub mod action;
pub mod long_capture;
pub mod service;
pub mod stitcher;

use crate::services::geometry::Rect;
use image::RgbaImage;
use std::sync::{Arc, LazyLock, Mutex};
use tracing::{debug, error};
use xcap::Monitor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualCaptureSource {
    Preview,
    Scroll,
}

pub const PREVIEW_SOURCE: &str = "image://minnow/preview";
pub const SCROLL_SOURCE: &str = "image://minnow/scroll";

fn strip_query_fragment(input: &str) -> &str {
    input.split(['?', '#']).next().unwrap_or(input)
}

pub fn normalize_virtual_source(source: &str) -> &str {
    strip_query_fragment(source)
}

#[derive(Default)]
struct CaptureRepository {
    last_capture: Mutex<Option<Arc<RgbaImage>>>,
    scroll_capture: Mutex<Option<Arc<RgbaImage>>>,
}

impl CaptureRepository {
    #[must_use]
    fn get_cached_capture(&self, source: VirtualCaptureSource) -> Option<Arc<RgbaImage>> {
        self.cache_cell(source).lock().ok().and_then(|cache| cache.as_ref().cloned())
    }

    fn set_cached_capture(&self, source: VirtualCaptureSource, image: RgbaImage) {
        if let Ok(mut cache) = self.cache_cell(source).lock() {
            *cache = Some(Arc::new(image));
        }
    }

    fn cache_cell(&self, source: VirtualCaptureSource) -> &Mutex<Option<Arc<RgbaImage>>> {
        match source {
            VirtualCaptureSource::Preview => &self.last_capture,
            VirtualCaptureSource::Scroll => &self.scroll_capture,
        }
    }
}

static CAPTURE_REPOSITORY: LazyLock<CaptureRepository> = LazyLock::new(CaptureRepository::default);

pub fn get_cached_capture(source: VirtualCaptureSource) -> Option<Arc<RgbaImage>> {
    CAPTURE_REPOSITORY.get_cached_capture(source)
}

pub fn set_cached_capture(source: VirtualCaptureSource, image: RgbaImage) {
    CAPTURE_REPOSITORY.set_cached_capture(source, image);
}

pub fn update_last_capture(image: RgbaImage) {
    set_cached_capture(VirtualCaptureSource::Preview, image);
}

#[must_use]
pub fn active_monitor_scale() -> f32 {
    active_monitor().and_then(|m| m.scale_factor().ok()).unwrap_or(1.0)
}

#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn perform_crop(image: &RgbaImage, rect: Rect, scale: f32) -> Option<RgbaImage> {
    debug!(
        "Performing crop: rect={},{} {}x{}, scale={scale}",
        rect.x, rect.y, rect.width, rect.height
    );
    let img_w = image.width();
    let img_h = image.height();

    let x_phys = (rect.x as f32 * scale) as i32;
    let y_phys = (rect.y as f32 * scale) as i32;
    let w_phys = (rect.width as f32 * scale) as i32;
    let h_phys = (rect.height as f32 * scale) as i32;

    let crop_x = x_phys.max(0) as u32;
    let crop_y = y_phys.max(0) as u32;

    if crop_x >= img_w || crop_y >= img_h {
        return None;
    }

    let max_w = img_w - crop_x;
    let max_h = img_h - crop_y;

    let crop_w = (w_phys.max(0) as u32).min(max_w);
    let crop_h = (h_phys.max(0) as u32).min(max_h);

    if crop_w == 0 || crop_h == 0 {
        return None;
    }

    let sub_image = image::imageops::crop_imm(image, crop_x, crop_y, crop_w, crop_h);
    Some(sub_image.to_image())
}

#[must_use]
pub fn capture_active_monitor() -> Option<RgbaImage> {
    let Some(monitor) = active_monitor() else {
        error!("No monitors found");
        return None;
    };

    match monitor.capture_image() {
        Ok(image) => Some(image),
        Err(e) => {
            error!("Failed to capture monitor: {e}");
            None
        }
    }
}

#[must_use]
pub fn active_monitor() -> Option<Monitor> {
    Monitor::all().ok()?.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image(width: u32, height: u32, value: u8) -> RgbaImage {
        RgbaImage::from_pixel(width, height, image::Rgba([value, value, value, 255]))
    }

    #[test]
    fn repository_updates_preview_and_scroll_caches_independently() {
        let repository = CaptureRepository::default();

        repository.set_cached_capture(VirtualCaptureSource::Preview, test_image(2, 3, 10));
        repository.set_cached_capture(VirtualCaptureSource::Scroll, test_image(4, 5, 20));

        assert_eq!(repository.get_cached_capture(VirtualCaptureSource::Preview).unwrap().dimensions(), (2, 3));
        assert_eq!(repository.get_cached_capture(VirtualCaptureSource::Scroll).unwrap().dimensions(), (4, 5));
    }

    #[test]
    fn perform_crop_clamps_to_image_bounds() {
        let image = test_image(10, 10, 10);
        let crop = perform_crop(&image, Rect::new(8, 8, 5, 5), 1.0).unwrap();

        assert_eq!(crop.dimensions(), (2, 2));
    }

    #[test]
    fn perform_crop_rejects_empty_or_out_of_bounds_rects() {
        let image = test_image(10, 10, 10);

        assert!(perform_crop(&image, Rect::new(20, 0, 5, 5), 1.0).is_none());
        assert!(perform_crop(&image, Rect::new(0, 0, 0, 5), 1.0).is_none());
    }
}

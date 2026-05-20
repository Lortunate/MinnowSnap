pub mod action;
pub mod long_capture;
pub mod service;
mod stitcher;

use crate::services::geometry::Rect;
use image::RgbaImage;
use std::sync::{Arc, LazyLock, Mutex};
use tracing::{debug, error};
use xcap::Monitor;

pub(crate) const PREVIEW_SOURCE: &str = "image://minnow/preview";

fn strip_query_fragment(input: &str) -> &str {
    input.split(['?', '#']).next().unwrap_or(input)
}

fn normalize_virtual_source(source: &str) -> &str {
    strip_query_fragment(source)
}

#[derive(Default)]
struct CaptureRepository {
    preview_capture: Mutex<Option<Arc<RgbaImage>>>,
}

impl CaptureRepository {
    #[must_use]
    fn get_preview_capture(&self) -> Option<Arc<RgbaImage>> {
        self.preview_capture.lock().ok().and_then(|cache| cache.as_ref().cloned())
    }

    fn set_preview_capture(&self, image: Arc<RgbaImage>) {
        if let Ok(mut cache) = self.preview_capture.lock() {
            *cache = Some(image);
        }
    }
}

static CAPTURE_REPOSITORY: LazyLock<CaptureRepository> = LazyLock::new(CaptureRepository::default);

fn get_preview_capture() -> Option<Arc<RgbaImage>> {
    CAPTURE_REPOSITORY.get_preview_capture()
}

fn set_preview_capture(image: Arc<RgbaImage>) {
    CAPTURE_REPOSITORY.set_preview_capture(image);
}

#[must_use]
pub(crate) fn active_monitor_scale() -> f32 {
    active_monitor().and_then(|m| m.scale_factor().ok()).unwrap_or(1.0)
}

#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn perform_crop(image: &RgbaImage, rect: Rect, scale: f32) -> Option<RgbaImage> {
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
fn capture_active_monitor() -> Option<RgbaImage> {
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
fn active_monitor() -> Option<Monitor> {
    Monitor::all().ok()?.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image(width: u32, height: u32, value: u8) -> RgbaImage {
        RgbaImage::from_pixel(width, height, image::Rgba([value, value, value, 255]))
    }

    #[test]
    fn repository_updates_preview_cache() {
        let repository = CaptureRepository::default();
        let image = Arc::new(test_image(2, 3, 10));

        repository.set_preview_capture(image.clone());

        let cached = repository.get_preview_capture().unwrap();
        assert!(Arc::ptr_eq(&cached, &image));
        assert_eq!(cached.dimensions(), (2, 3));
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

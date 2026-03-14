pub mod action;
pub mod datasource;
pub mod scroll_worker;
pub mod service;
pub mod stitcher;

use crate::core::geometry::Rect;
use image::RgbaImage;
use std::sync::{Arc, LazyLock, Mutex};
use tracing::{debug, error};
use xcap::Monitor;

pub static LAST_CAPTURE: LazyLock<Mutex<Option<Arc<RgbaImage>>>> = LazyLock::new(|| Mutex::new(None));
pub static SCROLL_CAPTURE: LazyLock<Mutex<Option<Arc<RgbaImage>>>> = LazyLock::new(|| Mutex::new(None));

pub fn update_last_capture(image: RgbaImage) {
    if let Ok(mut cache) = LAST_CAPTURE.lock() {
        *cache = Some(Arc::new(image));
    }
}

#[must_use]
pub fn get_primary_monitor_scale() -> f32 {
    Monitor::all()
        .ok()
        .and_then(|m| m.first().and_then(|m| m.scale_factor().ok()))
        .unwrap_or(1.0)
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
pub fn capture_primary_monitor() -> Option<RgbaImage> {
    let monitors = Monitor::all().unwrap_or_default();
    let Some(monitor) = monitors.first() else {
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
pub fn get_monitors() -> Vec<Monitor> {
    Monitor::all().unwrap_or_default()
}

#[must_use]
pub fn get_primary_monitor() -> Option<Monitor> {
    Monitor::all().unwrap_or_default().into_iter().next()
}

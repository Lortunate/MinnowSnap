pub mod scroll_worker;
pub mod service;
pub mod stitcher;

use image::RgbaImage;
use log::{debug, error};
use std::sync::{LazyLock, Mutex};
use xcap::Monitor;

pub static LAST_CAPTURE: LazyLock<Mutex<Option<RgbaImage>>> = LazyLock::new(|| Mutex::new(None));
pub static SCROLL_CAPTURE: LazyLock<Mutex<Option<RgbaImage>>> = LazyLock::new(|| Mutex::new(None));

pub fn update_last_capture(image: RgbaImage) {
    if let Ok(mut cache) = LAST_CAPTURE.lock() {
        *cache = Some(image);
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
pub fn perform_crop(image: &RgbaImage, x: i32, y: i32, width: i32, height: i32, scale: f32) -> Option<RgbaImage> {
    debug!("Performing crop: rect={x},{y} {width}x{height}, scale={scale}");
    let img_w = image.width();
    let img_h = image.height();

    let x_phys = (x as f32 * scale) as i32;
    let y_phys = (y as f32 * scale) as i32;
    let w_phys = (width as f32 * scale) as i32;
    let h_phys = (height as f32 * scale) as i32;

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

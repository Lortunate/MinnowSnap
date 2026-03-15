pub mod action;
pub mod datasource;
pub mod scroll_worker;
pub mod service;
pub mod stitcher;

use crate::core::capture::datasource::VirtualCaptureSource;
use crate::core::geometry::Rect;
use image::RgbaImage;
use std::sync::{Arc, LazyLock, Mutex};
use tracing::{debug, error, info};
use xcap::Monitor;

pub static LAST_CAPTURE: LazyLock<Mutex<Option<Arc<RgbaImage>>>> = LazyLock::new(|| Mutex::new(None));
pub static SCROLL_CAPTURE: LazyLock<Mutex<Option<Arc<RgbaImage>>>> = LazyLock::new(|| Mutex::new(None));
pub static ACTIVE_MONITOR_TARGET: LazyLock<Mutex<Option<CaptureMonitorTarget>>> = LazyLock::new(|| Mutex::new(None));

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureMonitorTarget {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale_factor: f32,
}

impl CaptureMonitorTarget {
    #[must_use]
    pub fn from_monitor(monitor: &Monitor) -> Option<Self> {
        let width = i32::try_from(monitor.width().ok()?).ok()?;
        let height = i32::try_from(monitor.height().ok()?).ok()?;
        Some(Self {
            id: monitor.id().ok()?,
            x: monitor.x().ok()?,
            y: monitor.y().ok()?,
            width,
            height,
            scale_factor: monitor.scale_factor().ok().unwrap_or(1.0),
        })
    }

    #[must_use]
    pub fn effective_scale(self) -> f32 {
        if self.scale_factor <= 0.0 { 1.0 } else { self.scale_factor }
    }

    #[must_use]
    pub fn logical_geometry(self) -> (f64, f64, f64, f64) {
        let scale = f64::from(self.effective_scale());
        (
            f64::from(self.x) / scale,
            f64::from(self.y) / scale,
            f64::from(self.width) / scale,
            f64::from(self.height) / scale,
        )
    }

    #[must_use]
    pub fn center(self) -> (i32, i32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    #[must_use]
    pub fn rect(self) -> Rect {
        Rect::new(self.x, self.y, self.width, self.height)
    }
}

fn cache_cell(source: VirtualCaptureSource) -> &'static Mutex<Option<Arc<RgbaImage>>> {
    match source {
        VirtualCaptureSource::Preview => &LAST_CAPTURE,
        VirtualCaptureSource::Scroll => &SCROLL_CAPTURE,
    }
}

pub fn get_cached_capture(source: VirtualCaptureSource) -> Option<Arc<RgbaImage>> {
    cache_cell(source).lock().ok().and_then(|cache| cache.as_ref().cloned())
}

pub fn set_cached_capture(source: VirtualCaptureSource, image: RgbaImage) {
    if let Ok(mut cache) = cache_cell(source).lock() {
        *cache = Some(Arc::new(image));
    }
}

pub fn clear_cached_captures() {
    if let Ok(mut cache) = LAST_CAPTURE.lock() {
        *cache = None;
    }
    if let Ok(mut cache) = SCROLL_CAPTURE.lock() {
        *cache = None;
    }
}

pub fn update_last_capture(image: RgbaImage) {
    set_cached_capture(VirtualCaptureSource::Preview, image);
}

#[must_use]
pub fn active_monitor_scale() -> f32 {
    active_monitor_target()
        .map(CaptureMonitorTarget::effective_scale)
        .or_else(|| active_monitor().and_then(|monitor| monitor.scale_factor().ok()))
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
    resolve_active_monitor()
}

fn set_active_monitor_target(target: Option<CaptureMonitorTarget>) {
    if let Ok(mut cell) = ACTIVE_MONITOR_TARGET.lock() {
        *cell = target;
    }
}

#[must_use]
pub fn active_monitor_target() -> Option<CaptureMonitorTarget> {
    ACTIVE_MONITOR_TARGET.lock().ok().and_then(|cell| *cell)
}

fn primary_monitor_target() -> Option<CaptureMonitorTarget> {
    Monitor::all()
        .ok()
        .and_then(|monitors| monitors.into_iter().next())
        .and_then(|monitor| CaptureMonitorTarget::from_monitor(&monitor))
}

fn monitor_target_at_point(x: i32, y: i32) -> Option<CaptureMonitorTarget> {
    Monitor::from_point(x, y)
        .ok()
        .and_then(|monitor| CaptureMonitorTarget::from_monitor(&monitor))
}

#[must_use]
pub fn activate_monitor_at_point(x: i32, y: i32) -> Option<CaptureMonitorTarget> {
    let target = monitor_target_at_point(x, y).or_else(primary_monitor_target);
    if let Some(target) = target {
        info!(
            "Capture monitor selected: cursor=({},{}), id={}, scale={}, rect=({},{} {}x{})",
            x,
            y,
            target.id,
            target.effective_scale(),
            target.x,
            target.y,
            target.width,
            target.height
        );
    } else {
        error!("Capture monitor selection failed: cursor=({},{})", x, y);
    }
    set_active_monitor_target(target);
    target
}

fn monitor_matches_target(monitor: &Monitor, target: CaptureMonitorTarget) -> bool {
    if monitor.id().ok() == Some(target.id) {
        return true;
    }

    let width = i32::try_from(monitor.width().ok().unwrap_or_default()).ok().unwrap_or_default();
    let height = i32::try_from(monitor.height().ok().unwrap_or_default()).ok().unwrap_or_default();
    let x = monitor.x().ok().unwrap_or_default();
    let y = monitor.y().ok().unwrap_or_default();
    x == target.x && y == target.y && width == target.width && height == target.height
}

fn monitor_for_target(target: CaptureMonitorTarget) -> Option<Monitor> {
    let (cx, cy) = target.center();
    if let Ok(monitor) = Monitor::from_point(cx, cy) {
        return Some(monitor);
    }

    Monitor::all().ok()?.into_iter().find(|monitor| monitor_matches_target(monitor, target))
}

fn resolve_active_monitor() -> Option<Monitor> {
    if let Some(target) = active_monitor_target()
        && let Some(monitor) = monitor_for_target(target)
    {
        return Some(monitor);
    }
    Monitor::all().ok()?.into_iter().next()
}

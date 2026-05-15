pub mod action;
pub mod long_capture;
pub mod service;
pub mod source;
pub mod stitcher;

use crate::services::capture::source::VirtualCaptureSource;
use crate::services::geometry::Rect;
use image::RgbaImage;
use std::sync::{Arc, LazyLock, Mutex};
use tracing::{debug, error, info};
use xcap::Monitor;

static CAPTURE_REPOSITORY: LazyLock<CaptureRepository> = LazyLock::new(CaptureRepository::default);

#[derive(Default)]
pub struct CaptureRepository {
    last_capture: Mutex<Option<Arc<RgbaImage>>>,
    scroll_capture: Mutex<Option<Arc<RgbaImage>>>,
    active_monitor_target: Mutex<Option<CaptureMonitorTarget>>,
}

impl CaptureRepository {
    #[must_use]
    pub fn get_cached_capture(&self, source: VirtualCaptureSource) -> Option<Arc<RgbaImage>> {
        self.cache_cell(source).lock().ok().and_then(|cache| cache.as_ref().cloned())
    }

    pub fn set_cached_capture(&self, source: VirtualCaptureSource, image: RgbaImage) {
        if let Ok(mut cache) = self.cache_cell(source).lock() {
            *cache = Some(Arc::new(image));
        }
    }

    pub fn clear_cached_captures(&self) {
        if let Ok(mut cache) = self.last_capture.lock() {
            *cache = None;
        }
        if let Ok(mut cache) = self.scroll_capture.lock() {
            *cache = None;
        }
    }

    #[must_use]
    pub fn active_monitor_target(&self) -> Option<CaptureMonitorTarget> {
        self.active_monitor_target.lock().ok().and_then(|cell| *cell)
    }

    fn set_active_monitor_target(&self, target: Option<CaptureMonitorTarget>) {
        if let Ok(mut cell) = self.active_monitor_target.lock() {
            *cell = target;
        }
    }

    fn cache_cell(&self, source: VirtualCaptureSource) -> &Mutex<Option<Arc<RgbaImage>>> {
        match source {
            VirtualCaptureSource::Preview => &self.last_capture,
            VirtualCaptureSource::Scroll => &self.scroll_capture,
        }
    }
}

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

pub fn get_cached_capture(source: VirtualCaptureSource) -> Option<Arc<RgbaImage>> {
    CAPTURE_REPOSITORY.get_cached_capture(source)
}

pub fn set_cached_capture(source: VirtualCaptureSource, image: RgbaImage) {
    CAPTURE_REPOSITORY.set_cached_capture(source, image);
}

pub fn clear_cached_captures() {
    CAPTURE_REPOSITORY.clear_cached_captures();
}

pub fn update_last_capture(image: RgbaImage) {
    set_cached_capture(VirtualCaptureSource::Preview, image);
}

#[must_use]
pub fn active_monitor_scale() -> f32 {
    active_monitor()
        .and_then(|m| m.scale_factor().ok())
        .or_else(|| active_monitor_target().map(|t| t.effective_scale()))
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
    CAPTURE_REPOSITORY.set_active_monitor_target(target);
}

#[must_use]
pub fn active_monitor_target() -> Option<CaptureMonitorTarget> {
    CAPTURE_REPOSITORY.active_monitor_target()
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

fn describe_monitor(monitor: &Monitor) -> Option<String> {
    let target = CaptureMonitorTarget::from_monitor(monitor)?;
    Some(describe_target(target))
}

fn describe_target(target: CaptureMonitorTarget) -> String {
    let (logical_x, logical_y, logical_w, logical_h) = target.logical_geometry();
    format!(
        "id={}, scale={}, physical=({},{} {}x{}), logical=({:.2},{:.2} {:.2}x{:.2})",
        target.id,
        target.effective_scale(),
        target.x,
        target.y,
        target.width,
        target.height,
        logical_x,
        logical_y,
        logical_w,
        logical_h
    )
}

fn log_available_monitors() {
    match Monitor::all() {
        Ok(monitors) if !monitors.is_empty() => {
            let summary = monitors.iter().filter_map(describe_monitor).collect::<Vec<_>>().join("; ");
            info!("Available monitors for capture: [{}]", summary);
        }
        Ok(_) => {
            error!("Available monitors for capture: []");
        }
        Err(e) => {
            error!("Failed to enumerate monitors before capture selection: {e}");
        }
    }
}

#[must_use]
pub fn activate_monitor_at_point(x: i32, y: i32) -> Option<CaptureMonitorTarget> {
    log_available_monitors();

    let target = monitor_target_at_point(x, y).or_else(primary_monitor_target);
    if let Some(target) = target {
        info!("Capture monitor selected: cursor=({},{}), {}", x, y, describe_target(target));
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
    if let Some(target) = active_monitor_target() {
        if let Some(monitor) = monitor_for_target(target) {
            return Some(monitor);
        }
        set_active_monitor_target(None);
    }
    Monitor::all().ok()?.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_image(width: u32, height: u32, value: u8) -> RgbaImage {
        RgbaImage::from_pixel(width, height, image::Rgba([value, value, value, 255]))
    }

    fn test_target(scale_factor: f32) -> CaptureMonitorTarget {
        CaptureMonitorTarget {
            id: 7,
            x: 10,
            y: 20,
            width: 300,
            height: 200,
            scale_factor,
        }
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
    fn repository_clear_drops_cached_captures() {
        let repository = CaptureRepository::default();
        repository.set_cached_capture(VirtualCaptureSource::Preview, test_image(1, 1, 10));
        repository.set_cached_capture(VirtualCaptureSource::Scroll, test_image(1, 1, 20));

        repository.clear_cached_captures();

        assert!(repository.get_cached_capture(VirtualCaptureSource::Preview).is_none());
        assert!(repository.get_cached_capture(VirtualCaptureSource::Scroll).is_none());
    }

    #[test]
    fn repository_tracks_active_monitor_target() {
        let repository = CaptureRepository::default();
        let target = test_target(1.5);

        repository.set_active_monitor_target(Some(target));
        assert_eq!(repository.active_monitor_target(), Some(target));

        repository.set_active_monitor_target(None);
        assert_eq!(repository.active_monitor_target(), None);
    }

    #[test]
    fn monitor_target_sanitizes_invalid_scale() {
        assert_eq!(test_target(0.0).effective_scale(), 1.0);
        assert_eq!(test_target(-2.0).effective_scale(), 1.0);
        assert_eq!(test_target(2.0).effective_scale(), 2.0);
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

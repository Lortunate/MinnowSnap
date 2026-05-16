use crate::services::capture::source::VirtualCaptureSource;
use image::RgbaImage;
use std::sync::{Arc, Mutex};

use super::CaptureMonitorTarget;

#[derive(Default)]
pub(super) struct CaptureRepository {
    last_capture: Mutex<Option<Arc<RgbaImage>>>,
    scroll_capture: Mutex<Option<Arc<RgbaImage>>>,
    active_monitor_target: Mutex<Option<CaptureMonitorTarget>>,
}

impl CaptureRepository {
    #[must_use]
    pub(super) fn get_cached_capture(&self, source: VirtualCaptureSource) -> Option<Arc<RgbaImage>> {
        self.cache_cell(source).lock().ok().and_then(|cache| cache.as_ref().cloned())
    }

    pub(super) fn set_cached_capture(&self, source: VirtualCaptureSource, image: RgbaImage) {
        if let Ok(mut cache) = self.cache_cell(source).lock() {
            *cache = Some(Arc::new(image));
        }
    }

    pub(super) fn clear_cached_captures(&self) {
        if let Ok(mut cache) = self.last_capture.lock() {
            *cache = None;
        }
        if let Ok(mut cache) = self.scroll_capture.lock() {
            *cache = None;
        }
    }

    #[must_use]
    pub(super) fn active_monitor_target(&self) -> Option<CaptureMonitorTarget> {
        self.active_monitor_target.lock().ok().and_then(|cell| *cell)
    }

    pub(super) fn set_active_monitor_target(&self, target: Option<CaptureMonitorTarget>) {
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

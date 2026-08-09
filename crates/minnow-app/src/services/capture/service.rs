use crate::services::capture::action::CaptureInputMode;
use crate::services::geometry::Rect;
use image::RgbaImage;
use std::sync::Arc;
use tracing::{error, info};

use super::{
    active_monitor_scale, capture_active_monitor, crop_scaled_region, crop_selection, get_preview_capture, normalize_virtual_source,
    set_preview_capture,
};

pub(crate) struct CaptureService;

pub(crate) enum ResolvedCaptureImage {
    Shared(Arc<RgbaImage>),
    Owned(RgbaImage),
}

impl ResolvedCaptureImage {
    pub(crate) fn as_rgba(&self) -> &RgbaImage {
        match self {
            Self::Shared(image) => image.as_ref(),
            Self::Owned(image) => image,
        }
    }

    pub(crate) fn into_arc(self) -> Arc<RgbaImage> {
        match self {
            Self::Shared(image) => image,
            Self::Owned(image) => Arc::new(image),
        }
    }

    fn crop_selection(self, rect: Rect) -> Option<Self> {
        let cropped = match &self {
            Self::Shared(image) => crop_selection(image.as_ref(), rect),
            Self::Owned(image) => crop_selection(image, rect),
        }?;
        Some(Self::Owned(cropped))
    }
}

impl CaptureService {
    fn is_full_request(rect: Rect, input_mode: CaptureInputMode) -> bool {
        input_mode == CaptureInputMode::FullImage || !rect.has_area()
    }

    fn get_cached_source_image(path_str: &str) -> Option<Arc<RgbaImage>> {
        (normalize_virtual_source(path_str) == super::PREVIEW_SOURCE)
            .then(get_preview_capture)
            .flatten()
    }

    fn resolve_image_from_path(path_str: &str) -> Option<RgbaImage> {
        match image::open(normalize_virtual_source(path_str)) {
            Ok(img) => Some(img.to_rgba8()),
            Err(e) => {
                error!("Failed to load source image '{path_str}': {e}");
                None
            }
        }
    }

    fn resolve_source_image(path_str: &str) -> Option<ResolvedCaptureImage> {
        if let Some(shared) = Self::get_cached_source_image(path_str) {
            return Some(ResolvedCaptureImage::Shared(shared));
        }
        Self::resolve_image_from_path(path_str).map(ResolvedCaptureImage::Owned)
    }

    pub(crate) fn capture_region(rect: Rect) -> Option<RgbaImage> {
        info!("Capturing region: x={}, y={}, w={}, h={}", rect.x, rect.y, rect.width, rect.height);
        let scale_factor = active_monitor_scale();

        if rect.has_area() {
            if let Some(monitor_img) = capture_active_monitor() {
                crop_scaled_region(&monitor_img, rect, scale_factor)
            } else {
                None
            }
        } else {
            capture_active_monitor()
        }
    }

    pub(crate) fn capture_preview() -> Option<Arc<RgbaImage>> {
        let image = Arc::new(Self::capture_region(Rect::empty())?);
        set_preview_capture(image.clone());
        Some(image)
    }

    pub(crate) fn resolve_capture_image(path: &str, rect: Rect, input_mode: CaptureInputMode) -> Option<ResolvedCaptureImage> {
        let image = Self::resolve_source_image(path)?;
        if Self::is_full_request(rect, input_mode) {
            Some(image)
        } else {
            image.crop_selection(rect)
        }
    }

    pub(crate) fn resolve_rgba_image(image: Arc<RgbaImage>, rect: Rect, input_mode: CaptureInputMode) -> Option<ResolvedCaptureImage> {
        if Self::is_full_request(rect, input_mode) {
            Some(ResolvedCaptureImage::Shared(image))
        } else {
            ResolvedCaptureImage::Shared(image).crop_selection(rect)
        }
    }

    pub(crate) fn decode_qrcode(image: &RgbaImage) -> Option<String> {
        let gray = image::imageops::grayscale(image);
        let (w, h) = gray.dimensions();
        let mut img = rqrr::PreparedImage::prepare_from_greyscale(w as usize, h as usize, |x, y| gray.get_pixel(x as u32, y as u32)[0]);
        let grids = img.detect_grids();
        let grid = grids.first()?;
        let (_meta, content) = grid.decode().ok()?;
        Some(content)
    }
}

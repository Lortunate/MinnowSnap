use crate::platform::clipboard::copy_image_to_clipboard;
use crate::platform::notify;
use crate::platform::storage::{save_image_to_user_dir, save_temp_image};
use crate::services::capture::action::CaptureInputMode;
use crate::services::geometry::Rect;
use crate::services::settings;
use image::RgbaImage;
use std::sync::Arc;
use tracing::{error, info};

use super::{VirtualCaptureSource, active_monitor_scale, capture_active_monitor, get_cached_capture, normalize_virtual_source, perform_crop};

pub struct CaptureService;

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
}

impl CaptureService {
    fn is_full_request(rect: Rect, input_mode: CaptureInputMode) -> bool {
        input_mode == CaptureInputMode::FullImage || !rect.has_area()
    }

    fn parse_cached_source(path_str: &str) -> Option<VirtualCaptureSource> {
        match normalize_virtual_source(path_str) {
            super::PREVIEW_SOURCE => Some(VirtualCaptureSource::Preview),
            super::SCROLL_SOURCE => Some(VirtualCaptureSource::Scroll),
            _ => None,
        }
    }

    fn get_cached_source_image(path_str: &str) -> Option<Arc<RgbaImage>> {
        let source = Self::parse_cached_source(path_str)?;
        get_cached_capture(source)
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

    fn crop_selection(img: &RgbaImage, rect: Rect) -> Option<RgbaImage> {
        let scale_factor = active_monitor_scale();
        let x_phys = (rect.x as f32 * scale_factor) as i32;
        let y_phys = (rect.y as f32 * scale_factor) as i32;
        let w_phys = (rect.width as f32 * scale_factor) as i32;
        let h_phys = (rect.height as f32 * scale_factor) as i32;
        let img_w = img.width() as i32;
        let img_h = img.height() as i32;

        let exceeds_bounds = x_phys < 0
            || y_phys < 0
            || x_phys >= img_w
            || y_phys >= img_h
            || x_phys.saturating_add(w_phys) > img_w
            || y_phys.saturating_add(h_phys) > img_h;

        let almost_full_image = (w_phys - img_w).abs() <= 2 && (h_phys - img_h).abs() <= 2;
        if exceeds_bounds && almost_full_image {
            return Some(img.clone());
        }

        perform_crop(img, rect, scale_factor)
    }

    pub fn capture_region(rect: Rect) -> Option<RgbaImage> {
        info!("Capturing region: x={}, y={}, w={}, h={}", rect.x, rect.y, rect.width, rect.height);
        let scale_factor = active_monitor_scale();

        if rect.has_area() {
            if let Some(monitor_img) = capture_active_monitor() {
                perform_crop(&monitor_img, rect, scale_factor)
            } else {
                None
            }
        } else {
            capture_active_monitor()
        }
    }

    pub(crate) fn resolve_capture_image(path: &str, rect: Rect, input_mode: CaptureInputMode) -> Option<ResolvedCaptureImage> {
        match Self::resolve_source_image(path)? {
            ResolvedCaptureImage::Shared(img) => {
                if Self::is_full_request(rect, input_mode) {
                    Some(ResolvedCaptureImage::Shared(img))
                } else {
                    Self::crop_selection(img.as_ref(), rect).map(ResolvedCaptureImage::Owned)
                }
            }
            ResolvedCaptureImage::Owned(img) => {
                if Self::is_full_request(rect, input_mode) {
                    Some(ResolvedCaptureImage::Owned(img))
                } else {
                    Self::crop_selection(&img, rect).map(ResolvedCaptureImage::Owned)
                }
            }
        }
    }

    pub(crate) fn resolve_rgba_image(image: Arc<RgbaImage>, rect: Rect, input_mode: CaptureInputMode) -> Option<ResolvedCaptureImage> {
        if Self::is_full_request(rect, input_mode) {
            Some(ResolvedCaptureImage::Shared(image))
        } else {
            Self::crop_selection(image.as_ref(), rect).map(ResolvedCaptureImage::Owned)
        }
    }

    pub(crate) fn copy_rgba(image: &RgbaImage) -> bool {
        copy_image_to_clipboard(image)
    }

    pub(crate) fn save_rgba_to_user_dir(image: &RgbaImage, save_path_override: Option<String>) -> Result<String, String> {
        let settings = settings::output_settings();
        let save_path = save_path_override.or(settings.save_path);

        let result = save_image_to_user_dir(image, settings.oxipng_enabled, save_path);
        if result.is_some() {
            notify::play_shutter();
        }
        result.ok_or_else(|| "Failed to save image to disk".to_string())
    }

    pub fn run_quick_capture_workflow(rect: Rect) -> bool {
        info!("Starting quick capture workflow");
        let Some(image) = Self::capture_region(rect) else {
            error!("Failed to capture quick capture image");
            return false;
        };

        if !copy_image_to_clipboard(&image) {
            error!("Failed to copy quick capture image to clipboard");
            return false;
        }

        notify::play_shutter();
        info!("Quick capture image copied to clipboard");
        true
    }

    pub fn save_temp(image: &RgbaImage) -> Option<String> {
        save_temp_image(image, false).map(|path| path.replace('\\', "/"))
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

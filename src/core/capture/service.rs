use crate::core::capture::action::CaptureInputMode;
use crate::core::capture::datasource::{self, VirtualCaptureSource};
use crate::core::capture::{active_monitor_scale, capture_active_monitor, get_cached_capture, perform_crop, update_last_capture};
use crate::core::geometry::Rect;
use crate::core::io::clipboard::copy_image_to_clipboard;
use crate::core::io::storage::{save_image_to_unique_temp, save_image_to_user_dir};
use crate::core::settings::SETTINGS;
use crate::core::window::fetch_windows_data;
use gpui::RenderImage;
use image::Frame;
use image::RgbaImage;
use std::sync::Arc;
use tracing::{error, info};

pub struct CaptureService;

enum SourceImage {
    Shared(Arc<RgbaImage>),
    Owned(RgbaImage),
}

impl CaptureService {
    pub(crate) fn render_image_from_rgba(mut image: RgbaImage) -> Arc<RenderImage> {
        for pixel in image.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }

        Arc::new(RenderImage::new([Frame::new(image)]))
    }

    fn is_full_request(rect: Rect, input_mode: CaptureInputMode) -> bool {
        input_mode == CaptureInputMode::FullImage || !rect.has_area()
    }

    fn parse_cached_source(path_str: &str) -> Option<VirtualCaptureSource> {
        datasource::parse_virtual_source(path_str)
    }

    fn get_cached_source_image(path_str: &str) -> Option<Arc<RgbaImage>> {
        let source = Self::parse_cached_source(path_str)?;
        get_cached_capture(source)
    }

    fn resolve_image_from_path(path_str: &str) -> Option<RgbaImage> {
        match image::open(datasource::normalize_virtual_source(path_str)) {
            Ok(img) => Some(img.to_rgba8()),
            Err(e) => {
                error!("Failed to load source image '{path_str}': {e}");
                None
            }
        }
    }

    fn resolve_source_image(path_str: &str) -> Option<SourceImage> {
        if let Some(shared) = Self::get_cached_source_image(path_str) {
            return Some(SourceImage::Shared(shared));
        }
        Self::resolve_image_from_path(path_str).map(SourceImage::Owned)
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

    fn resolve_from_shared_image(img: Arc<RgbaImage>, rect: Rect, input_mode: CaptureInputMode) -> Option<RgbaImage> {
        if Self::is_full_request(rect, input_mode) {
            return Some(img.as_ref().clone());
        }
        Self::crop_selection(img.as_ref(), rect)
    }

    fn resolve_from_owned_image(img: RgbaImage, rect: Rect, input_mode: CaptureInputMode) -> Option<RgbaImage> {
        if Self::is_full_request(rect, input_mode) {
            return Some(img);
        }
        Self::crop_selection(&img, rect)
    }

    pub fn capture_screen() -> bool {
        info!("Starting screen capture...");
        if let Some(image) = capture_active_monitor() {
            update_last_capture(image);
            info!("Screen capture successful");
            true
        } else {
            error!("CaptureService: Failed to capture active monitor");
            false
        }
    }

    pub fn fetch_windows_json() -> String {
        let windows = fetch_windows_data();
        serde_json::to_string(&windows).unwrap_or_else(|_| "[]".to_string())
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

    pub fn resolve_image(path: &str, rect: Rect, input_mode: CaptureInputMode) -> Option<RgbaImage> {
        match Self::resolve_source_image(path)? {
            SourceImage::Shared(img) => Self::resolve_from_shared_image(img, rect, input_mode),
            SourceImage::Owned(img) => Self::resolve_from_owned_image(img, rect, input_mode),
        }
    }

    pub fn copy_image(path: &str, rect: Rect, input_mode: CaptureInputMode) -> bool {
        if Self::is_full_request(rect, input_mode)
            && let Some(shared) = Self::get_cached_source_image(path)
        {
            return copy_image_to_clipboard(shared.as_ref());
        }

        if let Some(img) = Self::resolve_image(path, rect, input_mode) {
            return copy_image_to_clipboard(&img);
        }
        false
    }

    pub fn save_region_to_user_dir(path: &str, rect: Rect, input_mode: CaptureInputMode) -> Result<String, String> {
        let img = Self::resolve_image(path, rect, input_mode).ok_or_else(|| "Failed to resolve or crop image for saving".to_string())?;

        let settings = SETTINGS.lock().map_err(|_| "Failed to lock settings".to_string())?.get();

        let result = save_image_to_user_dir(&img, settings.output.oxipng_enabled, settings.output.save_path);
        if result.is_some() {
            crate::core::notify::play_shutter();
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

        crate::core::notify::play_shutter();
        info!("Quick capture image copied to clipboard");
        true
    }

    pub fn generate_temp_path(extension: &str) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let filename = format!("minnow_capture_{}.{}", timestamp, extension);
        let mut path = std::env::temp_dir();
        path.push(filename);
        path.to_string_lossy().replace('\\', "/")
    }

    pub fn save_temp(image: &RgbaImage) -> Option<String> {
        save_image_to_unique_temp(image, false).map(|path| path.replace('\\', "/"))
    }

    pub fn detect_qrcode(path: &str, rect: Rect, input_mode: CaptureInputMode) -> Option<String> {
        if let Some(cropped) = Self::resolve_image(path, rect, input_mode) {
            let gray = image::imageops::grayscale(&cropped);
            let (w, h) = gray.dimensions();
            let mut img = rqrr::PreparedImage::prepare_from_greyscale(w as usize, h as usize, |x, y| gray.get_pixel(x as u32, y as u32)[0]);
            let grids = img.detect_grids();
            if let Some(grid) = grids.first()
                && let Ok((_meta, content)) = grid.decode()
            {
                return Some(content);
            }
        }
        None
    }

    pub fn get_pixel_hex(x: i32, y: i32, scale: f64) -> Option<String> {
        let x_phys = (x as f64 * scale) as i32;
        let y_phys = (y as f64 * scale) as i32;
        let img = get_cached_capture(VirtualCaptureSource::Preview)?;
        let (Ok(u_x), Ok(u_y)) = (u32::try_from(x_phys), u32::try_from(y_phys)) else {
            return None;
        };
        if u_x >= img.width() || u_y >= img.height() {
            return None;
        }
        let pixel = img.get_pixel(u_x, u_y);
        Some(format!("#{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2]))
    }
}

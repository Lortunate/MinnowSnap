use crate::core::capture::action::CaptureInputMode;
use crate::core::capture::{LAST_CAPTURE, capture_primary_monitor, get_primary_monitor_scale, perform_crop, update_last_capture};
use crate::core::io::storage::{save_image_to_unique_temp, save_image_to_user_dir};
use crate::core::settings::SETTINGS;
use crate::core::window::fetch_windows_data;
use image::RgbaImage;
use tracing::{error, info};

pub struct CaptureService;

impl CaptureService {
    fn resolve_image_from_path(path_str: &str) -> Option<RgbaImage> {
        if (path_str.is_empty() || path_str.starts_with("image://minnow/preview"))
            && let Ok(cache) = LAST_CAPTURE.lock()
            && let Some(cached_img) = &*cache
        {
            return Some(cached_img.clone());
        }

        match image::open(path_str) {
            Ok(img) => Some(img.to_rgba8()),
            Err(e) => {
                error!("Failed to load source image '{path_str}': {e}");
                None
            }
        }
    }

    pub fn capture_screen() -> bool {
        info!("Starting screen capture...");
        if let Some(image) = capture_primary_monitor() {
            update_last_capture(image);
            info!("Screen capture successful");
            true
        } else {
            error!("CaptureService: Failed to capture primary monitor");
            false
        }
    }

    pub fn fetch_windows_json() -> String {
        let windows = fetch_windows_data();
        serde_json::to_string(&windows).unwrap_or_else(|_| "[]".to_string())
    }

    pub fn capture_region(x: i32, y: i32, width: i32, height: i32) -> Option<RgbaImage> {
        info!("Capturing region: x={}, y={}, w={}, h={}", x, y, width, height);
        let scale_factor = get_primary_monitor_scale();

        if width > 0 && height > 0 {
            if let Some(monitor_img) = capture_primary_monitor() {
                perform_crop(&monitor_img, x, y, width, height, scale_factor)
            } else {
                None
            }
        } else {
            capture_primary_monitor()
        }
    }

    pub fn resolve_image(path: &str, x: i32, y: i32, width: i32, height: i32, input_mode: CaptureInputMode) -> Option<RgbaImage> {
        let img = Self::resolve_image_from_path(path)?;

        if input_mode == CaptureInputMode::FullImage {
            return Some(img);
        }

        if width <= 0 || height <= 0 {
            return Some(img);
        }

        let scale_factor = get_primary_monitor_scale();
        let x_phys = (x as f32 * scale_factor) as i32;
        let y_phys = (y as f32 * scale_factor) as i32;
        let w_phys = (width as f32 * scale_factor) as i32;
        let h_phys = (height as f32 * scale_factor) as i32;
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
            return Some(img);
        }

        perform_crop(&img, x, y, width, height, scale_factor)
    }

    pub fn save_region_to_user_dir(path: &str, x: i32, y: i32, width: i32, height: i32, input_mode: CaptureInputMode) -> Result<String, String> {
        let img =
            Self::resolve_image(path, x, y, width, height, input_mode).ok_or_else(|| "Failed to resolve or crop image for saving".to_string())?;

        let settings = SETTINGS.lock().map_err(|_| "Failed to lock settings".to_string())?.get();

        let result = save_image_to_user_dir(&img, settings.output.oxipng_enabled, settings.output.save_path);
        if result.is_some() {
            crate::core::notify::play_shutter();
        }
        result.ok_or_else(|| "Failed to save image to disk".to_string())
    }

    pub fn run_quick_capture_workflow(x: i32, y: i32, width: i32, height: i32) -> Option<String> {
        info!("Starting quick capture workflow");
        let image = Self::capture_region(x, y, width, height)?;
        crate::core::notify::play_shutter();

        let settings = SETTINGS.lock().ok()?.get();
        let configured_path = settings.output.save_path.clone();

        info!("Retrieved configured save path: {:?}", configured_path);

        match save_image_to_user_dir(&image, settings.output.oxipng_enabled, configured_path) {
            Some(saved) => {
                info!("Quick capture auto-saved to: {}", saved);
                Some(saved)
            }
            None => {
                error!("Failed to save quick capture image");
                None
            }
        }
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

    pub fn detect_qrcode(path: &str, x: i32, y: i32, width: i32, height: i32, input_mode: CaptureInputMode) -> Option<String> {
        if let Some(cropped) = Self::resolve_image(path, x, y, width, height, input_mode) {
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
}

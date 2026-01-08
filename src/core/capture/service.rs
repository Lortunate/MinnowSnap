use crate::core::capture::{capture_primary_monitor, get_primary_monitor_scale, perform_crop, update_last_capture, LAST_CAPTURE};
use crate::core::io::clipboard::copy_image_to_clipboard;
use crate::core::io::storage::{save_image_to_unique_temp, save_image_to_user_dir};
use crate::core::settings::SETTINGS;
use crate::core::window::fetch_windows_data;
use image::RgbaImage;
use log::{error, info};

pub struct CaptureService;

impl CaptureService {
    fn resolve_image_from_path(path_str: &str) -> Option<RgbaImage> {
        if (path_str.is_empty() || path_str.starts_with("image://minnow/preview"))
            && let Ok(cache) = LAST_CAPTURE.lock()
            && let Some(cached_img) = &*cache
        {
            return Some(cached_img.clone());
        }

        let file_path = path_str.strip_prefix("file://").unwrap_or(path_str);
        match image::open(file_path) {
            Ok(img) => Some(img.to_rgba8()),
            Err(e) => {
                error!("Failed to load source image '{file_path}': {e}");
                None
            }
        }
    }

    pub fn prepare_capture() -> Option<(RgbaImage, String)> {
        let windows = fetch_windows_data();
        let json = serde_json::to_string(&windows).unwrap_or_else(|_| "[]".to_string());

        if let Some(image) = capture_primary_monitor() {
            update_last_capture(image.clone());
            Some((image, json))
        } else {
            error!("CaptureService: Failed to capture primary monitor");
            None
        }
    }

    pub fn capture_region(x: i32, y: i32, width: i32, height: i32) -> Option<RgbaImage> {
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

    pub fn resolve_and_crop(path: &str, x: i32, y: i32, width: i32, height: i32) -> Option<RgbaImage> {
        let img = Self::resolve_image_from_path(path)?;

        if width <= 0 || height <= 0 {
            return Some(img);
        }

        let scale_factor = get_primary_monitor_scale();
        perform_crop(&img, x, y, width, height, scale_factor)
    }

    pub fn save_temp(image: &RgbaImage) -> Option<String> {
        save_image_to_unique_temp(image, false).map(|path| {
            let mut p = path.replace('\\', "/");
            if cfg!(target_os = "windows") && !p.starts_with('/') {
                p = format!("/{}", p);
            }
            format!("file://{}", p)
        })
    }

    pub fn copy_region_to_clipboard(path: &str, x: i32, y: i32, width: i32, height: i32) -> Result<(), String> {
        let img = Self::resolve_and_crop(path, x, y, width, height).ok_or_else(|| "Failed to resolve or crop image for clipboard".to_string())?;

        if copy_image_to_clipboard(&img) {
            Ok(())
        } else {
            Err("Failed to copy image to clipboard".to_string())
        }
    }

    pub fn save_region_to_user_dir(path: &str, x: i32, y: i32, width: i32, height: i32) -> Result<String, String> {
        let img = Self::resolve_and_crop(path, x, y, width, height).ok_or_else(|| "Failed to resolve or crop image for saving".to_string())?;

        let settings = SETTINGS.lock().map_err(|_| "Failed to lock settings".to_string())?.get();

        save_image_to_user_dir(&img, settings.output.oxipng_enabled, settings.output.save_path).ok_or_else(|| "Failed to save image to disk".to_string())
    }

    pub fn run_quick_capture_workflow(x: i32, y: i32, width: i32, height: i32) -> Option<(String, Option<String>)> {
        let image = Self::capture_region(x, y, width, height)?;
        let temp_path = Self::save_temp(&image)?;

        let mut final_save_path = None;
        let settings_guard = SETTINGS.lock().ok()?;
        let settings = settings_guard.get();

        if let Some(saved) = save_image_to_user_dir(&image, settings.output.oxipng_enabled, settings.output.save_path.clone()) {
            info!("Quick capture auto-saved to: {}", saved);
            final_save_path = Some(saved);
        }

        Some((temp_path, final_save_path))
    }

    pub fn generate_temp_path(extension: &str) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let filename = format!("minnow_capture_{}.{}", timestamp, extension);
        let mut path = std::env::temp_dir();
        path.push(filename);
        path.to_string_lossy().replace('\\', "/")
    }
}

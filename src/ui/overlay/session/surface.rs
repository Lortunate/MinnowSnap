use gpui::RenderImage;
use image::RgbaImage;
use std::sync::Arc;

use crate::core::capture::service::CaptureService;
use crate::core::window::{WindowInfo, fetch_windows_data};

#[derive(Clone, Debug, Default)]
pub(crate) struct OverlaySurface {
    pub background_image: Option<Arc<RenderImage>>,
    pub background_pixels: Option<Arc<RgbaImage>>,
    pub background_path: Option<String>,
    pub windows: Vec<WindowInfo>,
}

impl OverlaySurface {
    pub fn capture() -> Self {
        let windows = fetch_windows_data();
        let background_path =
            CaptureService::capture_region(crate::core::geometry::Rect::empty()).and_then(|image| CaptureService::save_temp(&image));

        match background_path {
            Some(path) => Self {
                background_image: CaptureService::render_image_from_path(&path),
                background_pixels: image::open(&path).ok().map(|img| Arc::new(img.to_rgba8())),
                background_path: Some(path),
                windows,
            },
            None => Self { windows, ..Self::default() },
        }
    }
}

use gpui::RenderImage;
use image::RgbaImage;
use std::sync::Arc;

use crate::core::capture::service::CaptureService;
use crate::core::capture::update_last_capture;
use crate::core::window::{WindowInfo, fetch_windows_data};

#[derive(Clone, Debug, Default)]
pub(crate) struct OverlaySurface {
    pub background_image: Option<Arc<RenderImage>>,
    pub background_pixels: Option<Arc<RgbaImage>>,
    pub windows: Vec<WindowInfo>,
}

impl OverlaySurface {
    pub fn capture() -> Self {
        let windows = fetch_windows_data();
        match CaptureService::capture_region(crate::core::geometry::Rect::empty()) {
            Some(image) => {
                update_last_capture(image.clone());
                let background_image = Some(CaptureService::render_image_from_rgba(image.clone()));
                let background_pixels = Some(Arc::new(image));
                Self {
                    background_image,
                    background_pixels,
                    windows,
                }
            }
            None => Self { windows, ..Self::default() },
        }
    }
}

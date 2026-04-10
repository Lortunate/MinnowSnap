use gpui::RenderImage;
use image::RgbaImage;
use std::sync::Arc;

use crate::features::overlay::window_catalog::{WindowInfo, fetch_windows_data};
use crate::support::render_image;
use minnow_core::capture::service::CaptureService;
use minnow_core::capture::update_last_capture;

#[derive(Clone, Debug, Default)]
pub(crate) struct OverlaySurface {
    pub background_image: Option<Arc<RenderImage>>,
    pub background_pixels: Option<Arc<RgbaImage>>,
    pub windows: Vec<WindowInfo>,
}

impl OverlaySurface {
    pub fn capture() -> Self {
        let windows = fetch_windows_data();
        match CaptureService::capture_region(minnow_core::geometry::Rect::empty()) {
            Some(image) => {
                update_last_capture(image.clone());
                let background_image = Some(render_image::from_rgba(image.clone()));
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

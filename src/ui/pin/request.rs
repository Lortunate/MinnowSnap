use crate::core::geometry::Rect;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct PinRequest {
    pub(super) image_path: PathBuf,
    image_size: Option<(u32, u32)>,
    source_bounds: Option<Rect>,
}

impl PinRequest {
    pub fn new(image_path: impl Into<PathBuf>, source_bounds: Option<Rect>) -> Self {
        let image_path = image_path.into();
        let image_size = image::image_dimensions(&image_path).ok();
        Self {
            image_path,
            image_size,
            source_bounds,
        }
    }

    pub(crate) fn source_bounds(&self) -> Option<Rect> {
        self.source_bounds.filter(|bounds| bounds.has_area())
    }

    pub(crate) fn base_size(&self) -> (f32, f32) {
        if let Some(source) = self.source_bounds() {
            return (source.width as f32, source.height as f32);
        }

        match self.image_size {
            Some((width, height)) => (width as f32, height as f32),
            None => (960.0, 720.0),
        }
    }
}

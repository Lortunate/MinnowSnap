use minnow_application::PreparedImage;
use minnow_imaging::{ImageRef, RectF};

#[derive(Clone, Debug)]
pub struct PinRequest {
    image: ImageRef,
    source_bounds: Option<RectF>,
    auto_ocr: bool,
}

impl PinRequest {
    pub(crate) fn new(image: ImageRef, source_bounds: Option<RectF>, auto_ocr: bool) -> Self {
        Self {
            image,
            source_bounds,
            auto_ocr,
        }
    }

    pub(crate) fn from_application(prepared: PreparedImage, auto_ocr: bool) -> Self {
        Self::new(prepared.image, prepared.source_bounds, auto_ocr)
    }

    pub(crate) fn image(&self) -> ImageRef {
        self.image.clone()
    }
    pub(super) fn validate(&self) -> minnow_application::OperationResult<()> {
        if let Some(bounds) = self.source_bounds {
            let values = [
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                bounds.x + bounds.width,
                bounds.y + bounds.height,
            ];
            if values.into_iter().any(|value| !value.is_finite() || !(value as f32).is_finite()) || bounds.width <= 0.0 || bounds.height <= 0.0 {
                return Err(minnow_application::OperationError::new(
                    minnow_application::ErrorKind::InvalidInput,
                    "pin source bounds must be finite and have a positive area",
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn origin(&self) -> Option<(f32, f32)> {
        self.source_bounds().map(|bounds| (bounds.x as f32, bounds.y as f32))
    }
    pub(crate) fn source_bounds(&self) -> Option<RectF> {
        self.source_bounds.filter(|bounds| bounds.width > 0.0 && bounds.height > 0.0)
    }
    pub(crate) fn auto_ocr(&self) -> bool {
        self.auto_ocr
    }
    pub(crate) fn base_size(&self) -> (f32, f32) {
        if let Some(bounds) = self.source_bounds() {
            return (bounds.width as f32, bounds.height as f32);
        }
        (self.image.width() as f32, self.image.height() as f32)
    }
}

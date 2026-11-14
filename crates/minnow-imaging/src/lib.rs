//! Immutable images and explicit coordinate transforms shared by capture and editing.

pub mod geometry;

use image::RgbaImage;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub use geometry::{Rect, RectF};

static NEXT_IMAGE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImageId(u64);

impl ImageId {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Shared immutable pixels. An asset retains its identity across cheap clones.
pub type ImageRef = Arc<ImageAsset>;

#[derive(Debug)]
pub struct ImageAsset {
    id: ImageId,
    pixels: Arc<RgbaImage>,
}

impl ImageAsset {
    pub fn new(pixels: RgbaImage) -> ImageRef {
        Self::from_shared(Arc::new(pixels))
    }

    pub fn from_shared(pixels: Arc<RgbaImage>) -> ImageRef {
        Arc::new(Self {
            id: ImageId(NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed)),
            pixels,
        })
    }

    pub fn id(&self) -> ImageId {
        self.id
    }

    pub fn pixels(&self) -> &RgbaImage {
        &self.pixels
    }

    pub fn shared_pixels(&self) -> Arc<RgbaImage> {
        self.pixels.clone()
    }

    pub fn dimensions(&self) -> (u32, u32) {
        self.pixels.dimensions()
    }

    pub fn width(&self) -> u32 {
        self.pixels.width()
    }

    pub fn height(&self) -> u32 {
        self.pixels.height()
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DisplayId(pub String);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenPoint {
    pub x: f64,
    pub y: f64,
}

/// Logical coordinates local to the captured display, not desktop coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewRect(pub RectF);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// One selection resolved against the source pixels.
///
/// The logical bounds describe the actual pixel crop, including rounding and
/// clipping. Consumers must use this origin when translating local annotations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedSelection {
    source_size: (u32, u32),
    scale_factor: f64,
    pixel_rect: ImageRect,
    view_rect: ViewRect,
}

impl ResolvedSelection {
    pub fn resolve(selection: ViewRect, scale: f64, pixel_size: (u32, u32)) -> Result<Self, GeometryError> {
        let r = selection.0;
        if !scale.is_finite()
            || scale <= 0.0
            || ![r.x, r.y, r.width, r.height, r.x + r.width, r.y + r.height]
                .into_iter()
                .all(f64::is_finite)
        {
            return Err(GeometryError::InvalidTransform);
        }
        if pixel_size.0 == 0 || pixel_size.1 == 0 || r.width <= 0.0 || r.height <= 0.0 {
            return Err(GeometryError::Empty);
        }
        let left = (r.x * scale).floor().clamp(0.0, pixel_size.0 as f64) as u32;
        let top = (r.y * scale).floor().clamp(0.0, pixel_size.1 as f64) as u32;
        let right = ((r.x + r.width) * scale).ceil().clamp(0.0, pixel_size.0 as f64) as u32;
        let bottom = ((r.y + r.height) * scale).ceil().clamp(0.0, pixel_size.1 as f64) as u32;
        if right <= left || bottom <= top {
            return Err(GeometryError::OutsideImage);
        }
        let pixel_rect = ImageRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        };
        let view_rect = RectF::new(
            left as f64 / scale,
            top as f64 / scale,
            pixel_rect.width as f64 / scale,
            pixel_rect.height as f64 / scale,
        );
        if ![
            view_rect.x,
            view_rect.y,
            view_rect.width,
            view_rect.height,
            view_rect.x + view_rect.width,
            view_rect.y + view_rect.height,
        ]
        .into_iter()
        .all(f64::is_finite)
        {
            return Err(GeometryError::InvalidTransform);
        }
        Ok(Self {
            source_size: pixel_size,
            scale_factor: scale,
            pixel_rect,
            view_rect: ViewRect(view_rect),
        })
    }

    pub fn pixel_rect(&self) -> ImageRect {
        self.pixel_rect
    }

    pub fn view_rect(&self) -> ViewRect {
        self.view_rect
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    pub fn validate_source(&self, pixel_size: (u32, u32)) -> Result<(), GeometryError> {
        if self.source_size == pixel_size {
            Ok(())
        } else {
            Err(GeometryError::SizeMismatch)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CaptureGeometry {
    pub display_id: DisplayId,
    pub screen_origin: ScreenPoint,
    pub scale_factor: f64,
    pub pixel_size: (u32, u32),
}

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum GeometryError {
    #[error("capture scale and coordinates must be finite, with a positive scale")]
    InvalidTransform,
    #[error("capture image and selection must have a positive area")]
    Empty,
    #[error("selection does not intersect the captured image")]
    OutsideImage,
    #[error("capture geometry does not match the image dimensions")]
    SizeMismatch,
}

impl CaptureGeometry {
    pub fn validate(&self) -> Result<(), GeometryError> {
        if !self.scale_factor.is_finite() || self.scale_factor <= 0.0 || !self.screen_origin.x.is_finite() || !self.screen_origin.y.is_finite() {
            return Err(GeometryError::InvalidTransform);
        }
        if self.pixel_size.0 == 0 || self.pixel_size.1 == 0 {
            return Err(GeometryError::Empty);
        }
        Ok(())
    }

    pub fn viewport_bounds(&self) -> RectF {
        RectF::new(
            0.0,
            0.0,
            self.pixel_size.0 as f64 / self.scale_factor,
            self.pixel_size.1 as f64 / self.scale_factor,
        )
    }

    pub fn screen_to_view(&self, point: ScreenPoint) -> (f64, f64) {
        (point.x - self.screen_origin.x, point.y - self.screen_origin.y)
    }

    pub fn view_to_screen(&self, point: (f64, f64)) -> ScreenPoint {
        ScreenPoint {
            x: self.screen_origin.x + point.0,
            y: self.screen_origin.y + point.1,
        }
    }

    pub fn image_rect(&self, selection: ViewRect) -> Result<ImageRect, GeometryError> {
        self.resolve_selection(selection).map(|selection| selection.pixel_rect())
    }

    pub fn resolve_selection(&self, selection: ViewRect) -> Result<ResolvedSelection, GeometryError> {
        self.validate()?;
        ResolvedSelection::resolve(selection, self.scale_factor, self.pixel_size)
    }
}

#[derive(Clone, Debug)]
pub struct CapturedFrame {
    image: ImageRef,
    geometry: CaptureGeometry,
}

impl CapturedFrame {
    pub fn new(image: ImageRef, geometry: CaptureGeometry) -> Result<Self, GeometryError> {
        geometry.validate()?;
        if image.dimensions() != geometry.pixel_size {
            return Err(GeometryError::SizeMismatch);
        }
        Ok(Self { image, geometry })
    }

    pub fn image(&self) -> &ImageRef {
        &self.image
    }

    pub fn geometry(&self) -> &CaptureGeometry {
        &self.geometry
    }

    /// Replaces pixels while preserving the validated acquisition transform.
    pub fn with_image(&self, image: ImageRef) -> Result<Self, GeometryError> {
        Self::new(image, self.geometry.clone())
    }

    pub fn crop(&self, selection: ViewRect) -> Result<ImageRef, GeometryError> {
        self.crop_resolved(self.geometry.resolve_selection(selection)?)
    }

    pub fn crop_resolved(&self, selection: ResolvedSelection) -> Result<ImageRef, GeometryError> {
        selection.validate_source(self.image.dimensions())?;
        if selection.scale_factor() != self.geometry.scale_factor {
            return Err(GeometryError::InvalidTransform);
        }
        let r = selection.pixel_rect();
        if r.x == 0 && r.y == 0 && (r.width, r.height) == self.image.dimensions() {
            return Ok(self.image.clone());
        }
        Ok(ImageAsset::new(
            image::imageops::crop_imm(self.image.pixels(), r.x, r.y, r.width, r.height).to_image(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> CapturedFrame {
        CapturedFrame::new(
            ImageAsset::new(RgbaImage::new(300, 200)),
            CaptureGeometry {
                display_id: DisplayId("secondary".into()),
                screen_origin: ScreenPoint { x: -1200.0, y: -100.0 },
                scale_factor: 1.5,
                pixel_size: (300, 200),
            },
        )
        .unwrap()
    }

    #[test]
    fn fractional_scale_uses_the_captured_transform() {
        let frame = frame();
        let rect = frame.geometry.image_rect(ViewRect(RectF::new(10.0, 20.0, 40.0, 30.0))).unwrap();
        assert_eq!(
            rect,
            ImageRect {
                x: 15,
                y: 30,
                width: 60,
                height: 45
            }
        );
        let point = ScreenPoint { x: -1190.0, y: -80.0 };
        assert_eq!(frame.geometry.view_to_screen(frame.geometry.screen_to_view(point)), point);
    }

    #[test]
    fn full_frame_crops_keep_the_same_asset() {
        let frame = frame();
        let result = frame.crop(ViewRect(frame.geometry.viewport_bounds())).unwrap();
        assert!(Arc::ptr_eq(&frame.image, &result));
    }

    #[test]
    fn invalid_coordinates_and_sizes_are_rejected() {
        let frame = frame();
        assert!(frame.crop(ViewRect(RectF::new(f64::NAN, 0.0, 1.0, 1.0))).is_err());
        assert!(frame.crop(ViewRect(RectF::new(500.0, 0.0, 1.0, 1.0))).is_err());
        let mut geometry = frame.geometry.clone();
        geometry.pixel_size = (1, 1);
        assert_eq!(CapturedFrame::new(frame.image, geometry).unwrap_err(), GeometryError::SizeMismatch);
    }

    #[test]
    fn resolved_selection_shares_rounded_and_clipped_crop_coordinates() {
        let frame = frame();
        for selection in [RectF::new(10.1, 20.25, 40.2, 30.1), RectF::new(-0.3, -1.0, 10.0, 20.0)] {
            let resolved = frame.geometry().resolve_selection(ViewRect(selection)).unwrap();
            let pixels = resolved.pixel_rect();
            let logical = resolved.view_rect().0;
            assert_eq!(frame.crop(ViewRect(selection)).unwrap().dimensions(), (pixels.width, pixels.height));
            assert_eq!(logical.x, pixels.x as f64 / frame.geometry().scale_factor);
            assert_eq!(logical.y, pixels.y as f64 / frame.geometry().scale_factor);
            assert_eq!(resolved.validate_source(frame.image().dimensions()), Ok(()));
            assert_eq!(resolved.validate_source((301, 200)), Err(GeometryError::SizeMismatch));
            assert_eq!(resolved.scale_factor(), frame.geometry().scale_factor);
            assert_eq!(
                frame.crop_resolved(resolved).unwrap().pixels(),
                frame.crop(ViewRect(selection)).unwrap().pixels()
            );
        }
        let rounded = ResolvedSelection::resolve(ViewRect(RectF::new(10.1, 20.25, 40.2, 30.1)), 1.5, (300, 200)).unwrap();
        assert_eq!(
            rounded.pixel_rect(),
            ImageRect {
                x: 15,
                y: 30,
                width: 61,
                height: 46
            }
        );
        assert_eq!(rounded.view_rect().0.x, 10.0);
        assert_eq!(rounded.view_rect().0.y, 20.0);
        assert_eq!(
            ResolvedSelection::resolve(ViewRect(RectF::new(0.0, 0.0, 1.0, 1.0)), f64::MIN_POSITIVE / 8.0, (1, 1)),
            Err(GeometryError::InvalidTransform)
        );
    }

    #[test]
    fn replacing_pixels_cannot_invalidate_the_acquisition_geometry() {
        let frame = frame();
        let replacement = ImageAsset::new(RgbaImage::new(300, 200));
        let updated = frame.with_image(replacement.clone()).unwrap();
        assert!(Arc::ptr_eq(updated.image(), &replacement));
        assert_eq!(updated.geometry(), frame.geometry());
        assert_eq!(
            frame.with_image(ImageAsset::new(RgbaImage::new(1, 1))).unwrap_err(),
            GeometryError::SizeMismatch
        );
        assert_eq!(frame.image().dimensions(), (300, 200));
    }
}

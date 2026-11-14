use super::{ErrorKind, OperationError, OperationFuture, OperationResult};
use image::RgbaImage;
use minnow_imaging::ImageRef;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

/// Runtime capability is independent of the state of downloaded model files.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecognitionAvailability {
    Available,
    Unavailable { reason: String },
}

impl RecognitionAvailability {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    pub fn require_available(&self) -> OperationResult<()> {
        match self {
            Self::Available => Ok(()),
            Self::Unavailable { reason } => Err(OperationError::new(ErrorKind::Unavailable, reason.clone())),
        }
    }
}

/// Geometry relative to the original image, with a rotation in degrees.
///
/// Centers are fractions in `0..=1`; lengths are positive fractions of the
/// image width/height. A rotated edge can exceed one image dimension, so its
/// normalized length may exceed one. All values must be finite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalizedOcrGeometry {
    cx: f64,
    cy: f64,
    width: f64,
    height: f64,
    angle_degrees: f64,
}

impl NormalizedOcrGeometry {
    pub fn new(cx: f64, cy: f64, width: f64, height: f64, angle_degrees: f64) -> OperationResult<Self> {
        if ![cx, cy, width, height, angle_degrees].iter().all(|value| value.is_finite())
            || !(0.0..=1.0).contains(&cx)
            || !(0.0..=1.0).contains(&cy)
            || width <= 0.0
            || height <= 0.0
        {
            return Err(OperationError::new(
                ErrorKind::InvalidInput,
                "OCR geometry must contain finite normalized coordinates and positive lengths",
            ));
        }
        Ok(Self {
            cx,
            cy,
            width,
            height,
            angle_degrees,
        })
    }

    pub fn cx(self) -> f64 {
        self.cx
    }
    pub fn cy(self) -> f64 {
        self.cy
    }
    pub fn width(self) -> f64 {
        self.width
    }
    pub fn height(self) -> f64 {
        self.height
    }
    pub fn angle_degrees(self) -> f64 {
        self.angle_degrees
    }
}

/// A text block always has validated geometry normalized to its source image.
#[derive(Clone, Debug, PartialEq)]
pub struct OcrBlock {
    pub text: String,
    geometry: NormalizedOcrGeometry,
}

impl OcrBlock {
    pub fn new(text: impl Into<String>, geometry: NormalizedOcrGeometry) -> Self {
        Self { text: text.into(), geometry }
    }

    pub fn geometry(&self) -> NormalizedOcrGeometry {
        self.geometry
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ModelDownloadStatus {
    #[default]
    Idle,
    Downloading {
        progress_percent: u8,
    },
    Failed {
        message: String,
    },
}

/// Active artifact and the latest update are independent facts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModelStatus {
    pub active_generation: Option<String>,
    pub download: ModelDownloadStatus,
}

impl ModelStatus {
    pub fn is_ready(&self) -> bool {
        self.active_generation.is_some()
    }

    pub fn is_downloading(&self) -> bool {
        matches!(self.download, ModelDownloadStatus::Downloading { .. })
    }
}

pub trait TextRecognition: Send + Sync {
    fn availability(&self) -> RecognitionAvailability;
    fn recognize(&self, image: ImageRef, cancel: CancellationToken) -> OperationFuture<'_, Vec<OcrBlock>>;
}

pub trait QrDecoder: Send + Sync {
    fn decode_qr(&self, image: &RgbaImage) -> OperationResult<Option<String>>;
}

pub trait RecognitionModels: Send + Sync {
    fn model_status(&self) -> ModelStatus;
    fn subscribe_model_status(&self) -> watch::Receiver<ModelStatus>;
    fn ensure_models(&self, force: bool, cancel: CancellationToken) -> OperationFuture<'_, ()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_geometry_rejects_invalid_values_and_preserves_rotation() {
        for values in [
            [f64::NAN, 0.5, 0.2, 0.1, 0.0],
            [0.5, f64::INFINITY, 0.2, 0.1, 0.0],
            [-0.1, 0.5, 0.2, 0.1, 0.0],
            [0.5, 1.1, 0.2, 0.1, 0.0],
            [0.5, 0.5, 0.0, 0.1, 0.0],
            [0.5, 0.5, 0.2, -0.1, 0.0],
            [0.5, 0.5, 0.2, 0.1, f64::INFINITY],
        ] {
            assert_eq!(
                NormalizedOcrGeometry::new(values[0], values[1], values[2], values[3], values[4])
                    .unwrap_err()
                    .kind,
                ErrorKind::InvalidInput
            );
        }
        let geometry = NormalizedOcrGeometry::new(0.0, 1.0, 1.2, 0.1, -135.0).unwrap();
        let block = OcrBlock::new("rotated", geometry);
        assert_eq!(block.geometry(), geometry);
        assert_eq!(block.geometry().angle_degrees(), -135.0);
    }
}

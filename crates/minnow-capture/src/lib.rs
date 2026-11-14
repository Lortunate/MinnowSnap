//! Capture contracts and platform-independent capture processing.

mod run;
mod scroll;
mod stitcher;

use minnow_imaging::{CaptureGeometry, CapturedFrame, DisplayId, GeometryError, RectF};

pub use run::{CaptureControl, CaptureRequest, CaptureRun, CaptureRunConfig, PreviewSlot, PreviewSnapshot};
pub use scroll::{CaptureOutcome, CapturePhase, ScrollCapture, ScrollCaptureConfig};
pub use stitcher::{ScrollStitcher, StitchConfig, StitchFrameResult, StitchFrameStatus};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaptureTarget {
    Primary,
    Display(DisplayId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DisplayInfo {
    pub geometry: CaptureGeometry,
    pub name: String,
    pub is_primary: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowInfo {
    pub title: String,
    pub app_name: String,
    /// Logical coordinates local to the selected display.
    pub bounds: RectF,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CaptureError {
    #[error("capture backend failed: {0}")]
    Backend(String),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    #[error("capture was cancelled")]
    Cancelled,
    #[error("capture has no usable frames")]
    Empty,
    #[error("capture target geometry changed during the session")]
    TargetChanged,
    #[error("capture has already reached a terminal state")]
    Closed,
    #[error("invalid stitch configuration field: {field}")]
    InvalidStitchConfig { field: &'static str },
    #[error("capture exceeds the output budget of {max_pixels} pixels")]
    CapacityExceeded { max_pixels: u64 },
    #[error("source frame exceeds the admission limit of {max_pixels} pixels")]
    SourceTooLarge { max_pixels: u64 },
    #[error("capture working set needs {required_bytes} bytes, above its {max_bytes}-byte budget")]
    WorkingSetExceeded { max_bytes: u64, required_bytes: u64 },
    #[error("unable to allocate capture working buffers")]
    AllocationFailed,
}

/// Native capture implementations provide frames with their original geometry.
///
/// The backend may block while capturing. Cancellation is cooperative and does
/// not assert that an in-flight backend call has already stopped.
pub trait CaptureBackend: Send + Sync {
    fn capture(&self, target: &CaptureTarget) -> Result<CapturedFrame, CaptureError>;
    fn displays(&self) -> Result<Vec<DisplayInfo>, CaptureError>;
    fn windows(&self, display: &DisplayId) -> Result<Vec<WindowInfo>, CaptureError>;
}

use crate::{CaptureBackend, CaptureError, CaptureTarget};
use minnow_imaging::CapturedFrame;

/// Owned by a single capture run, on its worker thread. Dropping the source
/// releases native resources. Cancellation is checked between acquisitions.
pub trait CaptureSource {
    fn capture(&mut self) -> Result<CapturedFrame, CaptureError>;
}

pub(crate) struct BackendSource<'a, B: CaptureBackend + ?Sized> {
    backend: &'a B,
    target: CaptureTarget,
}

impl<'a, B: CaptureBackend + ?Sized> BackendSource<'a, B> {
    pub(crate) fn new(backend: &'a B, target: CaptureTarget) -> Self {
        Self { backend, target }
    }
}

impl<B: CaptureBackend + ?Sized> CaptureSource for BackendSource<'_, B> {
    fn capture(&mut self) -> Result<CapturedFrame, CaptureError> {
        let frame = self.backend.capture(&self.target)?;
        if let CaptureTarget::Display(id) = &self.target
            && frame.geometry().display_id != *id
        {
            return Err(CaptureError::TargetChanged);
        }
        self.target = CaptureTarget::Display(frame.geometry().display_id.clone());
        Ok(frame)
    }
}

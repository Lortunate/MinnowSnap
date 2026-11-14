use super::{ApplicationServices, CaptureSurface, blocking, capture_error};
use crate::{CompletionPolicy, JobAdmission, JobHandle, JobResource, JobScope, OperationError};
use minnow_capture::CaptureTarget;
use minnow_imaging::CapturedFrame;

impl ApplicationServices {
    pub fn capture_preview(&self, target: CaptureTarget) -> JobHandle<CapturedFrame> {
        let backend = self.capture.clone();
        let runtime = self.runtime.clone();
        self.jobs.spawn(
            "capture frame",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            move |_| async move { blocking(&runtime, move || backend.capture(&target).map_err(capture_error)).await },
        )
    }

    pub fn quick_capture(&self) -> JobHandle<()> {
        let backend = self.capture.clone();
        let output = self.output.clone();
        let runtime = self.runtime.clone();
        let jobs = self.jobs.clone();
        self.jobs.spawn(
            "quick capture",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::QuickCapture,
            move |cancel| async move {
                let capture_permit = jobs.acquire(JobResource::InteractiveCapture, &cancel).await?;
                let frame = blocking(&runtime, move || backend.capture(&CaptureTarget::Primary).map_err(capture_error)).await?;
                drop(capture_permit);
                if cancel.is_cancelled() {
                    return Err(OperationError::cancelled());
                }
                let _output_permit = jobs.acquire(JobResource::ImageOutput, &cancel).await?;
                blocking(&runtime, move || output.copy_image(frame.image().pixels())).await
            },
        )
    }

    pub fn capture_surface(&self, target: CaptureTarget) -> JobHandle<CaptureSurface> {
        let backend = self.capture.clone();
        let runtime = self.runtime.clone();
        self.jobs.spawn(
            "capture desktop",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            move |_| async move {
                blocking(&runtime, move || {
                    let frame = backend.capture(&target).map_err(capture_error)?;
                    let (windows, warnings) = match backend.windows(&frame.geometry().display_id) {
                        Ok(windows) => (windows, Vec::new()),
                        Err(error) => (Vec::new(), vec![capture_error(error)]),
                    };
                    Ok(CaptureSurface { frame, windows, warnings })
                })
                .await
            },
        )
    }
}

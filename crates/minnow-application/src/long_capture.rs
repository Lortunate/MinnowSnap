use crate::services::capture_error;
use crate::{
    ApplicationServices, CaptureTarget, CompletionPolicy, ErrorKind, ImageRef, JobAdmission, JobId, JobPhase, JobScope, JobSupervisor,
    OperationError, OperationResult, ViewRect,
};
use futures_util::FutureExt;
use futures_util::future::{BoxFuture, Shared};
use minnow_capture::{CaptureControl, CaptureOutcome, CaptureRequest, CaptureRun, CaptureRunConfig, PreviewSlot, PreviewSnapshot};
use std::sync::{Arc, Mutex};
use tokio::sync::{oneshot, watch};

#[derive(Clone, Debug)]
pub enum LongCaptureStatus {
    Queued,
    Running,
    Finishing,
    CancellationRequested,
    Ready(ImageRef),
    Cancelled { partial: Option<ImageRef> },
    Failed { error: LongCaptureFailure, partial: Option<ImageRef> },
}

/// Preserve capture evidence across the application boundary; task failures have
/// a separate origin and must not be parsed back out of display strings.
#[derive(Clone, Debug, thiserror::Error)]
pub enum LongCaptureFailure {
    #[error("{0}")]
    Capture(minnow_capture::CaptureError),
    #[error("{0}")]
    Task(OperationError),
}

impl From<LongCaptureFailure> for OperationError {
    fn from(error: LongCaptureFailure) -> Self {
        match error {
            LongCaptureFailure::Capture(error) => capture_error(error),
            LongCaptureFailure::Task(error) => error,
        }
    }
}

/// Each UI projection owns a clone; finishing and retrying never consume pixels.
#[derive(Clone)]
pub struct LongCaptureHandle {
    job_id: JobId,
    control: CaptureControl,
    previews: PreviewSlot,
    phase: watch::Receiver<JobPhase>,
    outcome: Arc<Mutex<Option<CaptureOutcome>>>,
    completion: Shared<BoxFuture<'static, OperationResult<ImageRef>>>,
    jobs: JobSupervisor,
    start_gate: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl LongCaptureHandle {
    pub fn job_id(&self) -> JobId {
        self.job_id
    }

    pub fn preview(&self) -> Option<PreviewSnapshot> {
        self.previews.latest()
    }

    pub fn status(&self) -> LongCaptureStatus {
        if let Some(outcome) = self.outcome() {
            return match outcome {
                CaptureOutcome::Ready(image) => LongCaptureStatus::Ready(image),
                CaptureOutcome::Cancelled { partial } => LongCaptureStatus::Cancelled { partial },
                CaptureOutcome::Failed { error, partial } => LongCaptureStatus::Failed {
                    error: LongCaptureFailure::Capture(error),
                    partial,
                },
            };
        }
        let phase = self.phase.borrow().clone();
        match &phase {
            JobPhase::Cancelled => LongCaptureStatus::Cancelled { partial: None },
            JobPhase::Failed(error) => LongCaptureStatus::Failed {
                error: LongCaptureFailure::Task(error.clone()),
                partial: None,
            },
            JobPhase::CancellationRequested => LongCaptureStatus::CancellationRequested,
            phase => match self.control.request() {
                CaptureRequest::Finish => LongCaptureStatus::Finishing,
                CaptureRequest::Cancel => LongCaptureStatus::CancellationRequested,
                _ if matches!(phase, JobPhase::Queued) || crate::sessions::lock(&self.start_gate).is_some() => LongCaptureStatus::Queued,
                _ => LongCaptureStatus::Running,
            },
        }
    }

    /// Completion notifications are independent of the lossy preview slot.
    pub fn subscribe(&self) -> watch::Receiver<JobPhase> {
        self.phase.clone()
    }

    /// Begin acquisition once all target windows exist and the selection overlay is closed.
    /// Every clone shares this one-shot start decision.
    pub fn start(&self) -> bool {
        let terminal = self.phase.borrow().is_terminal();
        let mut start_gate = crate::sessions::lock(&self.start_gate);
        if terminal || matches!(self.control.request(), CaptureRequest::Cancel | CaptureRequest::Finished) {
            return false;
        }
        start_gate.take().is_some_and(|gate| gate.send(()).is_ok())
    }

    /// A prepared run has no pixels to finish. Closing its gate settles it as
    /// cancelled rather than leaving a finish waiter suspended until `start`.
    pub fn request_finish(&self) -> bool {
        if crate::sessions::lock(&self.start_gate).take().is_some() {
            self.control.cancel();
            self.jobs.cancel(self.job_id);
            return false;
        }
        self.control.finish()
    }

    /// A request only. Native capture may still be running until the job settles.
    pub fn cancel(&self) -> bool {
        let requested = self.control.cancel();
        if requested {
            crate::sessions::lock(&self.start_gate).take();
            self.jobs.cancel(self.job_id);
        }
        requested
    }

    pub async fn finish(&self) -> OperationResult<ImageRef> {
        self.request_finish();
        self.completion.clone().await
    }

    pub async fn wait(&self) -> OperationResult<ImageRef> {
        self.completion.clone().await
    }

    pub fn result(&self) -> Option<ImageRef> {
        match self.outcome()? {
            CaptureOutcome::Ready(image) => Some(image),
            _ => None,
        }
    }

    /// Cancellation and failure can retain useful data without declaring success.
    pub fn outcome(&self) -> Option<CaptureOutcome> {
        crate::sessions::lock(&self.outcome).clone()
    }
}

impl ApplicationServices {
    pub fn start_long_capture(&self, target: CaptureTarget, selection: ViewRect) -> OperationResult<LongCaptureHandle> {
        self.start_long_capture_with_config(target, selection, CaptureRunConfig::default())
    }

    pub fn start_long_capture_with_config(
        &self,
        target: CaptureTarget,
        selection: ViewRect,
        config: CaptureRunConfig,
    ) -> OperationResult<LongCaptureHandle> {
        let handle = self.prepare_long_capture_with_config(target, selection, config)?;
        handle.start();
        Ok(handle)
    }

    /// Prepare a supervised run without acquiring any desktop pixels.
    pub fn prepare_long_capture(&self, target: CaptureTarget, selection: ViewRect) -> OperationResult<LongCaptureHandle> {
        self.prepare_long_capture_with_config(target, selection, CaptureRunConfig::default())
    }

    fn prepare_long_capture_with_config(
        &self,
        target: CaptureTarget,
        selection: ViewRect,
        config: CaptureRunConfig,
    ) -> OperationResult<LongCaptureHandle> {
        let run = CaptureRun::with_config(target, selection, config).map_err(capture_error)?;
        let control = run.control();
        let previews = run.previews();
        let outcome = Arc::new(Mutex::new(None));
        let worker_outcome = outcome.clone();
        let worker_control = control.clone();
        let backend = self.capture_backend().clone();
        let runtime = self.runtime().clone();
        let (start_tx, start_rx) = oneshot::channel();
        let job = self.jobs().spawn_after(
            "scroll capture",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::LongCapture,
            async move { start_rx.await.map_err(|_| OperationError::cancelled()) },
            move |cancel| async move {
                let mut worker = runtime.spawn_blocking(move || run.run(backend.as_ref()));
                let result = tokio::select! {
                    result = &mut worker => result,
                    _ = cancel.cancelled() => {
                        worker_control.cancel();
                        // An in-flight native call remains owned until it returns.
                        worker.await
                    }
                };
                let result = result.map_err(|error| OperationError::new(ErrorKind::Task, format!("scroll capture worker failed: {error}")))?;
                *crate::sessions::lock(&worker_outcome) = Some(result.clone());
                match result {
                    CaptureOutcome::Ready(image) => Ok(image),
                    CaptureOutcome::Cancelled { .. } => Err(OperationError::cancelled()),
                    CaptureOutcome::Failed { error, .. } => Err(capture_error(error)),
                }
            },
        );
        if let Some(error) = job.admission_error() {
            return Err(error.clone());
        }
        Ok(LongCaptureHandle {
            job_id: job.id(),
            phase: job.subscribe(),
            control,
            previews,
            outcome,
            completion: job.boxed().shared(),
            jobs: self.jobs().clone(),
            start_gate: Arc::new(Mutex::new(Some(start_tx))),
        })
    }
}

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use minnow_imaging::{ImageRef, ViewRect};

use crate::scroll::thumbnail;
use crate::{CaptureBackend, CaptureError, CaptureOutcome, CaptureTarget, ScrollCapture, ScrollCaptureConfig, StitchFrameStatus};

#[derive(Clone, Copy, Debug)]
pub struct CaptureRunConfig {
    pub scroll: ScrollCaptureConfig,
    pub frame_interval: Duration,
    pub preview_interval: Duration,
    pub preview_width: u32,
}

impl Default for CaptureRunConfig {
    fn default() -> Self {
        Self {
            scroll: ScrollCaptureConfig::default(),
            frame_interval: Duration::from_millis(16),
            preview_interval: Duration::from_millis(33),
            preview_width: 500,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CaptureRequest {
    Running = 0,
    Finish = 1,
    Cancel = 2,
    Finished = 3,
}

struct ControlState {
    request: AtomicU8,
    wake_lock: Mutex<()>,
    wake: Condvar,
}

/// Cooperative requests belonging to exactly one capture run.
#[derive(Clone)]
pub struct CaptureControl(Arc<ControlState>);

impl Default for CaptureControl {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureControl {
    pub fn new() -> Self {
        Self(Arc::new(ControlState {
            request: AtomicU8::new(CaptureRequest::Running as u8),
            wake_lock: Mutex::new(()),
            wake: Condvar::new(),
        }))
    }

    pub fn request(&self) -> CaptureRequest {
        match self.0.request.load(Ordering::Acquire) {
            0 => CaptureRequest::Running,
            1 => CaptureRequest::Finish,
            2 => CaptureRequest::Cancel,
            _ => CaptureRequest::Finished,
        }
    }

    /// Requests a useful final result. It does not wait for an in-flight frame.
    pub fn finish(&self) -> bool {
        let _lock = lock_recover(&self.0.wake_lock);
        let accepted = self
            .0
            .request
            .compare_exchange(
                CaptureRequest::Running as u8,
                CaptureRequest::Finish as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok();
        self.0.wake.notify_all();
        accepted
    }

    /// Cancels an unfinished run, including a pending finish request.
    pub fn cancel(&self) -> bool {
        let _lock = lock_recover(&self.0.wake_lock);
        let accepted = match self.request() {
            CaptureRequest::Running | CaptureRequest::Finish => {
                self.0.request.store(CaptureRequest::Cancel as u8, Ordering::Release);
                true
            }
            CaptureRequest::Cancel | CaptureRequest::Finished => false,
        };
        self.0.wake.notify_all();
        accepted
    }

    pub fn is_finished(&self) -> bool {
        self.request() == CaptureRequest::Finished
    }

    fn complete(&self) {
        let _lock = lock_recover(&self.0.wake_lock);
        self.0.request.store(CaptureRequest::Finished as u8, Ordering::Release);
        self.0.wake.notify_all();
    }

    fn settle(&self, outcome: CaptureOutcome, publish: impl FnOnce(&CaptureOutcome)) -> CaptureOutcome {
        let _lock = lock_recover(&self.0.wake_lock);
        let outcome = match (self.request(), outcome) {
            (CaptureRequest::Cancel, CaptureOutcome::Ready(image)) => CaptureOutcome::Cancelled { partial: Some(image) },
            (_, outcome) => outcome,
        };
        publish(&outcome);
        self.0.request.store(CaptureRequest::Finished as u8, Ordering::Release);
        self.0.wake.notify_all();
        outcome
    }

    fn wait(&self, duration: Duration) {
        if duration.is_zero() {
            return;
        }
        let guard = lock_recover(&self.0.wake_lock);
        let result = self
            .0
            .wake
            .wait_timeout_while(guard, duration, |_| self.request() == CaptureRequest::Running);
        if let Err(poisoned) = result {
            drop(poisoned.into_inner());
            self.0.wake_lock.clear_poison();
        }
    }
}

#[derive(Clone, Debug)]
pub struct PreviewSnapshot {
    pub revision: u64,
    pub height: u32,
    pub image: Option<ImageRef>,
    pub warning: Option<CaptureError>,
}

/// A single replaceable preview, never a queue of obsolete image buffers.
///
/// Completion is carried by [`CaptureOutcome`], independently of this lossy
/// display projection.
#[derive(Clone, Default)]
pub struct PreviewSlot(Arc<Mutex<Option<PreviewSnapshot>>>);

impl PreviewSlot {
    pub fn latest(&self) -> Option<PreviewSnapshot> {
        lock_recover(&self.0).clone()
    }

    fn publish(&self, height: u32, image: Option<ImageRef>, warning: Option<CaptureError>) {
        let mut slot = lock_recover(&self.0);
        let revision = slot.as_ref().map_or(1, |previous| previous.revision.saturating_add(1));
        // The height describes these exact pixels, even on warning-only updates.
        let height = if image.is_some() {
            height
        } else {
            slot.as_ref().map_or(0, |previous| previous.height)
        };
        let image = image.or_else(|| slot.as_ref().and_then(|previous| previous.image.clone()));
        *slot = Some(PreviewSnapshot {
            revision,
            height,
            image,
            warning,
        });
    }
}

/// A one-shot blocking capture job.
///
/// The caller chooses the executor and retains its task or thread handle. This
/// type never starts a runtime or detaches a worker. A native backend call may
/// remain in flight after cancellation is requested; its task return value is
/// the confirmation that capture processing has ended.
pub struct CaptureRun {
    target: CaptureTarget,
    session: ScrollCapture,
    config: CaptureRunConfig,
    control: CaptureControl,
    previews: PreviewSlot,
}

impl CaptureRun {
    pub fn new(target: CaptureTarget, selection: ViewRect) -> Result<Self, CaptureError> {
        Self::with_config(target, selection, CaptureRunConfig::default())
    }

    pub fn with_config(target: CaptureTarget, selection: ViewRect, config: CaptureRunConfig) -> Result<Self, CaptureError> {
        if config.preview_width == 0 {
            return Err(CaptureError::Backend("preview width must be positive".into()));
        }
        Ok(Self {
            target,
            session: ScrollCapture::with_config(selection, config.scroll)?,
            config,
            control: CaptureControl::new(),
            previews: PreviewSlot::default(),
        })
    }

    pub fn control(&self) -> CaptureControl {
        self.control.clone()
    }

    pub fn previews(&self) -> PreviewSlot {
        self.previews.clone()
    }

    pub fn run(mut self, backend: &dyn CaptureBackend) -> CaptureOutcome {
        let outcome = self.run_inner(backend);
        let image = outcome.image();
        let height = image.as_ref().map_or(0, |image| image.height());
        let preview = image.map(|image| thumbnail(&image, self.config.preview_width));
        // Expensive final thumbnail work precedes the short completion lock.
        // A cancellation accepted during finalization remains cancellation.
        self.control.settle(outcome, |outcome| {
            let warning = match outcome {
                CaptureOutcome::Failed { error, .. } => Some(error.clone()),
                _ => None,
            };
            self.previews.publish(height, preview, warning);
        })
    }

    fn run_inner(&mut self, backend: &dyn CaptureBackend) -> CaptureOutcome {
        let mut source = None;
        let mut preview_dirty = false;
        let mut warning = None;
        let mut last_preview = None::<Instant>;

        loop {
            match self.control.request() {
                CaptureRequest::Finish => {
                    let _ = self.session.finish();
                    return self.session.outcome().expect("finish produces a terminal outcome");
                }
                CaptureRequest::Cancel | CaptureRequest::Finished => return self.session.cancel(),
                CaptureRequest::Running => {}
            }

            if source.is_none() {
                source = match backend.open_scroll_source(&self.target) {
                    Ok(source) => Some(source),
                    Err(error) => {
                        return if self.control.request() == CaptureRequest::Cancel {
                            self.session.cancel()
                        } else {
                            self.session.fail(error)
                        };
                    }
                };
                // Initialization can itself wait on native callbacks.
                continue;
            }
            let frame = match source.as_mut().expect("source initialized").capture() {
                Ok(frame) => frame,
                Err(error) => {
                    return if self.control.request() == CaptureRequest::Cancel {
                        self.session.cancel()
                    } else {
                        self.session.fail(error)
                    };
                }
            };

            if self.control.request() == CaptureRequest::Cancel {
                return self.session.cancel();
            }
            if let CaptureTarget::Display(display) = &self.target
                && frame.geometry().display_id != *display
            {
                return self.session.fail(CaptureError::TargetChanged);
            }

            let update = match self.session.push_frame(&frame) {
                Ok(update) => update,
                Err(_) => return self.session.outcome().expect("frame failure produces a terminal outcome"),
            };

            let previous_warning = warning.clone();
            warning = update.warning.map(|reason| CaptureError::Incomplete { reason });
            preview_dirty |= update.status == StitchFrameStatus::Appended;

            let publish_frame = preview_dirty && last_preview.is_none_or(|last| last.elapsed() >= self.config.preview_interval);
            if publish_frame || previous_warning != warning {
                let image = publish_frame.then(|| self.session.preview(self.config.preview_width)).flatten();
                self.previews.publish(self.session.height(), image, warning.clone());
                if publish_frame {
                    preview_dirty = false;
                    last_preview = Some(Instant::now());
                }
            }

            self.control.wait(self.config.frame_interval);
        }
    }
}

impl Drop for CaptureRun {
    fn drop(&mut self) {
        // Dropping an unstarted job has no worker to join. During unwinding the
        // executor still observes the panic through its retained task handle.
        if !self.control.is_finished() {
            self.control.cancel();
            self.control.complete();
        }
    }
}

fn lock_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            let guard = poisoned.into_inner();
            mutex.clear_poison();
            guard
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CapturePhase, DisplayInfo, WindowInfo};
    use image::{Rgba, RgbaImage};
    use minnow_imaging::{CaptureGeometry, CapturedFrame, DisplayId, ImageAsset, RectF, ScreenPoint};
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    type FrameStep = Result<CapturedFrame, CaptureError>;

    struct Backend {
        steps: Mutex<VecDeque<FrameStep>>,
        targets: Mutex<Vec<CaptureTarget>>,
        calls: AtomicUsize,
        on_capture: Box<dyn Fn(usize) + Send + Sync>,
    }

    impl Backend {
        fn new(steps: impl IntoIterator<Item = FrameStep>, on_capture: impl Fn(usize) + Send + Sync + 'static) -> Self {
            Self {
                steps: Mutex::new(steps.into_iter().collect()),
                targets: Mutex::new(Vec::new()),
                calls: AtomicUsize::new(0),
                on_capture: Box::new(on_capture),
            }
        }
    }

    impl CaptureBackend for Backend {
        fn capture(&self, target: &CaptureTarget) -> Result<CapturedFrame, CaptureError> {
            self.targets.lock().unwrap().push(target.clone());
            let call = self.calls.fetch_add(1, AtomicOrdering::Relaxed) + 1;
            (self.on_capture)(call);
            self.steps.lock().unwrap().pop_front().expect("unexpected extra capture")
        }

        fn displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
            Ok(Vec::new())
        }

        fn windows(&self, _: &DisplayId) -> Result<Vec<WindowInfo>, CaptureError> {
            Ok(Vec::new())
        }
    }

    fn frame() -> CapturedFrame {
        CapturedFrame::new(
            ImageAsset::new(RgbaImage::from_pixel(80, 60, Rgba([20, 40, 80, 255]))),
            CaptureGeometry {
                display_id: DisplayId("display-a".into()),
                screen_origin: ScreenPoint { x: -400.0, y: 0.0 },
                scale_factor: 2.0,
                pixel_size: (80, 60),
            },
        )
        .unwrap()
    }

    fn run() -> CaptureRun {
        CaptureRun::with_config(
            CaptureTarget::Primary,
            ViewRect(RectF::new(0.0, 0.0, 40.0, 30.0)),
            CaptureRunConfig {
                frame_interval: Duration::ZERO,
                preview_interval: Duration::ZERO,
                ..CaptureRunConfig::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn finish_confirms_a_result_and_pins_the_initial_display() {
        let job = run();
        let control = job.control();
        let requested = control.clone();
        let backend = Backend::new([Ok(frame()), Ok(frame())], move |call| {
            if call == 2 {
                requested.finish();
            }
        });
        let outcome = job.run(&backend);
        let image = match outcome {
            CaptureOutcome::Ready(image) => image,
            other => panic!("unexpected outcome: {other:?}"),
        };
        assert_eq!(image.dimensions(), (80, 60));
        assert!(control.is_finished());
        assert!(!control.cancel());
        assert_eq!(
            *backend.targets.lock().unwrap(),
            [CaptureTarget::Primary, CaptureTarget::Display(DisplayId("display-a".into()))]
        );
    }

    #[test]
    fn backend_failure_retains_the_collected_prefix() {
        let job = run();
        let previews = job.previews();
        let error = CaptureError::Backend("screen source disappeared".into());
        let backend = Backend::new([Ok(frame()), Err(error.clone())], |_| {});
        let outcome = job.run(&backend);
        assert_eq!(outcome.phase(), CapturePhase::Failed);
        assert_eq!(outcome.error(), Some(&error));
        assert_eq!(outcome.image().unwrap().dimensions(), (80, 60));
        assert!(
            previews
                .latest()
                .unwrap()
                .warning
                .unwrap()
                .to_string()
                .contains("screen source disappeared")
        );
    }

    #[test]
    fn cancellation_during_an_inflight_frame_is_not_success() {
        let job = run();
        let control = job.control();
        let requested = control.clone();
        let backend = Backend::new([Ok(frame())], move |_| {
            requested.cancel();
        });
        let outcome = job.run(&backend);
        assert!(matches!(outcome, CaptureOutcome::Cancelled { partial: None }));
        assert!(control.is_finished());
    }

    struct SourceBackend {
        control: CaptureControl,
        cancel_on_open: bool,
        fail_on_open: bool,
        opened: AtomicUsize,
        captured: AtomicUsize,
        dropped: AtomicUsize,
    }

    impl CaptureBackend for SourceBackend {
        fn open_scroll_source(&self, _: &CaptureTarget) -> Result<Box<dyn crate::CaptureSource + '_>, CaptureError> {
            self.opened.fetch_add(1, AtomicOrdering::Relaxed);
            if self.cancel_on_open {
                self.control.cancel();
            }
            if self.fail_on_open {
                return Err(CaptureError::Backend("opening failed".into()));
            }
            Ok(Box::new(TestSource(self)))
        }

        fn capture(&self, _: &CaptureTarget) -> Result<CapturedFrame, CaptureError> {
            panic!("run must use its owned source")
        }

        fn displays(&self) -> Result<Vec<DisplayInfo>, CaptureError> {
            unreachable!()
        }
        fn windows(&self, _: &DisplayId) -> Result<Vec<WindowInfo>, CaptureError> {
            unreachable!()
        }
    }

    struct TestSource<'a>(&'a SourceBackend);

    impl crate::CaptureSource for TestSource<'_> {
        fn capture(&mut self) -> Result<CapturedFrame, CaptureError> {
            if self.0.captured.fetch_add(1, AtomicOrdering::Relaxed) == 1 {
                self.0.control.finish();
            }
            Ok(frame())
        }
    }

    impl Drop for TestSource<'_> {
        fn drop(&mut self) {
            self.0.dropped.fetch_add(1, AtomicOrdering::Relaxed);
        }
    }

    fn source_backend(control: CaptureControl, cancel_on_open: bool, fail_on_open: bool) -> SourceBackend {
        SourceBackend {
            control,
            cancel_on_open,
            fail_on_open,
            opened: AtomicUsize::new(0),
            captured: AtomicUsize::new(0),
            dropped: AtomicUsize::new(0),
        }
    }

    #[test]
    fn cancellation_during_source_open_wins_even_if_opening_fails() {
        for fail in [false, true] {
            let job = run();
            let backend = source_backend(job.control(), true, fail);
            assert!(matches!(job.run(&backend), CaptureOutcome::Cancelled { partial: None }));
            assert_eq!(backend.opened.load(AtomicOrdering::Relaxed), 1);
            assert_eq!(backend.captured.load(AtomicOrdering::Relaxed), 0);
            assert_eq!(backend.dropped.load(AtomicOrdering::Relaxed), usize::from(!fail));
        }
    }

    #[test]
    fn one_source_is_reused_and_released_before_completion_returns() {
        let job = run();
        let backend = source_backend(job.control(), false, false);
        assert!(matches!(job.run(&backend), CaptureOutcome::Ready(_)));
        assert_eq!(backend.opened.load(AtomicOrdering::Relaxed), 1);
        assert_eq!(backend.captured.load(AtomicOrdering::Relaxed), 2);
        assert_eq!(backend.dropped.load(AtomicOrdering::Relaxed), 1);
    }

    #[test]
    fn old_control_cannot_stop_a_new_capture() {
        let old = run();
        let old_control = old.control();
        assert!(old_control.cancel());
        let old_backend = Backend::new([], |_| panic!("cancelled run must not capture"));
        assert!(matches!(old.run(&old_backend), CaptureOutcome::Cancelled { .. }));

        let next = run();
        let next_control = next.control();
        let requested = next_control.clone();
        let backend = Backend::new([Ok(frame())], move |_| {
            requested.finish();
        });
        assert!(!old_control.finish());
        assert!(matches!(next.run(&backend), CaptureOutcome::Ready(_)));
    }

    #[test]
    fn previews_replace_old_buffers_and_never_carry_completion() {
        let previews = PreviewSlot::default();
        let first = ImageAsset::new(RgbaImage::new(10, 10));
        let old_buffer = Arc::downgrade(&first);
        previews.publish(10, Some(first), None);
        for height in 11..30 {
            previews.publish(height, Some(ImageAsset::new(RgbaImage::new(10, height))), None);
        }
        let latest = previews.latest().unwrap();
        assert_eq!(latest.revision, 20);
        assert_eq!(latest.height, 29);
        assert_eq!(latest.image.unwrap().dimensions(), (10, 29));
        assert!(old_buffer.upgrade().is_none());
    }
}

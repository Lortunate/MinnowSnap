use super::layout::frame_visibility_after_click_through;
use crate::flows::output::{OutputDelivery, OutputRequest, OutputState};
use gpui::{AnyWindowHandle, AppContext, AsyncWindowContext, Context, RenderImage, WeakEntity, Window, WindowId};
use minnow_application::{LongCaptureHandle, LongCaptureStatus, OperationError, OperationResult};
use minnow_imaging::ImageRef;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LongCaptureWindowKind {
    Frame,
    Toolbar,
    Preview,
}

#[derive(Clone)]
pub(crate) struct LongCaptureSnapshot {
    pub preview_image: Option<Arc<RenderImage>>,
    pub preview_height_px: i32,
    pub preview_reduced: bool,
    pub warning_text: String,
    pub export_error: Option<OperationError>,
    pub preflight_error: Option<OperationError>,
    pub output_can_retry: bool,
    pub window_warning: Option<String>,
    pub capture_error: Option<String>,
    pub result: LongCaptureResult,
    pub partial_accepted: bool,
    pub busy: bool,
    pub output_submitted: bool,
    pub frame_visible: bool,
}
impl Default for LongCaptureSnapshot {
    fn default() -> Self {
        Self {
            preview_image: None,
            preview_height_px: 0,
            preview_reduced: false,
            warning_text: String::new(),
            export_error: None,
            preflight_error: None,
            output_can_retry: false,
            window_warning: None,
            capture_error: None,
            result: LongCaptureResult::Capturing,
            partial_accepted: false,
            busy: false,
            output_submitted: false,
            frame_visible: true,
        }
    }
}

/// A projection of the application's outcome, independent of an export attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LongCaptureResult {
    Capturing,
    Complete,
    Partial,
    Failed,
    Cancelled,
}

impl LongCaptureResult {
    fn from_status(status: &LongCaptureStatus) -> Self {
        match status {
            LongCaptureStatus::Ready(_) => Self::Complete,
            LongCaptureStatus::Failed { partial: Some(_), .. } | LongCaptureStatus::Cancelled { partial: Some(_) } => Self::Partial,
            LongCaptureStatus::Failed { .. } => Self::Failed,
            LongCaptureStatus::Cancelled { .. } => Self::Cancelled,
            _ => Self::Capturing,
        }
    }

    pub(crate) fn permits_output(self, partial_accepted: bool) -> bool {
        match self {
            Self::Capturing | Self::Complete => true,
            Self::Partial => partial_accepted,
            Self::Failed | Self::Cancelled => false,
        }
    }
}

/// Cloning an accepted result keeps the original available for another export.
fn terminal_image(status: LongCaptureStatus, partial_accepted: bool) -> Option<OperationResult<ImageRef>> {
    match status {
        LongCaptureStatus::Ready(image) => Some(Ok(image)),
        LongCaptureStatus::Failed { partial: Some(image), .. } | LongCaptureStatus::Cancelled { partial: Some(image) } if partial_accepted => {
            Some(Ok(image))
        }
        LongCaptureStatus::Failed { error, .. } => Some(Err(error)),
        LongCaptureStatus::Cancelled { .. } => Some(Err(minnow_application::OperationError::cancelled())),
        _ => None,
    }
}
#[derive(Clone, Default)]
struct WindowHandles {
    frame: Option<AnyWindowHandle>,
    toolbar: Option<AnyWindowHandle>,
    preview: Option<AnyWindowHandle>,
}
#[derive(Default)]
struct PresentationState {
    snapshot: LongCaptureSnapshot,
    handles: WindowHandles,
    preview_revision: u64,
    poller_running: bool,
    acquiring_output: bool,
    output: OutputState,
    window_warnings: Vec<String>,
}

impl PresentationState {
    fn snapshot(&self, status: LongCaptureStatus) -> LongCaptureSnapshot {
        let mut snapshot = self.snapshot.clone();
        snapshot.busy = self.acquiring_output || self.output.is_pending();
        snapshot.export_error = (!self.acquiring_output).then(|| self.output.request_error().cloned()).flatten();
        snapshot.preflight_error = (!self.acquiring_output).then(|| self.output.preflight_error().cloned()).flatten();
        snapshot.output_can_retry = !self.acquiring_output && self.output.can_retry();
        snapshot.window_warning = (!self.window_warnings.is_empty()).then(|| self.window_warnings.join("\n"));
        snapshot.result = LongCaptureResult::from_status(&status);
        snapshot.capture_error = match status {
            LongCaptureStatus::Failed { error, .. } => Some(error.to_string()),
            _ => None,
        };
        snapshot
    }

    fn start_capture_action(&mut self, result: LongCaptureResult) -> bool {
        if self.acquiring_output || self.output.is_pending() || !result.permits_output(self.snapshot.partial_accepted) {
            return false;
        }
        self.acquiring_output = true;
        self.snapshot.output_submitted = false;
        true
    }

    fn begin_output(&mut self, request: &OutputRequest) -> Option<u64> {
        let attempt = self.output.begin(request)?;
        self.acquiring_output = false;
        self.snapshot.output_submitted = request.preserves_output();
        Some(attempt)
    }

    fn report_window_warning(&mut self, message: String) {
        // Only frame and preview can degrade while keeping the toolbar alive.
        if self.window_warnings.len() < 2 && !self.window_warnings.contains(&message) {
            self.window_warnings.push(message);
        }
    }
}

/// Coordinates windows and GPU previews. The application owns capture progress and final pixels.
pub(crate) struct LongCaptureCoordinator {
    capture: LongCaptureHandle,
    state: Mutex<PresentationState>,
}
impl LongCaptureCoordinator {
    pub(crate) fn new(capture: LongCaptureHandle) -> Self {
        Self {
            capture,
            state: Mutex::new(PresentationState::default()),
        }
    }
    fn state(&self) -> MutexGuard<'_, PresentationState> {
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    pub(crate) fn snapshot(&self) -> LongCaptureSnapshot {
        let status = self.capture.status();
        self.state().snapshot(status)
    }
    pub(crate) fn register_window(&self, kind: LongCaptureWindowKind, handle: AnyWindowHandle) {
        let mut state = self.state();
        match kind {
            LongCaptureWindowKind::Frame => state.handles.frame = Some(handle),
            LongCaptureWindowKind::Toolbar => state.handles.toolbar = Some(handle),
            LongCaptureWindowKind::Preview => state.handles.preview = Some(handle),
        }
    }
    pub(crate) fn on_frame_click_through_result(&self, success: bool) {
        self.state().snapshot.frame_visible = frame_visibility_after_click_through(success);
    }
    /// Window degradation remains visible until this capture document is replaced.
    /// Capture status and preview polling must never erase native-window feedback.
    pub(crate) fn report_window_warning(&self, message: String) {
        self.state().report_window_warning(message);
    }
    pub(crate) fn start_capture_action(&self) -> bool {
        let result = LongCaptureResult::from_status(&self.capture.status());
        self.state().start_capture_action(result)
    }
    pub(crate) fn accept_partial(&self) -> bool {
        if LongCaptureResult::from_status(&self.capture.status()) != LongCaptureResult::Partial {
            return false;
        }
        let mut state = self.state();
        state.snapshot.partial_accepted = true;
        true
    }
    pub(crate) fn begin_output(&self, request: &OutputRequest) -> Option<u64> {
        self.state().begin_output(request)
    }
    pub(crate) fn accepts_output(&self, attempt: u64) -> bool {
        self.state().output.is_pending_attempt(attempt)
    }
    pub(crate) fn finish_output(&self, attempt: u64, delivery: &OutputDelivery) -> bool {
        let mut state = self.state();
        if !delivery.finish(&mut state.output, attempt) {
            return false;
        }
        state.snapshot.output_submitted = false;
        true
    }
    pub(crate) fn retry_output(&self) -> Option<OutputRequest> {
        let state = self.state();
        (!state.acquiring_output).then(|| state.output.retry()).flatten()
    }
    pub(crate) fn show_preflight_error(&self, error: OperationError) {
        let mut state = self.state();
        state.acquiring_output = false;
        state.output.show_error(error);
    }
    pub(crate) fn cancel_capture(&self) {
        self.capture.cancel();
    }
    pub(crate) async fn finish_image(&self) -> OperationResult<ImageRef> {
        let accepted = self.state().snapshot.partial_accepted;
        if let Some(result) = terminal_image(self.capture.status(), accepted) {
            return result;
        }
        self.capture.finish().await
    }

    fn refresh_preview(&self) -> bool {
        let Some(preview) = self.capture.preview() else {
            return false;
        };
        let mut state = self.state();
        if preview.revision == state.preview_revision {
            return false;
        }
        state.preview_revision = preview.revision;
        state.snapshot.preview_height_px = preview.height.min(i32::MAX as u32) as i32;
        if let Some(image) = preview.image {
            state.snapshot.preview_reduced = crate::presentation::render_image::preview_is_reduced(image.width(), image.height());
            state.snapshot.preview_image = Some(crate::presentation::render_image::from_rgba_copy(image.pixels()));
        }
        if let Some(warning) = preview.warning {
            state.snapshot.warning_text = warning;
        }
        true
    }
    fn refresh_windows<C: AppContext>(&self, cx: &mut C) -> bool {
        let handles = self.state().handles.clone();
        let frame_alive = handles
            .frame
            .is_some_and(|handle| handle.update(cx, |_, window, _| window.refresh()).is_ok());
        let toolbar_alive = handles
            .toolbar
            .is_some_and(|handle| handle.update(cx, |_, window, _| window.refresh()).is_ok());
        let preview_alive = handles
            .preview
            .is_some_and(|handle| handle.update(cx, |_, window, _| window.refresh()).is_ok());
        let mut state = self.state();
        if !frame_alive {
            state.handles.frame = None;
        }
        if !toolbar_alive {
            state.handles.toolbar = None;
        }
        if !preview_alive {
            state.handles.preview = None;
        }
        frame_alive || toolbar_alive || preview_alive
    }
    pub(crate) fn ensure_runtime_poller<V: 'static>(self: &Arc<Self>, window: &mut Window, cx: &mut Context<V>) {
        {
            let mut state = self.state();
            if state.poller_running {
                return;
            }
            state.poller_running = true;
        }
        let coordinator = self.clone();
        let mut status = self.capture.subscribe();
        cx.spawn_in(window, move |_: WeakEntity<V>, cx: &mut AsyncWindowContext| {
            let mut cx = cx.clone();
            async move {
                let mut observing = true;
                loop {
                    // A latest-value slot bounds preview memory independently of rendering speed.
                    let timer = cx.background_executor().timer(Duration::from_millis(33));
                    tokio::select! { _ = timer => {}, change = status.changed(), if observing => { if change.is_err() { observing = false; } } }
                    coordinator.refresh_preview();
                    if !coordinator.refresh_windows(&mut cx) {
                        coordinator.capture.cancel();
                        break;
                    }
                }
                coordinator.state().poller_running = false;
            }
        })
        .detach();
    }
    pub(crate) fn close_windows_except<C: AppContext>(&self, except: Option<WindowId>, cx: &mut C) {
        let handles = self.state().handles.clone();
        for handle in [handles.frame, handles.toolbar, handles.preview].into_iter().flatten() {
            if except.is_some_and(|id| id == handle.window_id()) {
                continue;
            }
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
        let mut state = self.state();
        state.handles.frame = state.handles.frame.filter(|handle| Some(handle.window_id()) == except);
        state.handles.toolbar = state.handles.toolbar.filter(|handle| Some(handle.window_id()) == except);
        state.handles.preview = state.handles.preview.filter(|handle| Some(handle.window_id()) == except);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minnow_application::{ErrorKind, OperationError};
    use minnow_imaging::ImageAsset;

    #[test]
    fn failed_capture_requires_an_explicit_choice_before_partial_output() {
        let image = ImageAsset::new(image::RgbaImage::new(2, 3));
        let failed = LongCaptureStatus::Failed {
            error: OperationError::new(ErrorKind::Native, "capture stopped"),
            partial: Some(image.clone()),
        };
        let projection = LongCaptureResult::from_status(&failed);
        assert_eq!(projection, LongCaptureResult::Partial);
        assert!(!projection.permits_output(false));
        assert!(terminal_image(failed.clone(), false).unwrap().is_err());
        assert!(projection.permits_output(true));
        for _ in 0..2 {
            let output = terminal_image(failed.clone(), true).unwrap().unwrap();
            assert!(Arc::ptr_eq(&output, &image), "retry must keep the captured image");
        }
        assert!(matches!(failed, LongCaptureStatus::Failed { partial: Some(_), .. }));
    }

    #[test]
    fn failure_without_pixels_never_offers_output() {
        let failed = LongCaptureStatus::Failed {
            error: OperationError::new(ErrorKind::Native, "capture stopped"),
            partial: None,
        };
        assert_eq!(LongCaptureResult::from_status(&failed), LongCaptureResult::Failed);
        assert!(!LongCaptureResult::Failed.permits_output(true));
        assert!(terminal_image(failed, true).unwrap().is_err());
    }

    #[test]
    fn accepted_partial_output_keeps_one_frozen_retry_without_reentering_acquisition() {
        use crate::flows::output::{CaptureActionEffect, CaptureIntent};
        use minnow_application::ImageInput;

        let image = ImageAsset::new(image::RgbaImage::new(2, 3));
        let source = Arc::downgrade(&image);
        let failed = LongCaptureStatus::Failed {
            error: OperationError::new(ErrorKind::Native, "capture stopped"),
            partial: Some(image),
        };
        let mut state = PresentationState::default();
        assert!(!state.start_capture_action(LongCaptureResult::from_status(&failed)));
        state.snapshot.partial_accepted = true;
        assert!(state.start_capture_action(LongCaptureResult::from_status(&failed)));
        assert!(!state.start_capture_action(LongCaptureResult::from_status(&failed)));

        let input = ImageInput::image(terminal_image(failed.clone(), true).unwrap().unwrap(), None).unwrap();
        let request = CaptureIntent::Copy.output(input, false).unwrap();
        let attempt = state.begin_output(&request).unwrap();
        assert!(!state.acquiring_output, "only acquisition owns the partial-result gate");
        assert!(state.begin_output(&request).is_none());
        assert!(!state.start_capture_action(LongCaptureResult::from_status(&failed)));
        drop(request);

        let busy = OperationError::new(ErrorKind::Busy, "clipboard output is occupied");
        state.output.finish(attempt, &CaptureActionEffect::Warning(busy.clone()));
        let frame = state.snapshot(failed.clone());
        assert_eq!(frame.result, LongCaptureResult::Partial, "export cannot turn failed capture into success");
        assert_eq!(frame.export_error, Some(busy));
        assert!(frame.preflight_error.is_none());
        assert!(frame.output_can_retry);
        assert!(!frame.busy);
        let retry = state.output.retry().unwrap();
        assert!(matches!(
            &retry,
            OutputRequest::Image {
                command: minnow_application::ImageCommand::Copy,
                ..
            }
        ));
        let retry_attempt = state.begin_output(&retry).unwrap();
        assert!(retry_attempt > attempt);
        assert!(!state.acquiring_output, "retry already owns validated pixels");
        assert!(state.snapshot(failed.clone()).export_error.is_none());

        drop(failed);
        drop(state);
        assert!(source.upgrade().is_some(), "the frozen retry keeps accepted pixels independently");
        drop(retry);
        assert!(source.upgrade().is_none());
    }

    #[test]
    fn native_window_warnings_are_bounded_and_survive_capture_and_output_feedback() {
        let mut state = PresentationState::default();
        state.report_window_warning("frame hidden".into());
        state.report_window_warning("preview hidden".into());
        state.report_window_warning("frame hidden".into());
        state.snapshot.warning_text = "capture preview changed".into();
        state.output.show_error(OperationError::new(ErrorKind::Io, "output failed"));
        for status in [
            LongCaptureStatus::Running,
            LongCaptureStatus::Failed {
                error: OperationError::new(ErrorKind::Native, "capture failed"),
                partial: None,
            },
        ] {
            let frame = state.snapshot(status);
            assert_eq!(frame.window_warning.as_deref(), Some("frame hidden\npreview hidden"));
            assert_eq!(state.window_warnings.len(), 2);
        }
        assert!(PresentationState::default().snapshot(LongCaptureStatus::Queued).window_warning.is_none());
    }
}

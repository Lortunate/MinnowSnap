use super::LongCaptureRequest;
use super::layout::frame_visibility_after_click_through;
use crate::support::render_image;
use gpui::{AnyWindowHandle, AppContext, AsyncWindowContext, Context, RenderImage, WeakEntity, Window, WindowId};
use image::RgbaImage;
use minnow_core::capture::long_capture::{LongCaptureEvent, LongCaptureRuntime};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const COORDINATOR_POLL_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LongCaptureWindowKind {
    Frame,
    Toolbar,
    Preview,
}

#[derive(Clone)]
pub(crate) struct LongCaptureSnapshot {
    pub(crate) preview_image: Option<Arc<RenderImage>>,
    pub(crate) preview_height_px: i32,
    pub(crate) warning_text: String,
    pub(crate) busy: bool,
    final_image: Option<RgbaImage>,
    pub(crate) frame_visible: bool,
}

impl Default for LongCaptureSnapshot {
    fn default() -> Self {
        Self {
            preview_image: None,
            preview_height_px: 0,
            warning_text: String::new(),
            busy: false,
            final_image: None,
            frame_visible: true,
        }
    }
}

#[derive(Clone, Default)]
struct LongCaptureWindowHandles {
    frame: Option<AnyWindowHandle>,
    toolbar: Option<AnyWindowHandle>,
    preview: Option<AnyWindowHandle>,
}

#[derive(Default)]
struct LongCaptureCoordinatorState {
    snapshot: LongCaptureSnapshot,
    handles: LongCaptureWindowHandles,
    revision: u64,
    poller_running: bool,
}

pub(crate) struct LongCaptureCoordinator {
    runtime: LongCaptureRuntime,
    state: Mutex<LongCaptureCoordinatorState>,
}

impl LongCaptureCoordinator {
    pub(crate) fn new(request: LongCaptureRequest) -> Self {
        let runtime = LongCaptureRuntime::new();
        runtime.start_with_viewport(
            request.selection_rect,
            minnow_core::geometry::RectF::new(
                request.viewport_rect.x,
                request.viewport_rect.y,
                request.viewport_rect.width,
                request.viewport_rect.height,
            ),
            request.viewport_scale as f32,
        );

        Self {
            runtime,
            state: Mutex::new(LongCaptureCoordinatorState {
                revision: 1,
                ..LongCaptureCoordinatorState::default()
            }),
        }
    }

    fn revision(&self) -> u64 {
        self.state.lock().map(|state| state.revision).unwrap_or(0)
    }

    fn poll_runtime_events(&self) -> u64 {
        let events = self.runtime.drain_events();
        if events.is_empty() {
            return self.revision();
        }

        let mut changed = false;
        if let Ok(mut state) = self.state.lock() {
            for event in events {
                match event {
                    LongCaptureEvent::Started => {
                        changed = true;
                    }
                    LongCaptureEvent::Progress { height, preview_image } => {
                        state.snapshot.preview_height_px = height;
                        state.snapshot.preview_image = Some(render_image::from_rgba(preview_image));
                        changed = true;
                    }
                    LongCaptureEvent::Warning { text } => {
                        state.snapshot.warning_text = text;
                        changed = true;
                    }
                    LongCaptureEvent::Finished { final_image } => {
                        if let Some(image) = final_image {
                            state.snapshot.final_image = Some(image);
                        }
                        state.snapshot.busy = false;
                        changed = true;
                    }
                }
            }

            if changed {
                state.revision = state.revision.saturating_add(1);
            }
            return state.revision;
        }

        0
    }

    pub(crate) fn snapshot(&self) -> LongCaptureSnapshot {
        self.state.lock().map(|state| state.snapshot.clone()).unwrap_or_default()
    }

    fn has_registered_windows(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.handles.frame.is_some() || state.handles.toolbar.is_some() || state.handles.preview.is_some())
            .unwrap_or(false)
    }

    fn notify_registered_windows<C: AppContext>(&self, cx: &mut C) -> bool {
        let handles = self.state.lock().map(|state| state.handles.clone()).ok();
        let Some(handles) = handles else {
            return false;
        };

        let frame_alive = handles
            .frame
            .is_some_and(|handle| handle.update(cx, |_, window, _| window.refresh()).is_ok());
        let toolbar_alive = handles
            .toolbar
            .is_some_and(|handle| handle.update(cx, |_, window, _| window.refresh()).is_ok());
        let preview_alive = handles
            .preview
            .is_some_and(|handle| handle.update(cx, |_, window, _| window.refresh()).is_ok());

        if let Ok(mut state) = self.state.lock() {
            if !frame_alive {
                state.handles.frame = None;
            }
            if !toolbar_alive {
                state.handles.toolbar = None;
            }
            if !preview_alive {
                state.handles.preview = None;
            }
        }

        frame_alive || toolbar_alive || preview_alive
    }

    fn mark_poller_stopped(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.poller_running = false;
        }
    }

    pub(crate) fn ensure_runtime_poller<V>(self: &Arc<Self>, window: &mut Window, cx: &mut Context<V>)
    where
        V: 'static,
    {
        let should_spawn = if let Ok(mut state) = self.state.lock() {
            if state.poller_running {
                false
            } else {
                state.poller_running = true;
                true
            }
        } else {
            false
        };

        if !should_spawn {
            return;
        }

        let coordinator = self.clone();
        cx.spawn_in(window, move |_this: WeakEntity<V>, cx: &mut AsyncWindowContext| {
            let mut cx = cx.clone();
            async move {
                let mut revision = coordinator.revision();
                loop {
                    cx.background_executor().timer(COORDINATOR_POLL_INTERVAL).await;
                    let next_revision = coordinator.poll_runtime_events();
                    if next_revision != revision {
                        revision = next_revision;
                        if !coordinator.notify_registered_windows(&mut cx) {
                            break;
                        }
                    } else if !coordinator.has_registered_windows() {
                        break;
                    }
                }
                coordinator.mark_poller_stopped();
            }
        })
        .detach();
    }

    pub(crate) fn register_window(&self, kind: LongCaptureWindowKind, handle: AnyWindowHandle) {
        if let Ok(mut state) = self.state.lock() {
            match kind {
                LongCaptureWindowKind::Frame => state.handles.frame = Some(handle),
                LongCaptureWindowKind::Toolbar => state.handles.toolbar = Some(handle),
                LongCaptureWindowKind::Preview => state.handles.preview = Some(handle),
            }
            state.revision = state.revision.saturating_add(1);
        }
    }

    pub(crate) fn on_frame_click_through_result(&self, success: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.snapshot.frame_visible = frame_visibility_after_click_through(success);
            state.revision = state.revision.saturating_add(1);
        }
    }

    pub(crate) fn start_capture_action(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.snapshot.busy = true;
            state.snapshot.warning_text.clear();
            state.revision = state.revision.saturating_add(1);
        }
    }

    pub(crate) fn finish_capture_action_with_warning(&self, warning_text: String) {
        if let Ok(mut state) = self.state.lock() {
            state.snapshot.busy = false;
            state.snapshot.warning_text = warning_text;
            state.revision = state.revision.saturating_add(1);
        }
    }

    pub(crate) fn cancel_capture(&self) {
        self.runtime.stop();
    }

    pub(crate) fn take_capture_image(&self, timeout: Duration) -> Option<RgbaImage> {
        if let Ok(mut state) = self.state.lock()
            && let Some(image) = state.snapshot.final_image.take()
        {
            state.revision = state.revision.saturating_add(1);
            return Some(image);
        }

        self.runtime.stop_and_take_result(timeout)
    }

    pub(crate) fn close_windows_except<C: AppContext>(&self, except: Option<WindowId>, cx: &mut C) {
        let handles = self.state.lock().map(|state| state.handles.clone()).ok();
        let Some(handles) = handles else {
            return;
        };

        for handle in [handles.frame, handles.toolbar, handles.preview].into_iter().flatten() {
            if except.is_some_and(|id| id == handle.window_id()) {
                continue;
            }
            let _ = handle.update(cx, |_, window, _| {
                window.remove_window();
            });
        }

        if let Ok(mut state) = self.state.lock() {
            state.handles.frame = state.handles.frame.filter(|handle| except.is_some_and(|id| id == handle.window_id()));
            state.handles.toolbar = state.handles.toolbar.filter(|handle| except.is_some_and(|id| id == handle.window_id()));
            state.handles.preview = state.handles.preview.filter(|handle| except.is_some_and(|id| id == handle.window_id()));
        }
    }
}

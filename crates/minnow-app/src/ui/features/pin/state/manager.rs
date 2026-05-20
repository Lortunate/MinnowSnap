use gpui::{AnyWindowHandle, App, AppContext, Entity, Global, WindowId};
use tracing::{info, warn};

#[derive(Clone)]
pub(in crate::ui::features::pin) struct PinManager(Entity<PinManagerState>);

impl Global for PinManager {}

impl PinManager {
    pub(in crate::ui::features::pin) fn new(cx: &mut App) -> Self {
        Self(cx.new(|_| PinManagerState::default()))
    }

    pub(in crate::ui::features::pin) fn register(&self, handle: AnyWindowHandle, cx: &mut App) {
        self.prune_closed(cx);
        self.0.update(cx, |state, _| {
            state.register(handle);
        });
    }

    pub(in crate::ui::features::pin) fn unregister(&self, window_id: WindowId, cx: &mut App) {
        let _ = self.0.update(cx, |state, _| state.unregister(window_id));
    }

    pub(in crate::ui::features::pin) fn close_all(&self, cx: &mut App) {
        let handles = self.prune_closed(cx);
        info!(target: "minnowsnap::pin", count = handles.len(), "closing all pin windows");

        let mut succeeded_count = 0usize;
        let mut failed_count = 0usize;

        for handle in handles {
            match handle.update(cx, |_, window, _| {
                window.remove_window();
            }) {
                Ok(_) => {
                    succeeded_count += 1;
                }
                Err(_) => {
                    failed_count += 1;
                }
            }
        }

        if failed_count == 0 {
            self.0.update(cx, |state, _| state.clear());
        } else {
            let remaining_count = self.prune_closed(cx).len();
            warn!(
                target: "minnowsnap::pin",
                succeeded_count,
                failed_count,
                remaining_count,
                "failed to close some pin windows"
            );
        }
    }

    pub(in crate::ui::features::pin) fn prune_closed(&self, cx: &mut App) -> Vec<AnyWindowHandle> {
        let snapshot = self.0.read(cx).handles();
        let open_window_ids = cx.windows().into_iter().map(|handle| handle.window_id()).collect::<Vec<_>>();
        let live_handles = snapshot
            .into_iter()
            .filter(|handle| open_window_ids.contains(&handle.window_id()))
            .collect::<Vec<_>>();

        self.0.update(cx, |state, _| state.replace(live_handles.clone()));
        live_handles
    }
}

#[derive(Default)]
struct PinManagerState {
    windows: Vec<TrackedPinWindow>,
}

impl PinManagerState {
    fn register(&mut self, handle: AnyWindowHandle) {
        let tracked = TrackedPinWindow::new(handle);
        self.windows.retain(|existing| existing.id != tracked.id);
        self.windows.push(tracked);
    }

    fn unregister(&mut self, window_id: WindowId) -> bool {
        let original_len = self.windows.len();
        self.windows.retain(|tracked| tracked.id != window_id);
        original_len != self.windows.len()
    }

    fn replace(&mut self, handles: Vec<AnyWindowHandle>) {
        self.windows = handles.into_iter().map(TrackedPinWindow::new).collect();
    }

    fn handles(&self) -> Vec<AnyWindowHandle> {
        self.windows.iter().map(|tracked| tracked.handle).collect()
    }

    fn clear(&mut self) {
        self.windows.clear();
    }
}

#[derive(Clone, Copy)]
struct TrackedPinWindow {
    id: WindowId,
    handle: AnyWindowHandle,
}

impl TrackedPinWindow {
    fn new(handle: AnyWindowHandle) -> Self {
        Self {
            id: handle.window_id(),
            handle,
        }
    }
}

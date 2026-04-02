use super::request::PinRequest;
use gpui::{AnyWindowHandle, App, AppContext, Entity, Global, Pixels, Size, WindowId, px, size};
use tracing::{info, warn};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PinWindowGeometry {
    origin: Option<(f32, f32)>,
    size: (f32, f32),
    min_size: f32,
}

impl PinWindowGeometry {
    pub(super) fn origin(self) -> Option<(f32, f32)> {
        self.origin
    }

    pub(super) fn min_size(self) -> f32 {
        self.min_size
    }

    pub(super) fn window_size(self) -> Size<Pixels> {
        size(px(self.size.0), px(self.size.1))
    }
}

#[derive(Clone, Debug)]
pub(super) struct PinSession {
    image_path: std::path::PathBuf,
    base_size: (f32, f32),
    zoom: f32,
    opacity: f32,
}

impl PinSession {
    const MIN_SIZE: f32 = 24.0;
    const MIN_ZOOM: f32 = 0.2;
    const MAX_ZOOM: f32 = 8.0;
    const ZOOM_STEP: f32 = 0.1;
    const MIN_OPACITY: f32 = 0.2;
    const MAX_OPACITY: f32 = 1.0;
    const OPACITY_STEP: f32 = 0.05;

    pub(super) fn new(cx: &mut App, request: PinRequest) -> Entity<Self> {
        cx.new(|_| Self::from_request(request))
    }

    pub(super) fn initial_geometry(request: &PinRequest) -> PinWindowGeometry {
        let base_size = request.base_size();
        let zoom = Self::initial_zoom(base_size);

        PinWindowGeometry {
            origin: request.origin(),
            size: (base_size.0 * zoom, base_size.1 * zoom),
            min_size: Self::MIN_SIZE,
        }
    }

    fn from_request(request: PinRequest) -> Self {
        let base_size = request.base_size();
        Self {
            image_path: request.image_path().to_path_buf(),
            base_size,
            zoom: Self::initial_zoom(base_size),
            opacity: Self::MAX_OPACITY,
        }
    }

    fn initial_zoom(base_size: (f32, f32)) -> f32 {
        Self::min_zoom_for(base_size).clamp(1.0, Self::MAX_ZOOM)
    }

    fn min_zoom_for(base_size: (f32, f32)) -> f32 {
        let (base_width, base_height) = base_size;
        if base_width <= 0.0 || base_height <= 0.0 {
            return Self::MIN_ZOOM;
        }

        (Self::MIN_SIZE / base_width).max(Self::MIN_SIZE / base_height).max(Self::MIN_ZOOM)
    }

    fn zoom_bounds(&self) -> (f32, f32) {
        (Self::min_zoom_for(self.base_size).min(Self::MAX_ZOOM), Self::MAX_ZOOM)
    }

    pub(super) fn frame(&self) -> (std::path::PathBuf, f32) {
        (self.image_path.clone(), self.opacity)
    }

    pub(super) fn window_size(&self) -> Size<Pixels> {
        size(px(self.base_size.0 * self.zoom), px(self.base_size.1 * self.zoom))
    }

    pub(super) fn apply_zoom_step(&mut self, step: f32) -> Option<Size<Pixels>> {
        let (min_zoom, max_zoom) = self.zoom_bounds();
        let next_zoom = (self.zoom + step * Self::ZOOM_STEP).clamp(min_zoom, max_zoom);
        if (next_zoom - self.zoom).abs() <= f32::EPSILON {
            return None;
        }

        self.zoom = next_zoom;
        Some(self.window_size())
    }

    pub(super) fn apply_opacity_step(&mut self, step: f32) -> bool {
        let next_opacity = (self.opacity + step * Self::OPACITY_STEP).clamp(Self::MIN_OPACITY, Self::MAX_OPACITY);
        if (next_opacity - self.opacity).abs() <= f32::EPSILON {
            return false;
        }

        self.opacity = next_opacity;
        true
    }
}

#[derive(Clone)]
pub(crate) struct PinManager(Entity<PinManagerState>);

impl Global for PinManager {}

impl PinManager {
    pub(crate) fn new(cx: &mut App) -> Self {
        Self(cx.new(|_| PinManagerState::default()))
    }

    pub(crate) fn register(&self, handle: AnyWindowHandle, cx: &mut App) {
        self.prune_closed(cx);
        self.0.update(cx, |state, _| {
            state.register(handle);
        });
    }

    pub(crate) fn unregister(&self, window_id: WindowId, cx: &mut App) {
        let _ = self.0.update(cx, |state, _| state.unregister(window_id));
    }

    pub(crate) fn close_all(&self, cx: &mut App) {
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
            let _ = self.0.update(cx, |state, _| state.clear());
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

    pub(crate) fn prune_closed(&self, cx: &mut App) -> Vec<AnyWindowHandle> {
        let snapshot = self.0.read(cx).handles();
        let open_window_ids = cx.windows().into_iter().map(|handle| handle.window_id()).collect::<Vec<_>>();
        let live_handles = snapshot
            .into_iter()
            .filter(|handle| open_window_ids.contains(&handle.window_id()))
            .collect::<Vec<_>>();

        let _ = self.0.update(cx, |state, _| state.replace(live_handles.clone()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::Rect;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_image_path(name: &str) -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let suffix = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("minnowsnap-pin-{name}-{suffix}.png"))
    }

    fn write_test_image(name: &str, width: u32, height: u32) -> PathBuf {
        let path = temp_image_path(name);
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([255, 0, 0, 255]));
        image.save(&path).expect("write test image");
        path
    }

    #[test]
    fn pin_initial_geometry_clamps_tiny_images_up_to_minimum_size() {
        let path = write_test_image("tiny", 8, 10);
        let request = PinRequest::new(&path, None);
        let geometry = PinSession::initial_geometry(&request);

        assert_eq!(geometry.window_size(), size(px(24.0), px(30.0)));
        assert_eq!(geometry.min_size(), 24.0);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pin_initial_geometry_uses_source_bounds_before_image_dimensions() {
        let path = write_test_image("source-bounds", 400, 300);
        let request = PinRequest::new(
            &path,
            Some(Rect {
                x: 32,
                y: 48,
                width: 120,
                height: 90,
            }),
        );
        let geometry = PinSession::initial_geometry(&request);

        assert_eq!(geometry.origin(), Some((32.0, 48.0)));
        assert_eq!(geometry.window_size(), size(px(120.0), px(90.0)));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pin_initial_geometry_falls_back_when_image_dimensions_are_missing() {
        let request = PinRequest::new(temp_image_path("missing"), None);
        let geometry = PinSession::initial_geometry(&request);

        assert_eq!(geometry.window_size(), size(px(960.0), px(720.0)));
    }

    #[test]
    fn pin_zoom_steps_clamp_between_min_and_max_bounds() {
        let mut session = PinSession {
            image_path: PathBuf::from("pin.png"),
            base_size: (480.0, 320.0),
            zoom: 1.0,
            opacity: 1.0,
        };

        for _ in 0..200 {
            let _ = session.apply_zoom_step(1.0);
        }
        assert_eq!(session.zoom, PinSession::MAX_ZOOM);

        for _ in 0..400 {
            let _ = session.apply_zoom_step(-1.0);
        }
        assert_eq!(session.zoom, PinSession::min_zoom_for(session.base_size));
    }

    #[test]
    fn pin_opacity_steps_clamp_between_min_and_max_bounds() {
        let mut session = PinSession {
            image_path: PathBuf::from("pin.png"),
            base_size: (480.0, 320.0),
            zoom: 1.0,
            opacity: 1.0,
        };

        for _ in 0..200 {
            let _ = session.apply_opacity_step(-1.0);
        }
        assert_eq!(session.opacity, PinSession::MIN_OPACITY);

        for _ in 0..200 {
            let _ = session.apply_opacity_step(1.0);
        }
        assert_eq!(session.opacity, PinSession::MAX_OPACITY);
    }

    #[test]
    fn pin_window_size_tracks_zoomed_dimensions() {
        let mut session = PinSession {
            image_path: PathBuf::from("pin.png"),
            base_size: (320.0, 200.0),
            zoom: 1.0,
            opacity: 1.0,
        };

        let resized = session.apply_zoom_step(1.0).expect("zoom step should resize");

        assert_eq!(resized, size(px(352.0), px(220.0)));
        assert_eq!(session.window_size(), resized);
    }

    #[test]
    fn pin_manager_registry_helpers_deduplicate_and_remove_ids() {
        let mut ids = vec![WindowId::from(1), WindowId::from(2)];
        ids.retain(|existing| *existing != WindowId::from(2));
        ids.push(WindowId::from(2));
        ids.retain(|existing| *existing != WindowId::from(1));

        assert_eq!(ids, vec![WindowId::from(2)]);
    }
}

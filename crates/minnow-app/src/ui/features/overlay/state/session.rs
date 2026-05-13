use gpui::{App, AppContext, Entity, Global, Pixels, Point, RenderImage, Window};
use image::RgbaImage;
use std::sync::Arc;

#[cfg(feature = "overlay-diagnostics")]
use super::diagnostics::{OverlayDiagnostics, OverlayDiagnosticsSnapshot};
use super::{OverlaySurface, PickerFormat, PickerNeighborhood, PickerSample};
use crate::services::geometry::{RectF, clamp_point, normalize_rect};
use crate::ui::features::overlay::annotation::{AnnotationEngine, AnnotationUiState};
use crate::ui::features::overlay::window_catalog::{WindowInfo, find_window_at};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResizeCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Left,
    Right,
    Top,
    Bottom,
}

impl ResizeCorner {
    pub(crate) const fn geometry_key(self) -> &'static str {
        match self {
            Self::TopLeft => "top-left",
            Self::TopRight => "top-right",
            Self::BottomLeft => "bottom-left",
            Self::BottomRight => "bottom-right",
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum DragMode {
    #[default]
    Idle,
    Selecting,
    Resizing(ResizeCorner),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct OverlayViewportModel {
    pub(super) mode: DragMode,
    pub(super) selection: Option<RectF>,
    pub(super) target: Option<RectF>,
    pub(super) drag_start: Option<Point<Pixels>>,
    pub(super) drag_start_rect: Option<RectF>,
    /// During selection moves, `selection` is updated live while dragging.
    /// `selection_move_origin` captures the selection rect at drag start, and
    /// `selection_move_delta` is the current (dx, dy) relative to that origin.
    pub(super) selection_move_delta: Option<(f64, f64)>,
    pub(super) selection_move_origin: Option<RectF>,
    pub(super) confirm_target_on_release: bool,
    pub(super) viewport_w: f64,
    pub(super) viewport_h: f64,
    pub(super) pending_pointer: Option<Point<Pixels>>,
}

impl Default for OverlayViewportModel {
    fn default() -> Self {
        Self {
            mode: DragMode::Idle,
            selection: None,
            target: None,
            drag_start: None,
            drag_start_rect: None,
            selection_move_delta: None,
            selection_move_origin: None,
            confirm_target_on_release: false,
            viewport_w: 0.0,
            viewport_h: 0.0,
            pending_pointer: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct OverlaySession {
    pub(super) viewport: OverlayViewportModel,
    pub(super) background_image: Option<Arc<RenderImage>>,
    pub(super) background_pixels: Option<Arc<RgbaImage>>,
    pub(super) hovered_window: Option<WindowInfo>,
    pub(super) picker_cursor: Option<(f64, f64)>,
    pub(super) picker_last_pointer: Option<(f64, f64)>,
    pub(super) picker_pointer_lock: Option<(f64, f64)>,
    pub(super) picker_sample: Option<PickerSample>,
    pub(super) picker_neighborhood: Option<PickerNeighborhood>,
    pub(super) picker_format: PickerFormat,
    pub(super) annotation: AnnotationEngine,
    #[cfg(feature = "overlay-diagnostics")]
    pub(super) diagnostics: OverlayDiagnostics,
    pub(super) windows: Vec<WindowInfo>,
}

#[derive(Clone, Debug)]
pub(crate) struct PickerVm {
    pub cursor: Option<(f64, f64)>,
    pub sample: Option<PickerSample>,
    pub neighborhood: Option<PickerNeighborhood>,
    pub format: PickerFormat,
}

#[derive(Clone)]
pub(crate) struct SelectionVm {
    pub selection: Option<RectF>,
    pub target: Option<RectF>,
    pub drag_mode: DragMode,
}

#[derive(Clone, Default)]
pub(crate) struct HudVm {
    pub hovered_window: Option<WindowInfo>,
}

#[derive(Clone)]
pub(crate) struct OverlayFrame {
    pub background_image: Option<Arc<RenderImage>>,
    pub selection: SelectionVm,
    pub selection_move_delta: Option<(f64, f64)>,
    pub picker: Option<PickerVm>,
    pub annotation: AnnotationUiState,
    pub hud: HudVm,
    #[cfg(feature = "overlay-diagnostics")]
    pub diagnostics: OverlayDiagnosticsSnapshot,
}

#[derive(Clone)]
pub struct OverlayHandle(pub(super) Entity<OverlaySession>);

impl Global for OverlayHandle {}

impl OverlayHandle {
    pub fn new(cx: &mut App) -> Self {
        Self(cx.new(|_| OverlaySession::default()))
    }

    pub fn prepare(&self, cx: &mut App) {
        let surface = OverlaySurface::capture();
        self.0.update(cx, |session, _| {
            session.prepare_surface(surface);
        });
    }

    pub(crate) fn session(&self) -> Entity<OverlaySession> {
        self.0.clone()
    }

    pub(super) fn sync_viewport(&self, window: &Window, cx: &mut App) {
        let viewport = window.viewport_size();
        let viewport_w = viewport.width.to_f64();
        let viewport_h = viewport.height.to_f64();
        self.0.update(cx, |session, _| session.set_viewport_size(viewport_w, viewport_h));
    }
}

impl OverlaySession {
    pub(super) const MIN_SELECTION_SIZE: f64 = 8.0;

    pub(crate) fn mode(&self) -> DragMode {
        self.viewport.mode
    }

    pub(crate) fn selection(&self) -> Option<RectF> {
        self.viewport.selection
    }

    pub(crate) fn has_selection(&self) -> bool {
        self.viewport.selection.is_some()
    }

    pub(crate) fn prepare_surface(&mut self, surface: OverlaySurface) {
        self.reset_interaction_state();
        self.background_image = surface.background_image;
        self.background_pixels = surface.background_pixels;
        self.windows = surface.windows;
        self.picker_cursor = None;
        self.picker_sample = None;
        self.picker_neighborhood = None;
        self.picker_format = PickerFormat::Hex;
        self.clear_annotation_state();
        self.refresh_picker_sample();
    }

    pub(crate) fn set_viewport_size(&mut self, viewport_w: f64, viewport_h: f64) {
        self.viewport.viewport_w = viewport_w.max(0.0);
        self.viewport.viewport_h = viewport_h.max(0.0);

        if let Some(selection) = self.viewport.selection {
            self.viewport.selection = Some(self.clamp_rect_to_viewport(selection));
        }
        if let Some(target) = self.viewport.target {
            self.viewport.target = Some(self.clamp_rect_to_viewport(target));
        }
        self.refresh_picker_sample();
    }

    pub(super) fn reset_interaction_state(&mut self) {
        self.viewport.mode = DragMode::Idle;
        self.viewport.selection = None;
        self.viewport.target = None;
        self.hovered_window = None;
        self.viewport.drag_start = None;
        self.viewport.drag_start_rect = None;
        self.viewport.selection_move_delta = None;
        self.viewport.selection_move_origin = None;
        self.viewport.pending_pointer = None;
        self.viewport.confirm_target_on_release = false;
        self.cancel_annotation_interaction_state();
        self.picker_last_pointer = None;
        self.picker_pointer_lock = None;
    }

    pub(crate) fn queue_pointer(&mut self, point: Point<Pixels>) -> bool {
        self.diag_on_pointer_event();
        let was_empty = self.viewport.pending_pointer.is_none();
        self.viewport.pending_pointer = Some(point);
        self.diag_on_pointer_queue(was_empty);
        was_empty
    }

    pub(crate) fn apply_pending_pointer(&mut self) -> bool {
        let Some(point) = self.viewport.pending_pointer.take() else {
            return false;
        };
        self.diag_on_pointer_applied();

        let mut refresh = false;
        match self.mode() {
            DragMode::Selecting => {
                self.update_selection(point);
                refresh = true;
            }
            DragMode::Resizing(_) => {
                self.update_resize(point);
                refresh = true;
            }
            DragMode::Idle => {
                if self.viewport.selection_move_origin.is_some() {
                    self.update_move(point);
                    refresh = true;
                } else if self.has_active_annotation_interaction() {
                    refresh = self.update_annotation_interaction(point) || refresh;
                } else {
                    refresh = self.update_pointer(point) || refresh;
                    refresh = self.update_hover(point) || refresh;
                }
            }
        }
        refresh
    }

    pub(crate) fn update_hover(&mut self, point: Point<Pixels>) -> bool {
        if self.viewport.mode != DragMode::Idle || self.viewport.selection.is_some() {
            return false;
        }

        let (next_target, next_hovered_window) = match find_window_at(&self.windows, point.x.to_f64(), point.y.to_f64()) {
            Some(idx) => {
                let target = &self.windows[idx];
                (
                    Some(RectF::new(
                        f64::from(target.x),
                        f64::from(target.y),
                        f64::from(target.width),
                        f64::from(target.height),
                    )),
                    Some(target.clone()),
                )
            }
            None => (None, None),
        };

        let hovered_changed = self.hovered_window.as_ref().map(|w| (&w.title, &w.app_name, w.x, w.y, w.width, w.height))
            != next_hovered_window.as_ref().map(|w| (&w.title, &w.app_name, w.x, w.y, w.width, w.height));
        let changed = self.viewport.target != next_target || hovered_changed;
        self.viewport.target = next_target;
        self.hovered_window = next_hovered_window;
        changed
    }

    pub(super) fn selection_rect(&self) -> Option<crate::services::geometry::Rect> {
        self.viewport.selection.and_then(|selection| {
            if selection.width <= 0.0 || selection.height <= 0.0 {
                return None;
            }
            Some(normalize_rect(selection.x, selection.y, selection.width, selection.height))
        })
    }

    pub(crate) fn clamp_point_to_viewport(&self, point: Point<Pixels>) -> (f64, f64) {
        clamp_point(point.x.to_f64(), point.y.to_f64(), self.viewport.viewport_w, self.viewport.viewport_h)
    }

    pub(super) fn clamp_rect_to_viewport(&self, rect: RectF) -> RectF {
        let max_w = self.viewport.viewport_w.max(0.0);
        let max_h = self.viewport.viewport_h.max(0.0);
        let x = rect.x.clamp(0.0, max_w);
        let y = rect.y.clamp(0.0, max_h);
        let width = rect.width.max(0.0).min((max_w - x).max(0.0));
        let height = rect.height.max(0.0).min((max_h - y).max(0.0));
        RectF::new(x, y, width, height)
    }

    pub(super) fn diag_on_render(&mut self) {
        #[cfg(feature = "overlay-diagnostics")]
        {
            self.diagnostics.on_render();
        }
    }

    pub(super) fn diag_on_refresh(&mut self) {
        #[cfg(feature = "overlay-diagnostics")]
        {
            self.diagnostics.on_refresh();
        }
    }

    fn diag_on_pointer_event(&mut self) {
        #[cfg(feature = "overlay-diagnostics")]
        {
            self.diagnostics.on_pointer_event();
        }
    }

    fn diag_on_pointer_queue(&mut self, queued_first: bool) {
        #[cfg(feature = "overlay-diagnostics")]
        {
            self.diagnostics.on_pointer_queue(queued_first);
        }
        #[cfg(not(feature = "overlay-diagnostics"))]
        {
            let _ = queued_first;
        }
    }

    fn diag_on_pointer_applied(&mut self) {
        #[cfg(feature = "overlay-diagnostics")]
        {
            self.diagnostics.on_pointer_applied();
        }
    }

    #[cfg(feature = "overlay-diagnostics")]
    pub(super) fn diagnostics_snapshot(&mut self) -> OverlayDiagnosticsSnapshot {
        self.diagnostics.snapshot(self.annotation.raster_diagnostics())
    }
}

use gpui::{App, AppContext, Entity, Global, Pixels, Point, RenderImage, Window};
use image::RgbaImage;
use std::sync::Arc;

#[cfg(feature = "overlay-diagnostics")]
use super::diagnostics::{OverlayDiagnostics, OverlayDiagnosticsSnapshot};
use super::{PickerFormat, PickerNeighborhood, PickerSample};
use crate::features::overlay::annotation::{AnnotationEngine, AnnotationTextureCache, AnnotationUiState};
use crate::features::overlay::window_catalog::{WindowInfo, find_window_at};
use crate::flows::output::{OutputDelivery, OutputRequest, OutputState};
use crate::presentation::i18n;
use crate::presentation::render_image;
use minnow_imaging::CapturedFrame;
use minnow_imaging::geometry::{RectF, clamp_point, normalize_rect};

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

#[derive(Clone, Copy, Debug, PartialEq)]
struct OperationSignature {
    selection: Option<RectF>,
    annotation_output: u64,
    pending_edit: Option<Point<Pixels>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct InteractionSignature {
    output_revision: u64,
    picker_cursor: Option<(f64, f64)>,
    picker_format: PickerFormat,
    pending_pointer: Option<Point<Pixels>>,
}

#[derive(Clone, Debug)]
pub(super) enum AnnotationFontState {
    Available,
    Unavailable(minnow_application::OperationError),
}

impl Default for AnnotationFontState {
    fn default() -> Self {
        Self::Unavailable(minnow_application::OperationError::new(
            minnow_application::ErrorKind::Unavailable,
            i18n::overlay::annotation_font_unavailable(),
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct OverlaySession {
    pub(super) viewport: OverlayViewportModel,
    pub(super) captured_frame: Option<CapturedFrame>,
    pub(super) application_session: Option<minnow_application::SessionId>,
    pub(super) operation_revision: u64,
    operation_signature: Option<OperationSignature>,
    pub(super) interaction_revision: u64,
    interaction_signature: Option<InteractionSignature>,
    pub(super) output: OutputState,
    pub(super) output_close_revision: u64,
    pub(super) texture_cache: AnnotationTextureCache,
    pub(super) background_image: Option<Arc<RenderImage>>,
    pub(super) preview_reduced: bool,
    pub(super) background_pixels: Option<Arc<RgbaImage>>,
    pub(super) hovered_window: Option<WindowInfo>,
    pub(super) picker_cursor: Option<(f64, f64)>,
    pub(super) picker_last_pointer: Option<(f64, f64)>,
    pub(super) picker_pointer_lock: Option<(f64, f64)>,
    pub(super) picker_sample: Option<PickerSample>,
    pub(super) picker_neighborhood: Option<PickerNeighborhood>,
    pub(super) picker_format: PickerFormat,
    pub(super) annotation: AnnotationEngine,
    pub(super) annotation_font: AnnotationFontState,
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
    pub font_error: Option<minnow_application::OperationError>,
    pub output_pending: bool,
    pub output_error: Option<minnow_application::OperationError>,
    pub preflight_error: Option<minnow_application::OperationError>,
    pub output_can_retry: bool,
    pub preview_reduced: bool,
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
pub struct OverlayHandle(pub(super) Entity<OverlaySession>, pub(super) Arc<minnow_application::ApplicationServices>);

#[derive(Clone, Copy)]
pub(super) struct OutputIdentity {
    document: minnow_application::SessionId,
    attempt: u64,
    close_revision: u64,
}

impl Global for OverlayHandle {}

impl OverlayHandle {
    pub fn new(cx: &mut App) -> Self {
        Self(cx.new(|_| OverlaySession::default()), crate::application(cx))
    }

    pub(crate) fn prepare(&self, surface: OverlaySurface, cx: &mut App) -> minnow_application::OperationResult<()> {
        let application = self.1.clone();
        self.end_document(cx);
        let session = application.create_capture_session(surface.frame.clone(), 0);
        let font = crate::host(cx).annotation_font();
        self.mutate_session(cx, |state| {
            state.prepare_surface(surface);
            state.application_session = Some(session.id());
            state.set_annotation_font(font);
        })
    }

    pub(crate) fn end_document(&self, cx: &mut App) {
        let id = self.0.update(cx, |session, cx| {
            let id = session.end_document();
            cx.notify();
            id
        });
        if let Some(id) = id {
            self.1.close_session(id);
        }
    }

    pub(in crate::features::overlay) fn document_id(&self, cx: &App) -> Option<minnow_application::SessionId> {
        self.0.read(cx).application_session
    }

    pub(in crate::features::overlay) fn end_document_if_current(&self, id: Option<minnow_application::SessionId>, cx: &mut App) {
        if id.is_some() && self.document_id(cx) == id {
            self.end_document(cx);
        }
    }

    pub(in crate::features::overlay) fn sync_viewport(&self, window: &Window, cx: &mut App) {
        let viewport = window.viewport_size();
        let viewport_w = viewport.width.to_f64();
        let viewport_h = viewport.height.to_f64();
        // Publication failures are already projected into the local error panel.
        let _ = self.mutate_session(cx, |session| session.set_viewport_size(viewport_w, viewport_h));
    }
}

impl OverlaySession {
    pub(super) const MIN_SELECTION_SIZE: f64 = 8.0;

    pub(super) fn show_preflight_error(&mut self, error: minnow_application::OperationError) {
        self.output.show_error(error);
        // The later feedback belongs to the current interaction, even if no
        // image content changed. Earlier outputs no longer own its close right.
        self.interaction_revision = self.interaction_revision.saturating_add(1);
    }

    pub(super) fn set_annotation_font(&mut self, result: minnow_application::OperationResult<ab_glyph::FontArc>) {
        self.annotation_font = match result {
            Ok(font) => {
                self.annotation.set_text_font(Some(font));
                AnnotationFontState::Available
            }
            Err(error) => {
                self.annotation.set_text_font(None);
                AnnotationFontState::Unavailable(error)
            }
        };
    }

    pub(super) fn require_annotation_font(&self) -> minnow_application::OperationResult<()> {
        match &self.annotation_font {
            AnnotationFontState::Available => Ok(()),
            AnnotationFontState::Unavailable(error) => Err(minnow_application::OperationError::new(
                error.kind,
                i18n::overlay::annotation_font_failed(error),
            )),
        }
    }

    /// Ending a document releases every source, raster and texture owned by this projection.
    /// Already submitted immutable action contexts retain their own source independently.
    pub(crate) fn end_document(&mut self) -> Option<minnow_application::SessionId> {
        let id = self.application_session;
        *self = Self::default();
        id
    }

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
        self.preview_reduced = render_image::preview_is_reduced(surface.frame.image().width(), surface.frame.image().height());
        self.background_image = Some(render_image::from_rgba_copy(surface.frame.image().pixels()));
        self.background_pixels = Some(surface.frame.image().shared_pixels());
        self.captured_frame = Some(surface.frame);
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

    pub(super) fn selection_rect(&self) -> Option<minnow_imaging::geometry::Rect> {
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

impl OverlaySession {
    pub(super) fn frame(&mut self) -> OverlayFrame {
        OverlayFrame {
            font_error: match &self.annotation_font {
                AnnotationFontState::Available => None,
                AnnotationFontState::Unavailable(error) => Some(error.clone()),
            },
            output_pending: self.output.is_pending(),
            output_error: self.output.request_error().cloned(),
            preflight_error: self.output.preflight_error().cloned(),
            output_can_retry: self.output.can_retry(),
            preview_reduced: self.preview_reduced,
            background_image: self.background_image.clone(),
            selection: SelectionVm {
                selection: self.viewport.selection,
                target: self.viewport.target,
                drag_mode: self.viewport.mode,
            },
            selection_move_delta: self.viewport.selection_move_delta,
            picker: self.picker_visible().then(|| PickerVm {
                cursor: self.picker_cursor,
                sample: self.picker_sample.clone(),
                neighborhood: self.picker_neighborhood.clone(),
                format: self.picker_format,
            }),
            annotation: self.annotation_ui_state(),
            hud: HudVm {
                hovered_window: self.hovered_window.clone(),
            },
            #[cfg(feature = "overlay-diagnostics")]
            diagnostics: self.diagnostics_snapshot(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OverlaySurface {
    pub frame: CapturedFrame,
    pub windows: Vec<WindowInfo>,
}

impl OverlaySession {
    pub(super) fn begin_output(&mut self, request: &OutputRequest, close_revision: Option<u64>) -> Option<OutputIdentity> {
        let document = self.application_session?;
        let attempt = self.output.begin(request)?;
        let close_revision = close_revision.unwrap_or(self.interaction_revision);
        self.output_close_revision = close_revision;
        Some(OutputIdentity {
            document,
            attempt,
            close_revision,
        })
    }

    pub(super) fn accepts_output(&self, identity: OutputIdentity) -> bool {
        self.application_session == Some(identity.document) && self.output.is_pending_attempt(identity.attempt)
    }

    pub(super) fn output_delivery_may_close(&self, identity: OutputIdentity) -> bool {
        self.accepts_output(identity) && self.output_may_close(identity.close_revision)
    }

    pub(super) fn finish_output(&mut self, identity: OutputIdentity, delivery: &OutputDelivery) -> bool {
        self.accepts_output(identity) && delivery.finish(&mut self.output, identity.attempt)
    }

    pub(super) fn discard_stale_derived_retry(&mut self, services: &minnow_application::ApplicationServices) {
        self.output.discard_stale_derived_retry(|input| services.is_current(input.scope()));
    }

    pub(super) fn retry_output(&mut self, services: &minnow_application::ApplicationServices) -> Option<(OutputRequest, u64)> {
        self.discard_stale_derived_retry(services);
        self.output.retry().map(|request| (request, self.output_close_revision))
    }

    pub(super) fn output_may_close(&self, granted_interaction: u64) -> bool {
        self.captured_frame.is_some() && self.interaction_revision == granted_interaction
    }

    pub(crate) fn sync_operation_revision(&mut self) -> u64 {
        let editing = self.viewport.mode != DragMode::Idle || self.has_active_annotation_interaction();
        let signature = OperationSignature {
            selection: self.viewport.selection,
            annotation_output: self.annotation.output_revision(),
            pending_edit: self.viewport.pending_pointer.filter(|_| editing),
        };
        if self.operation_signature != Some(signature) {
            self.operation_revision = self.operation_revision.saturating_add(1);
            self.operation_signature = Some(signature);
        }
        let interaction = InteractionSignature {
            output_revision: self.operation_revision,
            picker_cursor: self.picker_cursor,
            picker_format: self.picker_format,
            pending_pointer: self.viewport.pending_pointer,
        };
        if self.interaction_signature != Some(interaction) {
            self.interaction_revision = self.interaction_revision.saturating_add(1);
            self.interaction_signature = Some(interaction);
        }
        self.operation_revision
    }

    pub(crate) fn capture_scale(&self) -> f64 {
        self.captured_frame.as_ref().map_or(1.0, |frame| frame.geometry().scale_factor)
    }
}

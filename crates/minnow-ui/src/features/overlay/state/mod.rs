mod annotation;
mod command;
mod diagnostics;
mod effects;
mod frame;
mod picker;
mod selection;
mod surface;

use gpui::{App, AppContext, Entity, Global, Pixels, Point, RenderImage, Window};
use image::RgbaImage;
use std::sync::Arc;

use crate::features::overlay::annotation::AnnotationEngine;
use crate::features::overlay::window_catalog::{WindowInfo, find_window_at};
use minnow_core::capture::action::{ActionContext, CaptureAction};
use minnow_core::geometry::{RectF, clamp_point, normalize_rect};
use minnow_core::notify::NotificationType;

pub(crate) use crate::features::overlay::annotation::{
    AnnotationKind, AnnotationKindTag, AnnotationLayerState, AnnotationSelectionInfo, AnnotationStyleState, AnnotationTool, AnnotationUiState,
    MosaicMode,
};
#[cfg(feature = "overlay-diagnostics")]
use diagnostics::OverlayDiagnostics;
#[cfg(feature = "overlay-diagnostics")]
pub(crate) use diagnostics::OverlayDiagnosticsSnapshot;
pub(crate) use picker::{PickerFormat, PickerNeighborhood, PickerSample};
pub(crate) use surface::OverlaySurface;

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
struct OverlayViewportModel {
    mode: DragMode,
    selection: Option<RectF>,
    target: Option<RectF>,
    drag_start: Option<Point<Pixels>>,
    drag_start_rect: Option<RectF>,
    /// During selection moves, `selection` is updated live while dragging.
    /// `selection_move_origin` captures the selection rect at drag start, and
    /// `selection_move_delta` is the current (dx, dy) relative to that origin.
    selection_move_delta: Option<(f64, f64)>,
    selection_move_origin: Option<RectF>,
    confirm_target_on_release: bool,
    viewport_w: f64,
    viewport_h: f64,
    pending_pointer: Option<Point<Pixels>>,
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
    viewport: OverlayViewportModel,
    background_image: Option<Arc<RenderImage>>,
    pub(super) background_pixels: Option<Arc<RgbaImage>>,
    hovered_window: Option<WindowInfo>,
    pub(super) picker_cursor: Option<(f64, f64)>,
    pub(super) picker_last_pointer: Option<(f64, f64)>,
    pub(super) picker_pointer_lock: Option<(f64, f64)>,
    pub(super) picker_sample: Option<PickerSample>,
    pub(super) picker_neighborhood: Option<PickerNeighborhood>,
    pub(super) picker_format: PickerFormat,
    pub(super) annotation: AnnotationEngine,
    #[cfg(feature = "overlay-diagnostics")]
    diagnostics: OverlayDiagnostics,
    windows: Vec<WindowInfo>,
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
    #[allow(dead_code)]
    pub selection_move_delta: Option<(f64, f64)>,
    pub picker: Option<PickerVm>,
    pub annotation: AnnotationUiState,
    pub hud: HudVm,
    #[cfg(feature = "overlay-diagnostics")]
    pub diagnostics: OverlayDiagnosticsSnapshot,
}

#[derive(Clone)]
pub struct OverlayHandle(Entity<OverlaySession>);

impl Global for OverlayHandle {}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CaptureCommand {
    Execute(CaptureAction),
    SaveWithPath(String),
    CopyPickerColor,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PickerCommand {
    CycleFormat,
    MoveByPixel { delta_x: i32, delta_y: i32 },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AnnotationCommand {
    SetTool(AnnotationTool),
    StartDraw(Point<Pixels>),
    StartMove { id: u64, point: Point<Pixels> },
    Select(Option<u64>),
    DeleteIntent,
    Undo,
    Redo,
    CycleColor,
    SetColor { color: u32 },
    ToggleFill,
    AdjustStroke { delta: f64 },
    SetMosaicMode(MosaicMode),
    AdjustMosaicIntensity { delta: f64 },
    AdjustByWheel { point: Point<Pixels>, delta: f64 },
    StartTextEdit,
    StartTextEditAtPoint(Point<Pixels>),
    AppendText { text: String },
    InsertTextNewline,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LifecycleCommand {
    StartSelection(Point<Pixels>),
    StartMove(Point<Pixels>),
    StartResize { corner: ResizeCorner, point: Point<Pixels> },
    PointerMoved(Point<Pixels>),
    PointerReleased,
    ClearSelection,
    CloseIntent,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum OverlayCommand {
    Capture(CaptureCommand),
    Picker(PickerCommand),
    Annotation(AnnotationCommand),
    Lifecycle(LifecycleCommand),
}

pub(crate) enum OverlayEffect {
    Refresh,
    Close,
    StartLongCapture {
        selection_rect: minnow_core::geometry::Rect,
        viewport_rect: RectF,
        viewport_scale: f64,
    },
    Capture {
        action: CaptureAction,
        context: ActionContext,
    },
    CopyText {
        text: String,
        title: String,
        message: String,
        notification_type: NotificationType,
        close_on_success: bool,
    },
}

#[derive(Default)]
pub(crate) struct OverlayOutcome {
    effects: Vec<OverlayEffect>,
}

enum SessionTransition {
    NoOp,
    Refresh,
    Effect(OverlayEffect),
}

impl SessionTransition {
    fn from_changed(changed: bool) -> Self {
        if changed { Self::Refresh } else { Self::NoOp }
    }

    fn into_outcome(self) -> OverlayOutcome {
        match self {
            Self::NoOp => OverlayOutcome::default(),
            Self::Refresh => OverlayOutcome::refresh(),
            Self::Effect(effect) => OverlayOutcome::with_effect(effect),
        }
    }
}

impl OverlayOutcome {
    fn push(&mut self, effect: OverlayEffect) {
        self.effects.push(effect);
    }

    fn refresh() -> Self {
        let mut outcome = Self::default();
        outcome.push(OverlayEffect::Refresh);
        outcome
    }

    fn with_effect(effect: OverlayEffect) -> Self {
        let mut outcome = Self::default();
        outcome.push(effect);
        outcome
    }
}

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

    fn sync_viewport(&self, window: &Window, cx: &mut App) {
        let viewport = window.viewport_size();
        let viewport_w = viewport.width.to_f64();
        let viewport_h = viewport.height.to_f64();
        self.0.update(cx, |session, _| session.set_viewport_size(viewport_w, viewport_h));
    }
}

impl OverlaySession {
    const MIN_SELECTION_SIZE: f64 = 8.0;

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

    fn reset_interaction_state(&mut self) {
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

    fn selection_rect(&self) -> Option<minnow_core::geometry::Rect> {
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

    fn clamp_rect_to_viewport(&self, rect: RectF) -> RectF {
        let max_w = self.viewport.viewport_w.max(0.0);
        let max_h = self.viewport.viewport_h.max(0.0);
        let x = rect.x.clamp(0.0, max_w);
        let y = rect.y.clamp(0.0, max_h);
        let width = rect.width.max(0.0).min((max_w - x).max(0.0));
        let height = rect.height.max(0.0).min((max_h - y).max(0.0));
        RectF::new(x, y, width, height)
    }

    fn diag_on_render(&mut self) {
        #[cfg(feature = "overlay-diagnostics")]
        {
            self.diagnostics.on_render();
        }
    }

    fn diag_on_refresh(&mut self) {
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
    fn diagnostics_snapshot(&mut self) -> OverlayDiagnosticsSnapshot {
        self.diagnostics.snapshot(self.annotation.raster_diagnostics())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::overlay::interaction::resolve_mouse_down_command;
    use minnow_core::capture::source::PREVIEW_SOURCE;

    fn window(title: &str, app_name: &str, x: i32, y: i32, width: u32, height: u32) -> WindowInfo {
        WindowInfo {
            title: title.into(),
            app_name: app_name.into(),
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn hover_tracks_window_entry_and_exit() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(400.0, 300.0);
        session.windows = vec![window("MinnowSnap", "MinnowSnap", 10, 10, 100, 100)];

        assert!(session.update_hover(Point::new(gpui::px(50.0), gpui::px(50.0))));
        assert_eq!(session.viewport.target, Some(RectF::new(10.0, 10.0, 100.0, 100.0)));
        assert!(session.hovered_window.is_some());

        assert!(session.update_hover(Point::new(gpui::px(250.0), gpui::px(250.0))));
        assert_eq!(session.viewport.target, None);
        assert!(session.hovered_window.is_none());
    }

    #[test]
    fn capture_requires_surface_and_selection() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(100.0, 100.0);

        let outcome = session.apply(OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Copy)));
        assert!(outcome.effects.is_empty());

        session.prepare_surface(OverlaySurface {
            background_pixels: Some(Arc::new(image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255])))),
            ..OverlaySurface::default()
        });
        let outcome = session.apply(OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Copy)));
        assert!(outcome.effects.is_empty());
    }

    #[test]
    fn capture_uses_preview_source_without_annotations() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(200.0, 120.0);
        session.prepare_surface(OverlaySurface {
            background_pixels: Some(Arc::new(image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255])))),
            ..OverlaySurface::default()
        });
        session.viewport.selection = Some(RectF::new(10.0, 10.0, 80.0, 40.0));

        let outcome = session.apply(OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Copy)));
        assert_eq!(outcome.effects.len(), 1);
        let OverlayEffect::Capture { context, .. } = &outcome.effects[0] else {
            panic!("expected capture effect");
        };
        assert_eq!(context.path, PREVIEW_SOURCE);
    }

    #[test]
    fn save_with_path_command_sets_capture_context_override() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(200.0, 120.0);
        session.prepare_surface(OverlaySurface {
            background_pixels: Some(Arc::new(image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255])))),
            ..OverlaySurface::default()
        });
        session.viewport.selection = Some(RectF::new(10.0, 10.0, 80.0, 40.0));

        let save_path = "D:/captures/overlay".to_string();
        let outcome = session.apply(OverlayCommand::Capture(CaptureCommand::SaveWithPath(save_path.clone())));
        assert_eq!(outcome.effects.len(), 1);
        let OverlayEffect::Capture { action, context } = &outcome.effects[0] else {
            panic!("expected capture effect");
        };
        assert_eq!(*action, CaptureAction::Save);
        assert_eq!(context.save_path_override, Some(save_path));
    }

    #[test]
    fn clear_resets_selection_and_drag_state() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(100.0, 100.0);
        session.viewport.mode = DragMode::Selecting;
        session.viewport.selection = Some(RectF::new(10.0, 10.0, 30.0, 20.0));
        session.clear();

        assert_eq!(session.mode(), DragMode::Idle);
        assert_eq!(session.selection(), None);
        assert_eq!(session.viewport.target, None);
    }

    #[test]
    fn delete_intent_backspaces_during_text_editing() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(300.0, 200.0);
        session.viewport.selection = Some(RectF::new(20.0, 20.0, 160.0, 100.0));
        session.set_annotation_tool(AnnotationTool::Text);
        assert!(session.start_annotation_draw(Point::new(gpui::px(40.0), gpui::px(60.0))));
        let selected = session.selected_annotation_item().unwrap();
        let AnnotationKind::Text { text, .. } = selected.kind else {
            panic!("expected text item");
        };
        assert_eq!(text, "Text");

        let outcome = session.apply(OverlayCommand::Annotation(AnnotationCommand::DeleteIntent));
        assert_eq!(outcome.effects.len(), 1);
        assert!(session.commit_text_edit());
        let selected = session.selected_annotation_item().unwrap();
        let AnnotationKind::Text { text, .. } = selected.kind else {
            panic!("expected text item");
        };
        assert_eq!(text, "Tex");
    }

    #[test]
    fn close_intent_cancels_text_edit_before_closing_overlay() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(300.0, 200.0);
        session.viewport.selection = Some(RectF::new(20.0, 20.0, 160.0, 100.0));
        session.set_annotation_tool(AnnotationTool::Text);
        assert!(session.start_annotation_draw(Point::new(gpui::px(40.0), gpui::px(60.0))));
        assert!(session.text_editing_id().is_some());

        let cancel_outcome = session.apply(OverlayCommand::Lifecycle(LifecycleCommand::CloseIntent));
        assert_eq!(cancel_outcome.effects.len(), 1);
        assert!(matches!(cancel_outcome.effects[0], OverlayEffect::Refresh));
        assert!(session.text_editing_id().is_none());

        let close_outcome = session.apply(OverlayCommand::Lifecycle(LifecycleCommand::CloseIntent));
        assert_eq!(close_outcome.effects.len(), 1);
        assert!(matches!(close_outcome.effects[0], OverlayEffect::Close));
    }

    #[test]
    fn set_color_command_routes_to_annotation_engine() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(300.0, 200.0);
        session.viewport.selection = Some(RectF::new(20.0, 20.0, 160.0, 100.0));
        session.set_annotation_tool(AnnotationTool::Rectangle);
        assert!(session.start_annotation_draw(Point::new(gpui::px(40.0), gpui::px(60.0))));
        assert!(session.update_annotation_interaction(Point::new(gpui::px(120.0), gpui::px(100.0))));
        assert!(session.finish_annotation_interaction());

        let first = session.apply(OverlayCommand::Annotation(AnnotationCommand::SetColor { color: 0x11aa55ff }));
        assert_eq!(first.effects.len(), 1);
        let second = session.apply(OverlayCommand::Annotation(AnnotationCommand::SetColor { color: 0x11aa55ff }));
        assert!(second.effects.is_empty());
    }

    #[test]
    fn set_tool_command_toggles_when_reselected() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(300.0, 200.0);
        session.viewport.selection = Some(RectF::new(20.0, 20.0, 160.0, 100.0));

        let first = session.apply(OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Rectangle)));
        assert_eq!(first.effects.len(), 1);
        assert_eq!(session.annotation_ui_state().tool, Some(AnnotationTool::Rectangle));

        let second = session.apply(OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Rectangle)));
        assert_eq!(second.effects.len(), 1);
        assert_eq!(session.annotation_ui_state().tool, None);

        let third = session.apply(OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Circle)));
        assert_eq!(third.effects.len(), 1);
        assert_eq!(session.annotation_ui_state().tool, Some(AnnotationTool::Circle));
    }

    #[test]
    fn selection_move_preview_offsets_annotation_layer() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(300.0, 200.0);
        session.viewport.selection = Some(RectF::new(20.0, 20.0, 160.0, 100.0));

        session.set_annotation_tool(AnnotationTool::Rectangle);
        assert!(session.start_annotation_draw(Point::new(gpui::px(60.0), gpui::px(60.0))));
        assert!(session.update_annotation_interaction(Point::new(gpui::px(100.0), gpui::px(100.0))));
        assert!(session.finish_annotation_interaction());

        let annotation_id = session.selected_annotation_item().unwrap().id;

        let frame_before = session.frame();
        let before_outline = frame_before
            .annotation
            .layer
            .outlines
            .iter()
            .find(|outline| outline.id == annotation_id)
            .unwrap();

        let dx: f64 = 50.0;
        let dy: f64 = 40.0;
        session.start_move(Point::new(gpui::px(60.0), gpui::px(60.0)));
        session.update_move(Point::new(
            gpui::px((60.0 + dx) as f32),
            gpui::px((60.0 + dy) as f32),
        ));

        // Selection-move preview is expected to offset the rendered annotation layer while dragging.
        let frame_after = session.frame();
        let after_outline = frame_after
            .annotation
            .layer
            .outlines
            .iter()
            .find(|outline| outline.id == annotation_id)
            .unwrap();

        assert_eq!(after_outline.bounds.x, before_outline.bounds.x + dx);
        assert_eq!(after_outline.bounds.y, before_outline.bounds.y + dy);
        assert_eq!(after_outline.bounds.width, before_outline.bounds.width);
        assert_eq!(after_outline.bounds.height, before_outline.bounds.height);
    }

    #[test]
    fn no_tool_disables_draw_but_still_allows_moving_existing_annotation() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(300.0, 200.0);
        session.viewport.selection = Some(RectF::new(20.0, 20.0, 160.0, 100.0));

        assert!(!session.start_annotation_draw(Point::new(gpui::px(40.0), gpui::px(60.0))));

        session.set_annotation_tool(AnnotationTool::Rectangle);
        assert!(session.start_annotation_draw(Point::new(gpui::px(40.0), gpui::px(60.0))));
        assert!(session.update_annotation_interaction(Point::new(gpui::px(120.0), gpui::px(100.0))));
        assert!(session.finish_annotation_interaction());

        let selected = session.selected_annotation_item().unwrap();
        let id = selected.id;
        let before_bounds = selected.bounds();

        session.set_annotation_tool(AnnotationTool::Rectangle);
        assert_eq!(session.annotation_ui_state().tool, None);

        assert!(session.start_annotation_move(id, Point::new(gpui::px(60.0), gpui::px(70.0))));
        assert!(session.update_annotation_interaction(Point::new(gpui::px(100.0), gpui::px(95.0))));
        assert!(session.finish_annotation_interaction());

        let after_bounds = session.selected_annotation_item().unwrap().bounds();
        assert_ne!(before_bounds, after_bounds);
    }

    #[test]
    fn left_click_inside_selection_without_tool_starts_move() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(300.0, 200.0);
        session.viewport.selection = Some(RectF::new(20.0, 20.0, 160.0, 100.0));

        let command = resolve_mouse_down_command(&session, gpui::MouseButton::Left, Point::new(gpui::px(80.0), gpui::px(70.0)), 1);
        assert!(matches!(
            command,
            Some(OverlayCommand::Lifecycle(LifecycleCommand::StartMove(_)))
        ));

        session.set_annotation_tool(AnnotationTool::Rectangle);
        let command = resolve_mouse_down_command(&session, gpui::MouseButton::Left, Point::new(gpui::px(80.0), gpui::px(70.0)), 1);
        assert!(matches!(command, Some(OverlayCommand::Annotation(AnnotationCommand::StartDraw(_)))));
    }

    #[test]
    fn drag_pointer_moves_coalesce_to_latest_pending_point() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(300.0, 200.0);
        session.apply(OverlayCommand::Lifecycle(LifecycleCommand::StartSelection(Point::new(gpui::px(40.0), gpui::px(40.0)))));

        let first_point = Point::new(gpui::px(90.0), gpui::px(90.0));
        let second_point = Point::new(gpui::px(95.0), gpui::px(110.0));

        let first = session.apply(OverlayCommand::Lifecycle(LifecycleCommand::PointerMoved(first_point)));
        let second = session.apply(OverlayCommand::Lifecycle(LifecycleCommand::PointerMoved(second_point)));

        // Pointer-move events during drag should coalesce: refresh once, but keep the latest pending point.
        // This test should fail until drag-pointer coalescing is implemented for drag modes.
        assert!(first.effects.iter().any(|effect| matches!(effect, OverlayEffect::Refresh)));
        assert!(second.effects.is_empty(), "expected coalesced pointer move to avoid refresh");
        assert_eq!(session.viewport.pending_pointer, Some(second_point));
    }

    #[test]
    fn pointer_release_applies_latest_pending_drag_point_before_finishing_selection() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(300.0, 200.0);
        session.apply(OverlayCommand::Lifecycle(LifecycleCommand::StartSelection(Point::new(gpui::px(40.0), gpui::px(40.0)))));

        let final_point = Point::new(gpui::px(150.0), gpui::px(120.0));
        session.apply(OverlayCommand::Lifecycle(LifecycleCommand::PointerMoved(final_point)));
        let outcome = session.apply(OverlayCommand::Lifecycle(LifecycleCommand::PointerReleased));

        assert!(outcome.effects.iter().any(|effect| matches!(effect, OverlayEffect::Refresh)));
        assert_eq!(session.selection(), Some(RectF::new(40.0, 40.0, 110.0, 80.0)));
        assert_eq!(session.mode(), DragMode::Idle);
    }

    #[test]
    fn no_op_annotation_command_does_not_refresh() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(300.0, 200.0);
        session.viewport.selection = Some(RectF::new(20.0, 20.0, 160.0, 100.0));

        let outcome = session.apply(OverlayCommand::Annotation(AnnotationCommand::SetMosaicMode(MosaicMode::Pixelate)));
        assert!(outcome.effects.is_empty());
    }

    #[test]
    fn scroll_capture_dispatches_long_capture_effect() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(600.0, 400.0);
        session.viewport.selection = Some(RectF::new(40.0, 40.0, 320.0, 220.0));

        let outcome = session.apply(OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Scroll)));
        assert_eq!(outcome.effects.len(), 1);
        assert!(matches!(outcome.effects[0], OverlayEffect::StartLongCapture { .. }));
    }

    #[test]
    fn scroll_capture_effect_ignores_overlay_background_image() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(600.0, 400.0);
        session.viewport.selection = Some(RectF::new(40.0, 40.0, 320.0, 220.0));

        let bg = Arc::new(RenderImage::new([image::Frame::new(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([1, 2, 3, 255]),
        ))]));
        session.background_image = Some(bg.clone());

        let outcome = session.apply(OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Scroll)));
        assert_eq!(outcome.effects.len(), 1);

        let OverlayEffect::StartLongCapture {
            selection_rect,
            viewport_rect,
            viewport_scale,
        } = &outcome.effects[0]
        else {
            panic!("expected StartLongCapture effect");
        };
        assert!(selection_rect.has_area());
        assert!(viewport_rect.width > 0.0);
        assert!(viewport_rect.height > 0.0);
        assert!(*viewport_scale >= 1.0);
        assert!(session.background_image.is_some());
    }
}

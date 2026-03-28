mod diagnostics;
mod picker;
mod selection;
mod surface;

use gpui::{App, AppContext, Entity, Global, Pixels, Point, RenderImage, Window};
use image::RgbaImage;
use std::sync::Arc;

use crate::core::capture::action::{ActionContext, ActionResult, CaptureAction};
use crate::core::geometry::{RectF, clamp_point, normalize_rect};
use crate::core::i18n;
use crate::core::io::clipboard::copy_text_to_clipboard;
use crate::core::notify::NotificationType;
use crate::core::window::{WindowInfo, find_window_at};
use crate::ui::pin::{self, PinRequest};

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
    Moving,
    Resizing(ResizeCorner),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct OverlayViewportModel {
    mode: DragMode,
    selection: Option<RectF>,
    target: Option<RectF>,
    drag_start: Option<Point<Pixels>>,
    drag_start_rect: Option<RectF>,
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
    background_path: Option<String>,
    hovered_window: Option<WindowInfo>,
    pub(super) picker_cursor: Option<(f64, f64)>,
    pub(super) picker_last_pointer: Option<(f64, f64)>,
    pub(super) picker_pointer_lock: Option<(f64, f64)>,
    pub(super) picker_sample: Option<PickerSample>,
    pub(super) picker_neighborhood: Option<PickerNeighborhood>,
    pub(super) picker_format: PickerFormat,
    #[cfg(feature = "overlay-diagnostics")]
    diagnostics: OverlayDiagnostics,
    windows: Vec<WindowInfo>,
}

#[derive(Clone, Debug)]
pub(crate) struct OverlayPickerFrame {
    pub cursor: Option<(f64, f64)>,
    pub sample: Option<PickerSample>,
    pub neighborhood: Option<PickerNeighborhood>,
    pub format: PickerFormat,
}

#[derive(Clone)]
pub(crate) struct OverlayFrame {
    pub background_image: Option<Arc<RenderImage>>,
    pub selection: Option<RectF>,
    pub target: Option<RectF>,
    pub hovered_window: Option<WindowInfo>,
    pub drag_mode: DragMode,
    pub picker: Option<OverlayPickerFrame>,
    #[cfg(feature = "overlay-diagnostics")]
    pub diagnostics: OverlayDiagnosticsSnapshot,
}

#[derive(Clone)]
pub struct OverlayHandle(Entity<OverlaySession>);

impl Global for OverlayHandle {}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum OverlayCommand {
    Capture(CaptureAction),
    CopyPickerColor,
    CyclePickerFormat,
    MovePickerByPixel { delta_x: i32, delta_y: i32 },
    StartSelection(Point<Pixels>),
    StartMove(Point<Pixels>),
    StartResize { corner: ResizeCorner, point: Point<Pixels> },
    PointerMoved(Point<Pixels>),
    PointerReleased,
    ClearSelection,
    Close,
}

pub(crate) enum OverlayEffect {
    Refresh,
    Close,
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

    pub(crate) fn dispatch(&self, command: OverlayCommand, window: &mut Window, cx: &mut App) {
        self.sync_viewport(window, cx);
        let outcome = self.0.update(cx, |session, _| session.apply(command));
        self.run_outcome(outcome, window, cx);
    }

    pub(crate) fn prepare_frame(&self, window: &Window, cx: &mut App) -> OverlayFrame {
        self.sync_viewport(window, cx);
        self.0.update(cx, |session, _| {
            let _ = session.apply_pending_pointer();
            session.diag_on_render();
            session.frame()
        })
    }

    fn sync_viewport(&self, window: &Window, cx: &mut App) {
        let viewport = window.viewport_size();
        let viewport_w = viewport.width.to_f64();
        let viewport_h = viewport.height.to_f64();
        self.0.update(cx, |session, _| session.set_viewport_size(viewport_w, viewport_h));
    }

    fn run_outcome(&self, outcome: OverlayOutcome, window: &mut Window, cx: &mut App) {
        for effect in outcome.effects {
            self.run_effect(effect, window, cx);
        }
    }

    fn run_effect(&self, effect: OverlayEffect, window: &mut Window, cx: &mut App) {
        match effect {
            OverlayEffect::Refresh => {
                self.0.update(cx, |session, _| session.diag_on_refresh());
                window.refresh();
            }
            OverlayEffect::Close => {
                self.0.update(cx, |session, _| session.clear());
                window.defer(cx, |window, _| {
                    window.remove_window();
                });
            }
            OverlayEffect::CopyText {
                text,
                title,
                message,
                notification_type,
                close_on_success,
            } => {
                if copy_text_to_clipboard(text) {
                    crate::core::notify::show(&title, &message, notification_type);
                    if close_on_success {
                        self.run_effect(OverlayEffect::Close, window, cx);
                    }
                } else {
                    self.run_effect(OverlayEffect::Refresh, window, cx);
                }
            }
            OverlayEffect::Capture { action, context } => match action.execute(context) {
                ActionResult::Copied => {
                    crate::core::notify::show(
                        i18n::app::capture_name().as_str(),
                        i18n::notify::copied_image().as_str(),
                        NotificationType::Copy,
                    );
                    self.run_effect(OverlayEffect::Close, window, cx);
                }
                ActionResult::ColorPicked(color) => {
                    self.run_effect(
                        OverlayEffect::CopyText {
                            text: color.clone(),
                            title: i18n::app::capture_name(),
                            message: format!("Color copied: {color}"),
                            notification_type: NotificationType::Copy,
                            close_on_success: true,
                        },
                        window,
                        cx,
                    );
                }
                ActionResult::Saved(path) => {
                    crate::core::notify::show(
                        i18n::app::capture_name().as_str(),
                        i18n::notify::saved_image(path).as_str(),
                        NotificationType::Save,
                    );
                    self.run_effect(OverlayEffect::Close, window, cx);
                }
                ActionResult::PinRequested(path, source_bounds, _auto_ocr) => {
                    let request = PinRequest::new(path, Some(source_bounds));
                    cx.defer(move |cx| {
                        pin::open_window(cx, request);
                    });
                    self.run_effect(OverlayEffect::Close, window, cx);
                }
                ActionResult::OcrResult(content) => {
                    self.run_effect(
                        OverlayEffect::CopyText {
                            text: content,
                            title: i18n::app::capture_name(),
                            message: i18n::notify::copied_qr(),
                            notification_type: NotificationType::Copy,
                            close_on_success: true,
                        },
                        window,
                        cx,
                    );
                }
                ActionResult::NoOp => {
                    if matches!(action, CaptureAction::QrCode) {
                        crate::core::notify::show(i18n::app::name().as_str(), i18n::overlay::qr_not_found().as_str(), NotificationType::Info);
                    }
                    self.run_effect(OverlayEffect::Refresh, window, cx);
                }
                ActionResult::Error(err) => {
                    tracing::error!("Action error: {err}");
                    if matches!(action, CaptureAction::QrCode) {
                        crate::core::notify::show(i18n::app::name().as_str(), i18n::overlay::qr_not_found().as_str(), NotificationType::Info);
                    }
                    self.run_effect(OverlayEffect::Refresh, window, cx);
                }
            },
        }
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

    fn frame(&mut self) -> OverlayFrame {
        OverlayFrame {
            background_image: self.background_image.clone(),
            selection: self.viewport.selection,
            target: self.viewport.target,
            hovered_window: self.hovered_window.clone(),
            drag_mode: self.viewport.mode,
            picker: self.picker_visible().then(|| OverlayPickerFrame {
                cursor: self.picker_cursor,
                sample: self.picker_sample.clone(),
                neighborhood: self.picker_neighborhood.clone(),
                format: self.picker_format,
            }),
            #[cfg(feature = "overlay-diagnostics")]
            diagnostics: self.diagnostics_snapshot(),
        }
    }

    pub(crate) fn prepare_surface(&mut self, surface: OverlaySurface) {
        self.reset_interaction_state();
        self.background_image = surface.background_image;
        self.background_pixels = surface.background_pixels;
        self.background_path = surface.background_path;
        self.windows = surface.windows;
        self.picker_cursor = None;
        self.picker_sample = None;
        self.picker_neighborhood = None;
        self.picker_format = PickerFormat::Hex;
        self.refresh_picker_sample();
    }

    fn apply(&mut self, command: OverlayCommand) -> OverlayOutcome {
        match command {
            OverlayCommand::Capture(action) => self.capture_effect(action).map(OverlayOutcome::with_effect).unwrap_or_default(),
            OverlayCommand::CopyPickerColor => self
                .picker_text()
                .map(|text| {
                    OverlayOutcome::with_effect(OverlayEffect::CopyText {
                        message: i18n::notify::copied_qr(),
                        text,
                        title: i18n::app::capture_name(),
                        notification_type: NotificationType::Copy,
                        close_on_success: true,
                    })
                })
                .unwrap_or_default(),
            OverlayCommand::CyclePickerFormat => {
                if self.cycle_picker_format() {
                    OverlayOutcome::refresh()
                } else {
                    OverlayOutcome::default()
                }
            }
            OverlayCommand::MovePickerByPixel { delta_x, delta_y } => {
                if self.move_picker_by_pixel(delta_x, delta_y) {
                    OverlayOutcome::refresh()
                } else {
                    OverlayOutcome::default()
                }
            }
            OverlayCommand::StartSelection(point) => {
                self.start_selection(point);
                OverlayOutcome::refresh()
            }
            OverlayCommand::StartMove(point) => {
                self.start_move(point);
                OverlayOutcome::refresh()
            }
            OverlayCommand::StartResize { corner, point } => {
                self.start_resize(corner, point);
                OverlayOutcome::refresh()
            }
            OverlayCommand::PointerMoved(point) => {
                let queued_first = self.queue_pointer(point);
                let refresh = match self.mode() {
                    DragMode::Idle => queued_first,
                    DragMode::Selecting | DragMode::Moving | DragMode::Resizing(_) => self.apply_pending_pointer(),
                };
                if refresh { OverlayOutcome::refresh() } else { OverlayOutcome::default() }
            }
            OverlayCommand::PointerReleased => {
                match self.mode() {
                    DragMode::Selecting => self.finish_selection(),
                    DragMode::Moving => self.finish_move(),
                    DragMode::Resizing(_) => self.finish_resize(),
                    DragMode::Idle => {}
                }
                OverlayOutcome::refresh()
            }
            OverlayCommand::ClearSelection => {
                self.clear();
                OverlayOutcome::refresh()
            }
            OverlayCommand::Close => OverlayOutcome::with_effect(OverlayEffect::Close),
        }
    }

    fn capture_effect(&self, action: CaptureAction) -> Option<OverlayEffect> {
        let background_path = self.background_path.as_deref()?;
        let selection = self.selection_rect()?;
        Some(OverlayEffect::Capture {
            action,
            context: ActionContext::crop_selection(background_path.to_string(), selection),
        })
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
        self.viewport.pending_pointer = None;
        self.viewport.confirm_target_on_release = false;
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
            DragMode::Moving => {
                self.update_move(point);
                refresh = true;
            }
            DragMode::Resizing(_) => {
                self.update_resize(point);
                refresh = true;
            }
            DragMode::Idle => {
                refresh = self.update_pointer(point) || refresh;
                refresh = self.update_hover(point) || refresh;
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

    fn selection_rect(&self) -> Option<crate::core::geometry::Rect> {
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
        self.diagnostics.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let outcome = session.apply(OverlayCommand::Capture(CaptureAction::Copy));
        assert!(outcome.effects.is_empty());

        session.prepare_surface(OverlaySurface {
            background_path: Some("fake.png".into()),
            ..OverlaySurface::default()
        });
        let outcome = session.apply(OverlayCommand::Capture(CaptureAction::Copy));
        assert!(outcome.effects.is_empty());
    }

    #[test]
    fn clear_resets_selection_and_drag_state() {
        let mut session = OverlaySession::default();
        session.set_viewport_size(100.0, 100.0);
        session.viewport.mode = DragMode::Moving;
        session.viewport.selection = Some(RectF::new(10.0, 10.0, 30.0, 20.0));
        session.clear();

        assert_eq!(session.mode(), DragMode::Idle);
        assert_eq!(session.selection(), None);
        assert_eq!(session.viewport.target, None);
    }
}

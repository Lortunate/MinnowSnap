#[cxx_qt::bridge]
pub mod qobject {
    #[qml_element]
    qnamespace!("CaptureActions");

    #[qenum]
    #[namespace = "CaptureActions"]
    pub enum UiAction {
        Unknown,
        Copy,
        Save,
        Pin,
        Ocr,
        Scroll,
        QrCode,
        Undo,
        Redo,
        Cancel,
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qrectf.h");
        type QRectF = cxx_qt_lib::QRectF;
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, is_capturing)]
        #[qproperty(i32, pin_count)]
        #[qproperty(QRectF, capture_screen_rect)]
        #[qproperty(f64, capture_screen_scale)]
        type ScreenCapture = super::ScreenCaptureRust;

        #[qinvokable]
        fn prepare_capture(self: Pin<&mut Self>);

        #[qinvokable]
        fn quick_capture(self: Pin<&mut Self>, selection_rect: QRectF);

        #[qinvokable]
        fn start_scroll_capture(self: Pin<&mut Self>, selection_rect: QRectF);

        #[qinvokable]
        fn stop_scroll_capture(self: Pin<&mut Self>);

        #[qinvokable]
        fn copy_text(self: Pin<&mut Self>, text: QString);

        #[qinvokable]
        fn copy_qrcode_result(self: Pin<&mut Self>, text: QString);

        #[qinvokable]
        fn register_hotkeys(self: Pin<&mut Self>);

        #[qinvokable]
        fn set_capture_shortcut(self: Pin<&mut Self>, shortcut: QString);

        #[qinvokable]
        fn set_quick_capture_shortcut(self: Pin<&mut Self>, shortcut: QString);

        #[qinvokable]
        fn generate_temp_path(self: Pin<&mut Self>, extension: QString) -> QString;

        #[qinvokable]
        fn get_pixel_color(self: Pin<&mut Self>, x: i32, y: i32, scale: f64) -> QString;

        #[qinvokable]
        fn set_cursor_position(self: Pin<&mut Self>, x: i32, y: i32, scale: f64);

        #[qinvokable]
        fn emit_close_all_pins(self: Pin<&mut Self>);

        #[qinvokable]
        fn increment_pin_count(self: Pin<&mut Self>);

        #[qinvokable]
        fn decrement_pin_count(self: Pin<&mut Self>);

        #[qinvokable]
        fn request_action(self: Pin<&mut Self>, path: QUrl, action: i32, selection_rect: QRectF, has_annotations: bool);

        #[qinvokable]
        fn request_scroll_action(self: Pin<&mut Self>, action: i32);

        #[qinvokable]
        fn cancel_scroll_capture(self: Pin<&mut Self>);

        #[qinvokable]
        fn submit_capture(self: Pin<&mut Self>, path: QUrl, action: i32, selection_rect: QRectF);

        #[qinvokable]
        fn submit_composited_capture(self: Pin<&mut Self>, path: QUrl, action: i32, selection_rect: QRectF);

        #[qinvokable]
        fn release_capture_buffers(self: Pin<&mut Self>);

        #[qinvokable]
        fn collect_memory(self: Pin<&mut Self>);

        #[qsignal]
        fn screen_capture_shortcut_triggered(self: Pin<&mut Self>);

        #[qsignal]
        fn quick_capture_shortcut_triggered(self: Pin<&mut Self>);

        #[qsignal]
        fn screenshot_captured(self: Pin<&mut Self>, path: QString);

        #[qsignal]
        fn window_info_ready(self: Pin<&mut Self>, json: QString);

        #[qsignal]
        fn capture_ready(self: Pin<&mut Self>);

        #[qsignal]
        fn scroll_capture_finished(self: Pin<&mut Self>, path: QString);

        #[qsignal]
        fn scroll_capture_updated(self: Pin<&mut Self>, height: i32);

        #[qsignal]
        fn scroll_capture_warning(self: Pin<&mut Self>, message: QString);

        #[qsignal]
        fn close_all_pins(self: Pin<&mut Self>);

        #[qsignal]
        fn request_composition(self: Pin<&mut Self>, action: i32, selection_rect: QRectF);

        #[qsignal]
        fn action_finished(self: Pin<&mut Self>);

        #[qsignal]
        fn ocr_result(self: Pin<&mut Self>, content: QString);

        #[qsignal]
        fn pin_window_requested(self: Pin<&mut Self>, path: QString, selection_rect: QRectF, auto_ocr: bool);

        #[qsignal]
        fn scroll_capture_started(self: Pin<&mut Self>, selection_rect: QRectF);
    }

    impl cxx_qt::Threading for ScreenCapture {}
}

use crate::core::capture::action::{ActionContext, ActionResult, CaptureAction, CaptureInputMode};
use crate::core::capture::datasource;
use crate::core::capture::scroll_worker::{ScrollObserver, start_scroll_capture_thread};
use crate::core::capture::service::CaptureService;
use crate::core::capture::{clear_cached_captures, set_cached_capture};
use crate::core::geometry::Rect;
use crate::core::hotkey::HotkeyManager;
use crate::core::settings::{SETTINGS, ShortcutSettings};
use crate::interop::qt_rect_adapter::{CaptureRequestRect, SelectionRect, rect_to_qrect};
use crate::interop::qt_url_adapter;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QRectF, QString, QUrl};
use image::RgbaImage;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tracing::{error, info};

pub const UI_ACTION_COPY: i32 = qobject::UiAction::Copy.repr;
pub const UI_ACTION_SAVE: i32 = qobject::UiAction::Save.repr;
pub const UI_ACTION_PIN: i32 = qobject::UiAction::Pin.repr;
pub const UI_ACTION_OCR: i32 = qobject::UiAction::Ocr.repr;
pub const UI_ACTION_SCROLL: i32 = qobject::UiAction::Scroll.repr;
pub const UI_ACTION_QRCODE: i32 = qobject::UiAction::QrCode.repr;
pub const UI_ACTION_UNDO: i32 = qobject::UiAction::Undo.repr;
pub const UI_ACTION_REDO: i32 = qobject::UiAction::Redo.repr;
pub const UI_ACTION_CANCEL: i32 = qobject::UiAction::Cancel.repr;

pub fn is_undo_action(action: i32) -> bool {
    action == UI_ACTION_UNDO
}

pub fn is_redo_action(action: i32) -> bool {
    action == UI_ACTION_REDO
}

pub fn is_cancel_action(action: i32) -> bool {
    action == UI_ACTION_CANCEL
}

fn capture_action_from_code(action: i32) -> Option<CaptureAction> {
    match action {
        UI_ACTION_COPY => Some(CaptureAction::Copy),
        UI_ACTION_SAVE => Some(CaptureAction::Save),
        UI_ACTION_PIN => Some(CaptureAction::Pin),
        UI_ACTION_OCR => Some(CaptureAction::Ocr),
        UI_ACTION_SCROLL => Some(CaptureAction::Scroll),
        UI_ACTION_QRCODE => Some(CaptureAction::QrCode),
        _ => None,
    }
}

#[inline]
fn collect_process_memory() {
    unsafe {
        libmimalloc_sys::mi_collect(true);
    }
}

fn normalize_scale(scale: f64) -> f64 {
    if scale > 0.0 { scale } else { 1.0 }
}

#[derive(Clone, Copy)]
struct CaptureSpace {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale: f64,
}

impl Default for CaptureSpace {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            scale: 1.0,
        }
    }
}

impl CaptureSpace {
    fn from_screen(screen: crate::bridge::app::CursorScreen) -> Self {
        Self {
            x: screen.x,
            y: screen.y,
            width: screen.width,
            height: screen.height,
            scale: normalize_scale(screen.scale),
        }
    }

    fn from_rect(rect: &QRectF, scale: f64) -> Option<Self> {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return None;
        }
        Some(Self {
            x: rect.x(),
            y: rect.y(),
            width: rect.width(),
            height: rect.height(),
            scale: normalize_scale(scale),
        })
    }

    fn from_cursor() -> Option<Self> {
        crate::bridge::app::cursor_screen().map(Self::from_screen)
    }

    fn from_target(target: crate::core::capture::CaptureMonitorTarget) -> Self {
        let (cx, cy) = target.center();
        if let Some(screen) = crate::bridge::app::screen_at(cx, cy) {
            return Self::from_screen(screen);
        }

        let (x, y, width, height) = target.logical_geometry();
        Self {
            x,
            y,
            width,
            height,
            scale: normalize_scale(f64::from(target.effective_scale())),
        }
    }

    fn viewport(target: Option<crate::core::capture::CaptureMonitorTarget>) -> Self {
        target.map(Self::from_target).or_else(Self::from_cursor).unwrap_or_default()
    }

    fn pointer(rect: &QRectF, current_scale: f64, target: Option<crate::core::capture::CaptureMonitorTarget>, hint_scale: f64) -> Self {
        Self::from_rect(rect, current_scale)
            .or_else(|| target.map(Self::from_target))
            .or_else(Self::from_cursor)
            .unwrap_or(Self {
                scale: normalize_scale(hint_scale),
                ..Self::default()
            })
    }

    fn rect(self) -> QRectF {
        QRectF::new(self.x, self.y, self.width, self.height)
    }
}

pub struct ScreenCaptureRust {
    hotkey_manager: HotkeyManager,
    is_capturing: bool,
    pin_count: i32,
    capture_screen_rect: QRectF,
    capture_screen_scale: f64,
    scroll_capture_active: Arc<AtomicBool>,
    last_scroll_path: Option<String>,
    pending_scroll_action: Option<CaptureAction>,
}

impl Default for ScreenCaptureRust {
    fn default() -> Self {
        Self {
            hotkey_manager: HotkeyManager::default(),
            is_capturing: false,
            pin_count: 0,
            capture_screen_rect: QRectF::default(),
            capture_screen_scale: 1.0,
            scroll_capture_active: Arc::new(AtomicBool::new(false)),
            last_scroll_path: None,
            pending_scroll_action: None,
        }
    }
}

struct QtScrollObserver {
    qt_thread: cxx_qt::CxxQtThread<qobject::ScreenCapture>,
}

impl ScrollObserver for QtScrollObserver {
    fn on_update(&self, height: i32, thumbnail: RgbaImage) {
        set_cached_capture(datasource::VirtualCaptureSource::Scroll, thumbnail);
        let _ = self.qt_thread.queue(move |mut qobject| {
            qobject.as_mut().scroll_capture_updated(height);
        });
    }

    fn on_warning(&self, message: String) {
        let msg = QString::from(&message);
        let _ = self.qt_thread.queue(move |mut qobject| {
            qobject.as_mut().scroll_capture_warning(msg);
        });
    }

    fn on_finished(&self, final_image: Option<RgbaImage>) {
        if let Some(final_img) = final_image {
            if let Some(path) = CaptureService::save_temp(&final_img) {
                let _ = self.qt_thread.queue(move |mut qobject| {
                    qobject.as_mut().rust_mut().last_scroll_path = Some(path.clone());

                    let action = qobject.as_ref().rust().pending_scroll_action.clone();

                    qobject.as_mut().scroll_capture_finished(QString::from(&path));

                    if let Some(action_enum) = action {
                        qobject
                            .as_mut()
                            .process_action(action_enum, path, Rect::empty(), CaptureInputMode::FullImage);
                    }
                });
            }
        } else {
            let _ = self.qt_thread.queue(move |mut qobject| {
                qobject.as_mut().scroll_capture_finished(QString::from(""));
            });
        }
    }
}

impl qobject::ScreenCapture {
    pub fn prepare_capture(mut self: Pin<&mut Self>) {
        if *self.is_capturing() {
            info!("Capture already in progress, skipping prepare_capture");
            return;
        }
        info!("Preparing screen capture...");
        self.as_mut().sync_capture_target_from_cursor();
        self.as_mut().set_is_capturing(true);

        crate::spawn_qt_task!(
            self,
            async move { tokio::task::spawn_blocking(CaptureService::capture_screen).await.unwrap_or(false) },
            |mut qobject: Pin<&mut qobject::ScreenCapture>, success| {
                if success {
                    qobject.as_mut().capture_ready();
                } else {
                    error!("Failed to capture screen");
                }
                qobject.as_mut().set_is_capturing(false);
            }
        );

        crate::spawn_qt_task!(
            self,
            async move { tokio::task::spawn_blocking(CaptureService::fetch_windows_json).await.unwrap_or_default() },
            |mut qobject: Pin<&mut qobject::ScreenCapture>, json| {
                qobject.as_mut().window_info_ready(QString::from(&json));
            }
        );
    }

    pub fn quick_capture(mut self: Pin<&mut Self>, selection_rect: QRectF) {
        let rect = CaptureRequestRect::from_qrect(&selection_rect).rect();
        if *self.is_capturing() {
            info!("Capture already in progress, skipping quick_capture");
            return;
        }
        info!("Starting quick capture region: {},{} {}x{}", rect.x, rect.y, rect.width, rect.height);
        self.as_mut().sync_capture_target_from_cursor();
        self.as_mut().set_is_capturing(true);

        crate::spawn_qt_task!(
            self,
            async move {
                tokio::task::spawn_blocking(move || CaptureService::run_quick_capture_workflow(rect))
                    .await
                    .unwrap_or(false)
            },
            |mut qobject: Pin<&mut qobject::ScreenCapture>, result| {
                if result {
                    crate::notify_tr!("ScreenCapture", "Success", "Image copied to clipboard", Copy);
                }
                qobject.as_mut().set_is_capturing(false);
            }
        );
    }

    pub fn start_scroll_capture(mut self: Pin<&mut Self>, selection_rect: QRectF) {
        let rect = SelectionRect::from_qrect(&selection_rect).rect();
        self.as_mut().start_scroll_capture_rect(rect);
    }

    pub fn request_scroll_action(mut self: Pin<&mut Self>, action: i32) {
        self.as_mut().rust_mut().pending_scroll_action = capture_action_from_code(action);
        self.as_mut().stop_scroll_capture();
    }

    pub fn cancel_scroll_capture(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().pending_scroll_action = None;
        self.as_mut().stop_scroll_capture();
    }

    pub fn stop_scroll_capture(self: Pin<&mut Self>) {
        info!("Stopping scroll capture");
        self.rust().scroll_capture_active.store(false, Ordering::SeqCst);
    }

    pub fn copy_text(self: Pin<&mut Self>, text: QString) {
        crate::spawn_clipboard_copy!(self, text, "Text copied to clipboard", Copy);
    }

    pub fn copy_qrcode_result(self: Pin<&mut Self>, text: QString) {
        crate::spawn_clipboard_copy!(self, text, "QR Code content copied to clipboard", QrCode);
    }

    pub fn register_hotkeys(mut self: Pin<&mut Self>) {
        let qt_thread_screen = self.qt_thread();
        let screen_callback = move || {
            let _ = qt_thread_screen.queue(|mut qobject| {
                qobject.as_mut().screen_capture_shortcut_triggered();
            });
        };

        let qt_thread_quick = self.qt_thread();
        let quick_callback = move || {
            let _ = qt_thread_quick.queue(|mut qobject| {
                qobject.as_mut().quick_capture_shortcut_triggered();
            });
        };

        let settings = SETTINGS.lock().unwrap().get();
        let defaults = ShortcutSettings::default();

        let screen_shortcut = if settings.shortcuts.capture.is_empty() {
            defaults.capture
        } else {
            settings.shortcuts.capture
        };

        let quick_shortcut = if settings.shortcuts.quick_capture.is_empty() {
            defaults.quick_capture
        } else {
            settings.shortcuts.quick_capture
        };

        self.as_mut()
            .rust_mut()
            .hotkey_manager
            .register_global_hotkeys(&screen_shortcut, &quick_shortcut, screen_callback, quick_callback);
    }

    pub fn set_capture_shortcut(mut self: Pin<&mut Self>, shortcut: QString) {
        let shortcut_str = shortcut.to_string();
        self.as_mut().rust_mut().hotkey_manager.update_shortcut(&shortcut_str, true);
    }

    pub fn set_quick_capture_shortcut(mut self: Pin<&mut Self>, shortcut: QString) {
        let shortcut_str = shortcut.to_string();
        self.as_mut().rust_mut().hotkey_manager.update_shortcut(&shortcut_str, false);
    }

    pub fn generate_temp_path(self: Pin<&mut Self>, extension: QString) -> QString {
        QString::from(CaptureService::generate_temp_path(&extension.to_string()))
    }

    pub fn get_pixel_color(self: Pin<&mut Self>, x: i32, y: i32, scale: f64) -> QString {
        if let Some(hex) = CaptureService::get_pixel_hex(x, y, scale) {
            return QString::from(&hex);
        }
        QString::from("")
    }

    pub fn set_cursor_position(self: Pin<&mut Self>, x: i32, y: i32, scale: f64) {
        let local_x = f64::from(x);
        let local_y = f64::from(y);
        let target = crate::core::capture::active_monitor_target();
        let space = CaptureSpace::pointer(self.capture_screen_rect(), *self.capture_screen_scale(), target, scale);
        let global_x = space.x + local_x;
        let global_y = space.y + local_y;
        let effective_scale = space.scale;
        let (sx, sy) = if cfg!(target_os = "macos") {
            (global_x, global_y)
        } else {
            (global_x * effective_scale, global_y * effective_scale)
        };
        crate::core::RUNTIME.spawn_blocking(move || {
            if let Err(e) = rdev::simulate(&rdev::EventType::MouseMove { x: sx, y: sy }) {
                error!("Failed to move cursor: {:?}", e);
            }
        });
    }

    pub fn emit_close_all_pins(self: Pin<&mut Self>) {
        self.close_all_pins();
    }

    pub fn increment_pin_count(mut self: Pin<&mut Self>) {
        let count = *self.pin_count();
        self.as_mut().set_pin_count(count + 1);
    }

    pub fn decrement_pin_count(mut self: Pin<&mut Self>) {
        let count = *self.pin_count();
        if count > 0 {
            self.as_mut().set_pin_count(count - 1);
        }
    }

    pub fn request_action(self: Pin<&mut Self>, path: QUrl, action: i32, selection_rect: QRectF, has_annotations: bool) {
        let selection = SelectionRect::from_qrect(&selection_rect);
        let rect = selection.rect();
        let Some(action_enum) = capture_action_from_code(action) else {
            self.action_finished();
            return;
        };

        match action_enum {
            CaptureAction::Scroll => {
                self.start_scroll_capture_rect(rect);
            }
            CaptureAction::QrCode if !has_annotations => {
                let path_str = self.rust().resolve_path(&path);
                self.process_action(CaptureAction::QrCode, path_str, rect, CaptureInputMode::CropSelection);
            }
            _ => {
                if has_annotations {
                    self.request_composition(action, selection.to_qrect());
                } else {
                    self.submit_action_internal(path, action_enum, rect, CaptureInputMode::CropSelection);
                }
            }
        }
    }

    pub fn submit_capture(mut self: Pin<&mut Self>, path: QUrl, action: i32, selection_rect: QRectF) {
        let rect = SelectionRect::from_qrect(&selection_rect).rect();
        self.as_mut().submit_capture_by_code(path, action, rect, CaptureInputMode::CropSelection);
    }

    pub fn submit_composited_capture(mut self: Pin<&mut Self>, path: QUrl, action: i32, selection_rect: QRectF) {
        let rect = SelectionRect::from_qrect(&selection_rect).rect();
        self.as_mut().submit_capture_by_code(path, action, rect, CaptureInputMode::FullImage);
    }

    pub fn release_capture_buffers(mut self: Pin<&mut Self>) {
        clear_cached_captures();
        crate::bridge::provider::clear_cached_qimages();
        self.as_mut().rust_mut().last_scroll_path = None;
    }

    pub fn collect_memory(self: Pin<&mut Self>) {
        crate::core::RUNTIME.spawn_blocking(collect_process_memory);
    }

    fn submit_capture_by_code(mut self: Pin<&mut Self>, path: QUrl, action: i32, rect: Rect, input_mode: CaptureInputMode) {
        let Some(action_enum) = capture_action_from_code(action) else {
            self.as_mut().action_finished();
            return;
        };
        self.as_mut().submit_action_internal(path, action_enum, rect, input_mode);
    }

    fn submit_action_internal(mut self: Pin<&mut Self>, path: QUrl, action: CaptureAction, rect: Rect, input_mode: CaptureInputMode) {
        let path_str = self.rust().resolve_path(&path);

        info!("Submitting capture action: {:?}, path: {}", action, path_str);

        match action {
            CaptureAction::Copy | CaptureAction::Save | CaptureAction::Pin | CaptureAction::Ocr | CaptureAction::QrCode => {
                self.process_action(action, path_str, rect, input_mode)
            }
            _ => {
                self.as_mut().action_finished();
            }
        }
    }

    fn start_scroll_capture_rect(mut self: Pin<&mut Self>, rect: Rect) {
        if self.rust().scroll_capture_active.load(Ordering::SeqCst) {
            info!("Scroll capture already active");
            return;
        }
        info!(
            "Starting optimized scroll capture at {},{} {}x{}",
            rect.x, rect.y, rect.width, rect.height
        );
        self.as_mut().rust_mut().scroll_capture_active.store(true, Ordering::SeqCst);
        let active_flag = self.rust().scroll_capture_active.clone();
        let observer = Box::new(QtScrollObserver { qt_thread: self.qt_thread() });
        start_scroll_capture_thread(rect, active_flag, observer);
        self.as_mut().scroll_capture_started(rect_to_qrect(rect));
    }

    fn process_action(self: Pin<&mut Self>, action: CaptureAction, path: String, rect: Rect, input_mode: CaptureInputMode) {
        let context = match input_mode {
            CaptureInputMode::CropSelection => ActionContext::crop_selection(path, rect),
            CaptureInputMode::FullImage => ActionContext::full_image(path),
        };
        let action_clone = action.clone();

        crate::spawn_qt_task!(
            self,
            async move {
                tokio::task::spawn_blocking(move || action_clone.execute(context))
                    .await
                    .unwrap_or(ActionResult::Error("Task failed".to_string()))
            },
            |mut qobject: Pin<&mut qobject::ScreenCapture>, result| {
                match result {
                    ActionResult::Copied => {
                        crate::notify_tr!("ScreenCapture", "Success", "Image copied to clipboard", Copy);
                    }
                    ActionResult::Saved(saved_path) => {
                        let title = crate::bridge::app::tr("ScreenCapture", "Saved");
                        let msg = format!("{}: {}", crate::bridge::app::tr("ScreenCapture", "Image saved to"), saved_path);
                        crate::core::notify::show(&title.to_string(), &msg, crate::core::notify::NotificationType::Save);
                    }
                    ActionResult::PinRequested(temp_path, auto_ocr) => {
                        qobject
                            .as_mut()
                            .pin_window_requested(QString::from(&temp_path), rect_to_qrect(rect), auto_ocr);
                    }
                    ActionResult::OcrResult(content) => {
                        crate::spawn_clipboard_copy!(qobject, QString::from(&content), "QR Code content copied to clipboard", QrCode);
                        qobject.as_mut().ocr_result(QString::from(&content));
                    }
                    ActionResult::Error(e) => {
                        error!("Action error: {e}");
                        if action == CaptureAction::QrCode {
                            crate::core::notify::show("MinnowSnap", "No QR Code detected", crate::core::notify::NotificationType::Info);
                        }
                    }
                    ActionResult::NoOp => {}
                }
                qobject.as_mut().action_finished();
            }
        );
    }

    fn sync_capture_target_from_cursor(mut self: Pin<&mut Self>) {
        let (cursor_x, cursor_y) = crate::bridge::app::cursor_position();
        let target = crate::core::capture::activate_monitor_at_point(cursor_x, cursor_y);
        self.as_mut().apply_capture_target(target);
    }

    fn apply_capture_target(mut self: Pin<&mut Self>, target: Option<crate::core::capture::CaptureMonitorTarget>) {
        let space = CaptureSpace::viewport(target);
        self.as_mut().set_capture_screen_rect(space.rect());
        self.as_mut().set_capture_screen_scale(space.scale);
    }
}

impl ScreenCaptureRust {
    fn resolve_path(&self, path: &QUrl) -> String {
        let mut path_str = qt_url_adapter::qurl_to_local_or_uri(path);
        if (path_str.is_empty() || datasource::parse_virtual_source(&path_str).is_some())
            && let Some(last) = self.last_scroll_path.as_deref()
        {
            path_str = last.to_string();
        }

        path_str
    }
}

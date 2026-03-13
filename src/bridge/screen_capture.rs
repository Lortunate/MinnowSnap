#![allow(clippy::too_many_arguments)]
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
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, is_capturing)]
        #[qproperty(i32, pin_count)]
        type ScreenCapture = super::ScreenCaptureRust;

        #[qinvokable]
        fn prepare_capture(self: Pin<&mut Self>);

        #[qinvokable]
        fn quick_capture(self: Pin<&mut Self>, x: i32, y: i32, width: i32, height: i32);

        #[qinvokable]
        fn start_scroll_capture(self: Pin<&mut Self>, x: i32, y: i32, width: i32, height: i32);

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
        fn request_action(self: Pin<&mut Self>, path: QString, action: i32, x: i32, y: i32, width: i32, height: i32, has_annotations: bool);

        #[qinvokable]
        fn request_scroll_action(self: Pin<&mut Self>, action: i32);

        #[qinvokable]
        fn cancel_scroll_capture(self: Pin<&mut Self>);

        #[qinvokable]
        fn submit_capture(self: Pin<&mut Self>, path: QString, action: i32, x: i32, y: i32, width: i32, height: i32);

        #[qinvokable]
        fn submit_composited_capture(self: Pin<&mut Self>, path: QString, action: i32, x: i32, y: i32, width: i32, height: i32);

        #[qinvokable]
        fn release_capture_buffers(self: Pin<&mut Self>);

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
        fn request_composition(self: Pin<&mut Self>, action: i32, x: i32, y: i32, width: i32, height: i32);

        #[qsignal]
        fn action_finished(self: Pin<&mut Self>);

        #[qsignal]
        fn ocr_result(self: Pin<&mut Self>, content: QString);

        #[qsignal]
        fn pin_window_requested(self: Pin<&mut Self>, path: QString, x: i32, y: i32, width: i32, height: i32, auto_ocr: bool);

        #[qsignal]
        fn scroll_capture_started(self: Pin<&mut Self>, x: i32, y: i32, width: i32, height: i32);
    }

    impl cxx_qt::Threading for ScreenCapture {}
}

use crate::core::capture::action::{ActionContext, ActionResult, CaptureAction, CaptureInputMode};
use crate::core::capture::scroll_worker::{ScrollObserver, start_scroll_capture_thread};
use crate::core::capture::service::CaptureService;
use crate::core::capture::datasource;
use crate::core::capture::{LAST_CAPTURE, SCROLL_CAPTURE};
use crate::core::hotkey::HotkeyManager;
use crate::core::settings::{SETTINGS, ShortcutSettings};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use image::RgbaImage;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tracing::{error, info};

pub const UI_ACTION_COPY: i32 = qobject::UiAction::Copy.repr as i32;
pub const UI_ACTION_SAVE: i32 = qobject::UiAction::Save.repr as i32;
pub const UI_ACTION_PIN: i32 = qobject::UiAction::Pin.repr as i32;
pub const UI_ACTION_OCR: i32 = qobject::UiAction::Ocr.repr as i32;
pub const UI_ACTION_SCROLL: i32 = qobject::UiAction::Scroll.repr as i32;
pub const UI_ACTION_QRCODE: i32 = qobject::UiAction::QrCode.repr as i32;
pub const UI_ACTION_UNDO: i32 = qobject::UiAction::Undo.repr as i32;
pub const UI_ACTION_REDO: i32 = qobject::UiAction::Redo.repr as i32;
pub const UI_ACTION_CANCEL: i32 = qobject::UiAction::Cancel.repr as i32;

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

pub struct ScreenCaptureRust {
    hotkey_manager: HotkeyManager,
    is_capturing: bool,
    pin_count: i32,
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
        if let Ok(mut cache) = SCROLL_CAPTURE.lock() {
            *cache = Some(thumbnail);
        }
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
                        let path_clean = crate::core::io::storage::clean_url_path(&path);
                        qobject
                            .as_mut()
                            .process_action(action_enum, path_clean, 0, 0, 0, 0, CaptureInputMode::FullImage);
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

    pub fn quick_capture(mut self: Pin<&mut Self>, x: i32, y: i32, width: i32, height: i32) {
        if *self.is_capturing() {
            info!("Capture already in progress, skipping quick_capture");
            return;
        }
        info!("Starting quick capture region: {},{} {}x{}", x, y, width, height);
        self.as_mut().set_is_capturing(true);

        crate::spawn_qt_task!(
            self,
            async move {
                tokio::task::spawn_blocking(move || CaptureService::run_quick_capture_workflow(x, y, width, height))
                    .await
                    .unwrap_or(None)
            },
            |mut qobject: Pin<&mut qobject::ScreenCapture>, result| {
                if let Some(saved) = result {
                    let title = crate::bridge::app::tr("ScreenCapture", "Quick Capture");
                    let msg = format!("{}: {}", crate::bridge::app::tr("ScreenCapture", "Image saved to"), saved);
                    crate::core::notify::show(&title.to_string(), &msg, crate::core::notify::NotificationType::Save);
                }
                qobject.as_mut().set_is_capturing(false);
            }
        );
    }

    pub fn start_scroll_capture(mut self: Pin<&mut Self>, x: i32, y: i32, width: i32, height: i32) {
        if self.rust().scroll_capture_active.load(Ordering::SeqCst) {
            info!("Scroll capture already active");
            return;
        }
        info!("Starting optimized scroll capture at {x},{y} {width}x{height}");
        self.as_mut().rust_mut().scroll_capture_active.store(true, Ordering::SeqCst);
        let active_flag = self.rust().scroll_capture_active.clone();

        let observer = Box::new(QtScrollObserver { qt_thread: self.qt_thread() });

        start_scroll_capture_thread(x, y, width, height, active_flag, observer);

        self.as_mut().scroll_capture_started(x, y, width, height);
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
        let x = x as f64;
        let y = y as f64;
        let (sx, sy) = if cfg!(target_os = "macos") { (x, y) } else { (x * scale, y * scale) };
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

    pub fn request_action(self: Pin<&mut Self>, path: QString, action: i32, x: i32, y: i32, width: i32, height: i32, has_annotations: bool) {
        let Some(action_enum) = capture_action_from_code(action) else {
            self.action_finished();
            return;
        };

        match action_enum {
            CaptureAction::Scroll => {
                self.start_scroll_capture(x, y, width, height);
            }
            CaptureAction::QrCode if !has_annotations => {
                let path_str = self.rust().resolve_path(&path);
                self.process_action(CaptureAction::QrCode, path_str, x, y, width, height, CaptureInputMode::CropSelection);
            }
            _ => {
                if has_annotations {
                    self.request_composition(action, x, y, width, height);
                } else {
                    self.submit_capture(path, action, x, y, width, height);
                }
            }
        }
    }

    pub fn submit_capture(mut self: Pin<&mut Self>, path: QString, action: i32, x: i32, y: i32, width: i32, height: i32) {
        self.as_mut()
            .submit_capture_internal(path, action, x, y, width, height, CaptureInputMode::CropSelection);
    }

    pub fn submit_composited_capture(mut self: Pin<&mut Self>, path: QString, action: i32, x: i32, y: i32, width: i32, height: i32) {
        self.as_mut()
            .submit_capture_internal(path, action, x, y, width, height, CaptureInputMode::FullImage);
    }

    pub fn release_capture_buffers(mut self: Pin<&mut Self>) {
        if let Ok(mut cache) = LAST_CAPTURE.lock() {
            *cache = None;
        }
        if let Ok(mut cache) = SCROLL_CAPTURE.lock() {
            *cache = None;
        }
        self.as_mut().rust_mut().last_scroll_path = None;
    }

    fn submit_capture_internal(
        mut self: Pin<&mut Self>,
        path: QString,
        action: i32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        input_mode: CaptureInputMode,
    ) {
        let Some(action_enum) = capture_action_from_code(action) else {
            self.as_mut().action_finished();
            return;
        };
        let path_str = self.rust().resolve_path(&path);

        info!("Submitting capture action: {}, path: {}", action, path_str);

        match action_enum {
            CaptureAction::Copy | CaptureAction::Save | CaptureAction::Pin | CaptureAction::Ocr | CaptureAction::QrCode => {
                self.process_action(action_enum, path_str, x, y, width, height, input_mode)
            }
            _ => {
                self.as_mut().action_finished();
            }
        }
    }

    fn process_action(
        self: Pin<&mut Self>,
        action: CaptureAction,
        path: String,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        input_mode: CaptureInputMode,
    ) {
        let context = match input_mode {
            CaptureInputMode::CropSelection => ActionContext::crop_selection(path, x, y, width, height),
            CaptureInputMode::FullImage => ActionContext::full_image(path, x, y, width, height),
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
                            .pin_window_requested(QString::from(&temp_path), x, y, width, height, auto_ocr);
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
}

impl ScreenCaptureRust {
    fn resolve_path(&self, path: &QString) -> String {
        let mut path_str = path.to_string();
        if (path_str.is_empty() || datasource::parse_virtual_source(&path_str).is_some())
            && let Some(last) = self.last_scroll_path.as_deref()
        {
            path_str = last.to_string();
        }

        crate::core::io::storage::clean_url_path(&path_str)
    }
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, is_capturing, cxx_name = "isCapturing")]
        #[qproperty(i32, pin_count, cxx_name = "pinCount")]
        type ScreenCapture = super::ScreenCaptureRust;

        #[qinvokable]
        #[cxx_name = "prepareCapture"]
        fn prepare_capture(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "quickCapture"]
        fn quick_capture(self: Pin<&mut Self>, x: i32, y: i32, width: i32, height: i32);

        #[qinvokable]
        #[cxx_name = "startScrollCapture"]
        fn start_scroll_capture(self: Pin<&mut Self>, x: i32, y: i32, width: i32, height: i32);

        #[qinvokable]
        #[cxx_name = "stopScrollCapture"]
        fn stop_scroll_capture(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "copyText"]
        fn copy_text(self: Pin<&mut Self>, text: QString);

        #[qinvokable]
        #[cxx_name = "copyQrcodeResult"]
        fn copy_qrcode_result(self: Pin<&mut Self>, text: QString);

        #[qinvokable]
        #[cxx_name = "registerHotkeys"]
        fn register_hotkeys(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "setCaptureShortcut"]
        fn set_capture_shortcut(self: Pin<&mut Self>, shortcut: QString);

        #[qinvokable]
        #[cxx_name = "setQuickCaptureShortcut"]
        fn set_quick_capture_shortcut(self: Pin<&mut Self>, shortcut: QString);

        #[qinvokable]
        #[cxx_name = "generateTempPath"]
        fn generate_temp_path(self: Pin<&mut Self>, extension: QString) -> QString;

        #[qinvokable]
        #[cxx_name = "getPixelColor"]
        fn get_pixel_color(self: Pin<&mut Self>, x: i32, y: i32, scale: f64) -> QString;

        #[qinvokable]
        #[cxx_name = "setCursorPosition"]
        fn set_cursor_position(self: Pin<&mut Self>, x: i32, y: i32, scale: f64);

        #[qinvokable]
        #[cxx_name = "emitCloseAllPins"]
        fn emit_close_all_pins(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "incrementPinCount"]
        fn increment_pin_count(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "decrementPinCount"]
        fn decrement_pin_count(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "requestAction"]
        fn request_action(self: Pin<&mut Self>, path: QString, action: QString, x: i32, y: i32, width: i32, height: i32, has_annotations: bool);

        #[qinvokable]
        #[cxx_name = "requestScrollAction"]
        fn request_scroll_action(self: Pin<&mut Self>, action: QString);

        #[qinvokable]
        #[cxx_name = "cancelScrollCapture"]
        fn cancel_scroll_capture(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "submitCapture"]
        fn submit_capture(self: Pin<&mut Self>, path: QString, action: QString, x: i32, y: i32, width: i32, height: i32);

        #[qinvokable]
        #[cxx_name = "submitCompositedCapture"]
        fn submit_composited_capture(self: Pin<&mut Self>, path: QString, action: QString, x: i32, y: i32, width: i32, height: i32);

        #[qsignal]
        #[cxx_name = "screenCaptureShortcutTriggered"]
        fn screen_capture_shortcut_triggered(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "quickCaptureShortcutTriggered"]
        fn quick_capture_shortcut_triggered(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "screenshotCaptured"]
        fn screenshot_captured(self: Pin<&mut Self>, path: QString);

        #[qsignal]
        #[cxx_name = "windowInfoReady"]
        fn window_info_ready(self: Pin<&mut Self>, json: QString);

        #[qsignal]
        #[cxx_name = "captureReady"]
        fn capture_ready(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "scrollCaptureFinished"]
        fn scroll_capture_finished(self: Pin<&mut Self>, path: QString);

        #[qsignal]
        #[cxx_name = "scrollCaptureUpdated"]
        fn scroll_capture_updated(self: Pin<&mut Self>, height: i32);

        #[qsignal]
        #[cxx_name = "scrollCaptureWarning"]
        fn scroll_capture_warning(self: Pin<&mut Self>, message: QString);

        #[qsignal]
        #[cxx_name = "closeAllPins"]
        fn close_all_pins(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "requestComposition"]
        fn request_composition(self: Pin<&mut Self>, action: QString, x: i32, y: i32, width: i32, height: i32);

        #[qsignal]
        #[cxx_name = "actionFinished"]
        fn action_finished(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "ocrResult"]
        fn ocr_result(self: Pin<&mut Self>, content: QString);

        #[qsignal]
        #[cxx_name = "pinWindowRequested"]
        fn pin_window_requested(self: Pin<&mut Self>, path: QString, x: i32, y: i32, width: i32, height: i32, auto_ocr: bool);

        #[qsignal]
        #[cxx_name = "scrollCaptureStarted"]
        fn scroll_capture_started(self: Pin<&mut Self>, x: i32, y: i32, width: i32, height: i32);
    }

    impl cxx_qt::Threading for ScreenCapture {}
}

use crate::core::hotkey_state::{update_hotkey, HotkeyState};
use crate::core::capture::SCROLL_CAPTURE;
use crate::core::capture::action::{ActionContext, ActionResult, CaptureAction, CaptureInputMode};
use crate::core::capture::scroll_worker::{ScrollObserver, start_scroll_capture_thread};
use crate::core::capture::service::CaptureService;
use crate::core::hotkey::HotkeyService;
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

use std::str::FromStr;

pub struct ScreenCaptureRust {
    hotkey_state: HotkeyState,
    is_capturing: bool,
    pin_count: i32,
    scroll_capture_active: Arc<AtomicBool>,
    last_scroll_path: Option<String>,
    pending_scroll_action: Option<String>,
}

impl Default for ScreenCaptureRust {
    fn default() -> Self {
        Self {
            hotkey_state: HotkeyState::default(),
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

                    if let Some(act) = action {
                        let action_enum = CaptureAction::from_str(&act).unwrap_or(CaptureAction::Unknown);
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

    pub fn request_scroll_action(mut self: Pin<&mut Self>, action: QString) {
        self.as_mut().rust_mut().pending_scroll_action = Some(action.to_string());
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

        if let Some(registration) = HotkeyService::register_global_hotkeys(&screen_shortcut, &quick_shortcut, screen_callback, quick_callback) {
            let mut rust = self.as_mut().rust_mut();
            rust.hotkey_state.manager = Some(registration.manager);
            rust.hotkey_state.ids = registration.ids;
            rust.hotkey_state.screen = registration.screen_hotkey;
            rust.hotkey_state.quick = registration.quick_hotkey;
        }
    }

    pub fn set_capture_shortcut(mut self: Pin<&mut Self>, shortcut: QString) {
        let state = &mut self.as_mut().rust_mut().hotkey_state;
        update_hotkey(state, shortcut, true);
    }

    pub fn set_quick_capture_shortcut(mut self: Pin<&mut Self>, shortcut: QString) {
        let state = &mut self.as_mut().rust_mut().hotkey_state;
        update_hotkey(state, shortcut, false);
    }

    pub fn generate_temp_path(self: Pin<&mut Self>, extension: QString) -> QString {
        QString::from(CaptureService::generate_temp_path(&extension.to_string()))
    }

    pub fn get_pixel_color(self: Pin<&mut Self>, x: i32, y: i32, scale: f64) -> QString {
        use crate::core::capture::LAST_CAPTURE;

        let x_phys = (x as f64 * scale) as i32;
        let y_phys = (y as f64 * scale) as i32;

        if let Ok(lock) = LAST_CAPTURE.lock()
            && let Some(img) = &*lock
            && let (Ok(u_x), Ok(u_y)) = (u32::try_from(x_phys), u32::try_from(y_phys))
            && u_x < img.width()
            && u_y < img.height()
        {
            let pixel = img.get_pixel(u_x, u_y);
            let hex = format!("#{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2]);
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

    pub fn request_action(self: Pin<&mut Self>, path: QString, action: QString, x: i32, y: i32, width: i32, height: i32, has_annotations: bool) {
        let action_str = action.to_string();
        let action_enum = CaptureAction::from_str(&action_str).unwrap_or(CaptureAction::Unknown);

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

    pub fn submit_capture(mut self: Pin<&mut Self>, path: QString, action: QString, x: i32, y: i32, width: i32, height: i32) {
        self.as_mut()
            .submit_capture_internal(path, action, x, y, width, height, CaptureInputMode::CropSelection);
    }

    pub fn submit_composited_capture(mut self: Pin<&mut Self>, path: QString, action: QString, x: i32, y: i32, width: i32, height: i32) {
        self.as_mut()
            .submit_capture_internal(path, action, x, y, width, height, CaptureInputMode::FullImage);
    }

    fn submit_capture_internal(
        mut self: Pin<&mut Self>,
        path: QString,
        action: QString,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        input_mode: CaptureInputMode,
    ) {
        let action_str = action.to_string();
        let action_enum = CaptureAction::from_str(&action_str).unwrap_or(CaptureAction::Unknown);
        let path_str = self.rust().resolve_path(&path);

        info!("Submitting capture action: {}, path: {}", action_str, path_str);

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
        if (path_str.is_empty() || path_str.starts_with("image://minnow/preview"))
            && let Some(last) = self.last_scroll_path.as_deref()
        {
            path_str = last.to_string();
        }

        crate::core::io::storage::clean_url_path(&path_str)
    }
}

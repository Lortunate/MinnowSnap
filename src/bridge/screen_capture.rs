#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, is_capturing, cxx_name = "isCapturing")]
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
        #[cxx_name = "cropImage"]
        fn crop_image(self: Pin<&mut Self>, path: QString, x: i32, y: i32, width: i32, height: i32) -> QString;

        #[qinvokable]
        #[cxx_name = "copyImage"]
        fn copy_image(self: Pin<&mut Self>, path: QString, x: i32, y: i32, width: i32, height: i32);

        #[qinvokable]
        #[cxx_name = "saveImage"]
        fn save_image(self: Pin<&mut Self>, path: QString, x: i32, y: i32, width: i32, height: i32) -> QStringList;

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
        #[cxx_name = "emitCloseAllPins"]
        fn emit_close_all_pins(self: Pin<&mut Self>);

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
    }

    impl cxx_qt::Threading for ScreenCapture {}
}

use crate::core::app::APP_NAME;
use crate::core::capture::scroll_worker::{start_scroll_capture_thread, ScrollObserver};
use crate::core::capture::service::CaptureService;
use crate::core::capture::SCROLL_CAPTURE;
use crate::core::hotkey::{HotkeyIds, HotkeyService};
use crate::core::settings::{ShortcutSettings, SETTINGS};
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use global_hotkey::{hotkey::HotKey, GlobalHotKeyManager};
use image::RgbaImage;
use log::{error, info};
use notify_rust::Notification;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct ScreenCaptureRust {
    hotkey_manager: Option<GlobalHotKeyManager>,
    hotkey_ids: Arc<Mutex<HotkeyIds>>,
    current_screen_hotkey: Option<HotKey>,
    current_quick_hotkey: Option<HotKey>,
    is_capturing: bool,
    scroll_capture_active: Arc<AtomicBool>,
    last_scroll_path: Option<String>,
}

impl Default for ScreenCaptureRust {
    fn default() -> Self {
        Self {
            hotkey_manager: None,
            hotkey_ids: Arc::new(Mutex::new(HotkeyIds::default())),
            current_screen_hotkey: None,
            current_quick_hotkey: None,
            is_capturing: false,
            scroll_capture_active: Arc::new(AtomicBool::new(false)),
            last_scroll_path: None,
        }
    }
}

fn spawn_thread<F>(name: &str, f: F)
where
    F: FnOnce() + Send + 'static,
{
    thread::Builder::new().name(name.to_string()).spawn(f).expect("Failed to spawn thread");
}

fn send_notification(title: &str, message: &str) {
    if let Err(e) = Notification::new().summary(title).body(message).appname(APP_NAME).show() {
        error!("Failed to send notification: {}", e);
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
        self.qt_thread
            .queue(move |mut qobject| {
                qobject.as_mut().scroll_capture_updated(height);
            })
            .ok();
    }

    fn on_warning(&self, message: String) {
        let msg = QString::from(&message);
        self.qt_thread
            .queue(move |mut qobject| {
                qobject.as_mut().scroll_capture_warning(msg);
            })
            .ok();
    }

    fn on_finished(&self, final_image: Option<RgbaImage>) {
        if let Some(final_img) = final_image {
            if let Some(path) = CaptureService::save_temp(&final_img) {
                self.qt_thread
                    .queue(move |mut qobject| {
                        let path_val = path.strip_prefix("file://").unwrap_or(&path).to_string();
                        qobject.as_mut().rust_mut().last_scroll_path = Some(path);
                        qobject.as_mut().scroll_capture_finished(QString::from(&path_val));
                    })
                    .ok();
            }
        } else {
            self.qt_thread
                .queue(move |mut qobject| {
                    qobject.as_mut().scroll_capture_finished(QString::from(""));
                })
                .ok();
        }
    }
}

impl qobject::ScreenCapture {
    pub fn prepare_capture(mut self: Pin<&mut Self>) {
        if *self.is_capturing() {
            return;
        }
        self.as_mut().set_is_capturing(true);

        let qt_thread = self.qt_thread();

        spawn_thread("minnow-capture-monitor", move || {
            let result = CaptureService::prepare_capture();

            qt_thread
                .queue(move |mut qobject| {
                    if let Some((_image, json)) = result {
                        qobject.as_mut().window_info_ready(QString::from(&json));
                        qobject.as_mut().capture_ready();
                    } else {
                        error!("Failed to capture screen");
                    }
                    qobject.as_mut().set_is_capturing(false);
                })
                .ok();
        });
    }

    pub fn quick_capture(mut self: Pin<&mut Self>, x: i32, y: i32, width: i32, height: i32) {
        if *self.is_capturing() {
            return;
        }
        self.as_mut().set_is_capturing(true);

        let qt_thread = self.qt_thread();

        spawn_thread("minnow-quick-capture", move || {
            let result = CaptureService::run_quick_capture_workflow(x, y, width, height);

            qt_thread
                .queue(move |mut qobject| {
                    if let Some((temp_path, saved_path)) = result {
                        qobject.as_mut().screenshot_captured(QString::from(&temp_path));

                        if let Some(saved) = saved_path {
                            let title = crate::bridge::app::tr("ScreenCapture", "Quick Capture");
                            let msg = format!("{}: {}", crate::bridge::app::tr("ScreenCapture", "Image saved to"), saved);
                            send_notification(&title.to_string(), &msg);
                        }
                    }
                    qobject.as_mut().set_is_capturing(false);
                })
                .ok();
        });
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
    }

    pub fn stop_scroll_capture(self: Pin<&mut Self>) {
        info!("Stopping scroll capture");
        self.rust().scroll_capture_active.store(false, Ordering::SeqCst);
    }

    pub fn crop_image(self: Pin<&mut Self>, path: QString, x: i32, y: i32, width: i32, height: i32) -> QString {
        let path_str = self.rust().resolve_path(&path);

        if let Some(cropped) = CaptureService::resolve_and_crop(&path_str, x, y, width, height) {
            if let Some(saved_path) = CaptureService::save_temp(&cropped) {
                return QString::from(&saved_path);
            }
        }
        QString::from("")
    }

    pub fn copy_image(self: Pin<&mut Self>, path: QString, x: i32, y: i32, width: i32, height: i32) {
        let path_str = self.rust().resolve_path(&path);
        let qt_thread = self.qt_thread();

        spawn_thread("minnow-copy-clipboard", move || {
            match CaptureService::copy_region_to_clipboard(&path_str, x, y, width, height) {
                Ok(_) => {
                    qt_thread
                        .queue(|_qobject| {
                            let title = crate::bridge::app::tr("ScreenCapture", "Success");
                            let msg = crate::bridge::app::tr("ScreenCapture", "Image copied to clipboard");
                            send_notification(&title.to_string(), &msg.to_string());
                        })
                        .ok();
                }
                Err(e) => error!("Clipboard error: {e}"),
            }
        });
    }

    pub fn save_image(self: Pin<&mut Self>, path: QString, x: i32, y: i32, width: i32, height: i32) -> QStringList {
        let path_str = self.rust().resolve_path(&path);
        let qt_thread = self.qt_thread();

        spawn_thread("minnow-save-files", move || {
            match CaptureService::save_region_to_user_dir(&path_str, x, y, width, height) {
                Ok(saved_path) => {
                    qt_thread
                        .queue(move |_qobject| {
                            let title = crate::bridge::app::tr("ScreenCapture", "Saved");
                            let msg = format!("{}: {}", crate::bridge::app::tr("ScreenCapture", "Image saved to"), saved_path);
                            send_notification(&title.to_string(), &msg);
                        })
                        .ok();
                }
                Err(e) => error!("Save error: {e}"),
            }
        });

        QStringList::default()
    }

    pub fn register_hotkeys(mut self: Pin<&mut Self>) {
        let qt_thread_screen = self.qt_thread();
        let screen_callback = move || {
            qt_thread_screen
                .queue(|mut qobject| {
                    qobject.as_mut().screen_capture_shortcut_triggered();
                })
                .ok();
        };

        let qt_thread_quick = self.qt_thread();
        let quick_callback = move || {
            qt_thread_quick
                .queue(|mut qobject| {
                    qobject.as_mut().quick_capture_shortcut_triggered();
                })
                .ok();
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

        let hotkey_ids = self.rust().hotkey_ids.clone();

        if let Some(manager) = HotkeyService::register_global_hotkeys(&screen_shortcut, &quick_shortcut, screen_callback, quick_callback, hotkey_ids)
        {
            self.as_mut().rust_mut().hotkey_manager = Some(manager);
        }
    }

    pub fn set_capture_shortcut(mut self: Pin<&mut Self>, shortcut: QString) {
        self.as_mut().rust_mut().update_hotkey(shortcut, true);
    }

    pub fn set_quick_capture_shortcut(mut self: Pin<&mut Self>, shortcut: QString) {
        self.as_mut().rust_mut().update_hotkey(shortcut, false);
    }

    pub fn generate_temp_path(self: Pin<&mut Self>, extension: QString) -> QString {
        QString::from(CaptureService::generate_temp_path(&extension.to_string()))
    }

    pub fn emit_close_all_pins(self: Pin<&mut Self>) {
        self.close_all_pins();
    }
}

impl ScreenCaptureRust {
    fn update_hotkey(&mut self, shortcut: QString, is_screen: bool) {
        let mut shortcut_str = shortcut.to_string();
        if shortcut_str.is_empty() {
            let defaults = ShortcutSettings::default();
            shortcut_str = if is_screen { defaults.capture } else { defaults.quick_capture };
        }

        if let Some(manager) = &self.hotkey_manager {
            let current_hotkey = if is_screen {
                &mut self.current_screen_hotkey
            } else {
                &mut self.current_quick_hotkey
            };

            HotkeyService::update_hotkey_registration(manager, current_hotkey, &shortcut_str, &self.hotkey_ids, is_screen);
        }
    }

    fn resolve_path(&self, path: &QString) -> String {
        let mut path_str = path.to_string();
        if path_str.is_empty() || path_str.starts_with("image://minnow/preview") {
            if let Some(last) = self.last_scroll_path.as_deref() {
                path_str = last.to_string();
            }
        }
        path_str
    }
}

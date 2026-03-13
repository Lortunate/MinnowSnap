use cxx_qt::Threading;
use cxx_qt_lib::QString;
use std::pin::Pin;

#[cxx_qt::bridge]
mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, image_path, cxx_name = "imagePath")]
        #[qproperty(bool, auto_ocr, cxx_name = "autoOcr")]
        type PinController = super::PinControllerRust;

        #[qinvokable]
        #[cxx_name = "copyImage"]
        fn copy_image(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "saveImage"]
        fn save_image(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "close"]
        fn close(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "closeAll"]
        fn close_all(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "triggerOcr"]
        fn trigger_ocr(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "closeRequested"]
        fn close_requested(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "closeAllRequested"]
        fn close_all_requested(self: Pin<&mut Self>);

        #[qsignal]
        #[cxx_name = "ocrRequested"]
        fn ocr_requested(self: Pin<&mut Self>);
    }

    impl cxx_qt::Threading for PinController {}
}

#[derive(Default)]
pub struct PinControllerRust {
    image_path: QString,
    auto_ocr: bool,
}

impl qobject::PinController {
    pub fn copy_image(self: Pin<&mut Self>) {
        let path = self.image_path().to_string();
        let clean_path = crate::core::io::storage::clean_url_path(&path);
        crate::spawn_qt_task!(
            self,
            async move {
                tokio::task::spawn_blocking(move || {
                    crate::core::capture::action::CaptureAction::Copy.execute(crate::core::capture::action::ActionContext::full_image(clean_path, 0, 0, 0, 0))
                })
                .await
                .unwrap_or(crate::core::capture::action::ActionResult::NoOp)
            },
            |_qobject, result| {
                if matches!(result, crate::core::capture::action::ActionResult::Copied) {
                    crate::notify_tr!("ScreenCapture", "Success", "Image copied to clipboard", Copy);
                }
            }
        );
    }

    pub fn save_image(self: Pin<&mut Self>) {
        let path = self.image_path().to_string();
        let clean_path = crate::core::io::storage::clean_url_path(&path);
        crate::spawn_qt_task!(
            self,
            async move {
                tokio::task::spawn_blocking(move || {
                    crate::core::capture::action::CaptureAction::Save.execute(crate::core::capture::action::ActionContext::full_image(clean_path, 0, 0, 0, 0))
                })
                .await
                .unwrap_or(crate::core::capture::action::ActionResult::NoOp)
            },
            |_qobject, result| {
                if let crate::core::capture::action::ActionResult::Saved(saved_path) = result {
                    let title = crate::bridge::app::tr("ScreenCapture", "Saved");
                    let msg = format!("{}: {}", crate::bridge::app::tr("ScreenCapture", "Image saved to"), saved_path);
                    crate::core::notify::show(&title.to_string(), &msg, crate::core::notify::NotificationType::Save);
                }
            }
        );
    }

    pub fn close(self: Pin<&mut Self>) {
        self.close_requested();
    }

    pub fn close_all(self: Pin<&mut Self>) {
        self.close_all_requested();
    }

    pub fn trigger_ocr(self: Pin<&mut Self>) {
        self.ocr_requested();
    }
}

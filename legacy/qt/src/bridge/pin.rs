use crate::interop::qt_url_adapter;
use cxx_qt::Threading;
use cxx_qt_lib::QUrl;
use std::pin::Pin;

#[cxx_qt::bridge]
mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QUrl, image_path)]
        #[qproperty(bool, auto_ocr)]
        type PinController = super::PinControllerRust;

        #[qinvokable]
        fn copy_image(self: Pin<&mut Self>);

        #[qinvokable]
        fn save_image(self: Pin<&mut Self>);

        #[qinvokable]
        fn close(self: Pin<&mut Self>);

        #[qinvokable]
        fn close_all(self: Pin<&mut Self>);

        #[qinvokable]
        fn trigger_ocr(self: Pin<&mut Self>);

        #[qsignal]
        fn close_requested(self: Pin<&mut Self>);

        #[qsignal]
        fn close_all_requested(self: Pin<&mut Self>);

        #[qsignal]
        fn ocr_requested(self: Pin<&mut Self>);
    }

    impl cxx_qt::Threading for PinController {}
}

#[derive(Default)]
pub struct PinControllerRust {
    image_path: QUrl,
    auto_ocr: bool,
}

impl qobject::PinController {
    pub fn copy_image(self: Pin<&mut Self>) {
        let resolved_path = qt_url_adapter::qurl_to_local_or_uri(self.image_path());
        crate::spawn_qt_task!(
            self,
            async move {
                tokio::task::spawn_blocking(move || {
                    crate::core::capture::action::CaptureAction::Copy.execute(crate::core::capture::action::ActionContext::full_image(resolved_path))
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
        let resolved_path = qt_url_adapter::qurl_to_local_or_uri(self.image_path());
        crate::spawn_qt_task!(
            self,
            async move {
                tokio::task::spawn_blocking(move || {
                    crate::core::capture::action::CaptureAction::Save.execute(crate::core::capture::action::ActionContext::full_image(resolved_path))
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

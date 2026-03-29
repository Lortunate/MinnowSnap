use crate::core::{i18n, ocr_service};
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{error, info};

#[cxx_qt::bridge]
mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, enabled)]
        #[qproperty(bool, is_downloading)]
        #[qproperty(f32, download_progress)]
        #[qproperty(bool, is_model_ready)]
        #[qproperty(QString, status_message)]
        type OcrManager = super::OcrManagerRust;

        #[qinvokable]
        fn init(self: Pin<&mut Self>);

        #[qinvokable]
        fn set_ocr_enabled_persist(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        fn check_status(self: Pin<&mut Self>);

        #[qinvokable]
        fn download_models(self: Pin<&mut Self>);
    }

    impl cxx_qt::Threading for OcrManager {}
}

#[derive(Default)]
pub struct OcrManagerRust {
    enabled: bool,
    is_downloading: bool,
    download_progress: f32,
    is_model_ready: bool,
    status_message: QString,
}

impl qobject::OcrManager {
    pub fn init(mut self: Pin<&mut Self>) {
        let enabled = ocr_service::is_enabled();
        self.as_mut().set_enabled(enabled);
        info!("OCR Manager initialized. Enabled: {}", enabled);
        self.check_status();
    }

    pub fn set_ocr_enabled_persist(mut self: Pin<&mut Self>, enabled: bool) {
        info!("Setting OCR enabled to: {}", enabled);
        self.as_mut().set_enabled(enabled);
        ocr_service::set_enabled(enabled);
        if enabled {
            self.check_status();
        }
    }

    pub fn check_status(mut self: Pin<&mut Self>) {
        let status = ocr_service::current_status(
            *self.is_downloading(),
            (*self.download_progress() * 100.0).round().clamp(0.0, 100.0) as u8,
            None,
        );
        self.as_mut().set_is_model_ready(matches!(status, ocr_service::OcrModelStatus::Ready));
        self.as_mut().set_status_message(ocr_status_message(&status));
    }

    pub fn download_models(mut self: Pin<&mut Self>) {
        if *self.is_downloading() {
            return;
        }

        self.as_mut().set_is_downloading(true);
        self.as_mut().set_download_progress(0.0);
        let starting = ocr_service::OcrModelStatus::Downloading { progress_percent: 0 };
        self.as_mut().set_status_message(ocr_status_message(&starting));
        info!("Starting OCR model download...");

        let qt_thread_progress = self.qt_thread();

        let progress_cb = Arc::new(move |p: f32| {
            let qt_thread = qt_thread_progress.clone();
            let _ = qt_thread.queue(move |mut qobject: Pin<&mut qobject::OcrManager>| {
                qobject.as_mut().set_download_progress(p);
                let status = ocr_service::OcrModelStatus::Downloading {
                    progress_percent: (p * 100.0).round().clamp(0.0, 100.0) as u8,
                };
                qobject.as_mut().set_status_message(ocr_status_message(&status));
            });
        });

        crate::spawn_qt_task!(
            self,
            async move { ocr_service::download_mobile_models(true, Some(progress_cb)).await },
            |mut qobject: Pin<&mut qobject::OcrManager>, result| {
                qobject.as_mut().set_is_downloading(false);
                match result {
                    Ok(_) => {
                        info!("OCR model download completed successfully");
                        qobject.as_mut().set_is_model_ready(true);
                        qobject.as_mut().set_download_progress(1.0);
                        qobject
                            .as_mut()
                            .set_status_message(ocr_status_message(&ocr_service::OcrModelStatus::Ready));
                    }
                    Err(e) => {
                        error!("Download failed: {}", e);
                        qobject.as_mut().set_is_model_ready(false);
                        qobject.as_mut().set_status_message(ocr_status_message(&ocr_service::OcrModelStatus::Failed {
                            message: e,
                        }));
                    }
                }
            }
        );
    }
}

fn ocr_status_message(status: &ocr_service::OcrModelStatus) -> QString {
    let text = match status {
        ocr_service::OcrModelStatus::Missing => i18n::preferences::ocr_status_missing(),
        ocr_service::OcrModelStatus::Downloading { progress_percent } => {
            i18n::preferences::ocr_status_downloading(*progress_percent)
        }
        ocr_service::OcrModelStatus::Ready => i18n::preferences::ocr_status_ready(),
        ocr_service::OcrModelStatus::Failed { message } => i18n::preferences::ocr_status_failed(message.clone()),
    };
    QString::from(&text)
}

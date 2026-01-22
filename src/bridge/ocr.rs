use crate::core::settings::SETTINGS;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use log::error;
use ocr::OcrModelType;
use std::pin::Pin;
use std::sync::Arc;

#[cxx_qt::bridge]
mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, enabled)]
        #[qproperty(bool, is_downloading, cxx_name = "isDownloading")]
        #[qproperty(f32, download_progress, cxx_name = "downloadProgress")]
        #[qproperty(bool, is_model_ready, cxx_name = "isModelReady")]
        #[qproperty(QString, status_message, cxx_name = "statusMessage")]
        type OcrManager = super::OcrManagerRust;

        #[qinvokable]
        fn init(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "setOcrEnabledPersist"]
        fn set_ocr_enabled_persist(self: Pin<&mut Self>, enabled: bool);

        #[qinvokable]
        #[cxx_name = "checkStatus"]
        fn check_status(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "downloadModels"]
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
        let settings = SETTINGS.lock().unwrap().get();
        self.as_mut().set_enabled(settings.ocr.enabled);
        self.check_status();
    }

    pub fn set_ocr_enabled_persist(mut self: Pin<&mut Self>, enabled: bool) {
        self.as_mut().set_enabled(enabled);
        SETTINGS.lock().unwrap().set_ocr_enabled(enabled);
        if enabled {
            self.check_status();
        }
    }

    pub fn check_status(mut self: Pin<&mut Self>) {
        let ready = ocr::check_models_ready(OcrModelType::Mobile);
        self.as_mut().set_is_model_ready(ready);
        if ready {
            self.as_mut().set_status_message(QString::from("Ready"));
        } else if !self.is_downloading() {
            self.as_mut().set_status_message(QString::from("Models not found"));
        }
    }

    pub fn download_models(mut self: Pin<&mut Self>) {
        if *self.is_downloading() {
            return;
        }

        self.as_mut().set_is_downloading(true);
        self.as_mut().set_download_progress(0.0);
        self.as_mut().set_status_message(QString::from("Starting download..."));

        let qt_thread = self.qt_thread();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

            let progress_qt_thread = qt_thread.clone();
            let progress_cb = Arc::new(move |p: f32| {
                let qt_thread = progress_qt_thread.clone();
                let _ = qt_thread.queue(move |mut qobject: Pin<&mut qobject::OcrManager>| {
                    qobject.as_mut().set_download_progress(p);
                    let percent = (p * 100.0) as i32;
                    qobject
                        .as_mut()
                        .set_status_message(QString::from(&format!("Downloading... {}%", percent)));
                });
            });

            // Force Mobile models as per requirement
            let result = rt.block_on(ocr::download_models(OcrModelType::Mobile, true, Some(progress_cb)));

            let _ = qt_thread.queue(move |mut qobject: Pin<&mut qobject::OcrManager>| {
                qobject.as_mut().set_is_downloading(false);
                match result {
                    Ok(_) => {
                        qobject.as_mut().set_is_model_ready(true);
                        qobject.as_mut().set_download_progress(1.0);
                        qobject.as_mut().set_status_message(QString::from("Download complete"));
                    }
                    Err(e) => {
                        error!("Download failed: {}", e);
                        qobject.as_mut().set_status_message(QString::from("Download failed"));
                    }
                }
            });
        });
    }
}

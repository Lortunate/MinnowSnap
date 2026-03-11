use crate::core::geometry::normalize_polygon;
use crate::core::ocr::{OcrBlock, format_selected_blocks};
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use ocr::{OcrContext, OcrModelType};
use std::pin::Pin;
use tracing::error;

#[cxx_qt::bridge]
mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, is_processing, cxx_name = "isProcessing")]
        #[qproperty(QString, ocr_data_json, cxx_name = "ocrDataJson")]
        type OcrController = super::OcrControllerRust;

        #[qinvokable]
        #[cxx_name = "recognizeImage"]
        fn recognize_image(self: Pin<&mut Self>, image_path: QString);

        #[qinvokable]
        #[cxx_name = "copySelectedText"]
        fn copy_selected_text(self: Pin<&mut Self>, selected_indices_json: QString);
    }

    impl cxx_qt::Threading for OcrController {}
}

#[derive(Default)]
pub struct OcrControllerRust {
    is_processing: bool,
    ocr_data_json: QString,
}

impl qobject::OcrController {
    pub fn recognize_image(mut self: Pin<&mut Self>, image_path: QString) {
        if *self.is_processing() {
            return;
        }

        let path_str = image_path.to_string();
        self.as_mut().set_is_processing(true);
        self.as_mut().set_ocr_data_json(QString::from("[]"));

        crate::spawn_qt_task!(
            self,
            async move {
                let res = async {
                    let clean_path = crate::core::io::storage::clean_url_path(&path_str);
                    let image = tokio::task::spawn_blocking(move || image::open(&clean_path).map_err(|e| e.to_string()))
                        .await
                        .map_err(|e| e.to_string())??;

                    let (img_w, img_h) = (image.width() as f64, image.height() as f64);
                    let mut context = OcrContext::new(None::<std::path::PathBuf>, Some(OcrModelType::Mobile), None, None, None, None)
                        .await
                        .map_err(|e| e.to_string())?;

                    let ocr_results = tokio::task::spawn_blocking(move || context.recognize(&image).map_err(|e| e.to_string()))
                        .await
                        .map_err(|e| e.to_string())??;

                    let blocks: Vec<OcrBlock> = ocr_results
                        .into_iter()
                        .map(|res| {
                            let rect = normalize_polygon(&res.box_points, img_w, img_h);
                            OcrBlock {
                                text: res.text,
                                cx: rect.cx,
                                cy: rect.cy,
                                width: rect.width,
                                height: rect.height,
                                angle: rect.angle,
                                percentage_coordinates: true,
                            }
                        })
                        .collect();

                    serde_json::to_string(&blocks).map_err(|e| e.to_string())
                }
                .await;

                res.unwrap_or_else(|e| {
                    error!("OCR error: {}", e);
                    "[]".to_string()
                })
            },
            |mut qobject: Pin<&mut qobject::OcrController>, json_output| {
                qobject.as_mut().set_ocr_data_json(QString::from(&json_output));
                qobject.as_mut().set_is_processing(false);
            }
        );
    }

    pub fn copy_selected_text(self: Pin<&mut Self>, selected_indices_json: QString) {
        let indices_json = selected_indices_json.to_string();
        let data_json = self.ocr_data_json().to_string();

        crate::spawn_qt_task!(
            self,
            async move {
                tokio::task::spawn_blocking(move || {
                    let indices: Vec<usize> = serde_json::from_str(&indices_json).unwrap_or_default();
                    let blocks: Vec<OcrBlock> = serde_json::from_str(&data_json).unwrap_or_default();
                    format_selected_blocks(&blocks, &indices)
                })
                .await
                .unwrap_or(None)
            },
            |_qobject, result| {
                if let Some(text) = result {
                    crate::core::io::clipboard::copy_text_to_clipboard(text);
                    crate::notify_tr!("ScreenCapture", "Success", "Text copied to clipboard", Copy);
                }
            }
        );
    }
}

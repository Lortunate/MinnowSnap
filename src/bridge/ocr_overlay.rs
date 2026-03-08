use crate::core::geometry::normalize_polygon;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use ocr::{OcrContext, OcrModelType};
use serde::Serialize;
use std::pin::Pin;
use tracing::{error, info};

#[derive(Serialize)]
struct OcrBlock {
    text: String,
    cx: f64,
    cy: f64,
    width: f64,
    height: f64,
    angle: f64,
    percentage_coordinates: bool,
}

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
        type OcrViewModel = super::OcrViewModelRust;

        #[qinvokable]
        #[cxx_name = "recognizeImage"]
        fn recognize_image(self: Pin<&mut Self>, image_path: QString);
    }

    impl cxx_qt::Threading for OcrViewModel {}
}

#[derive(Default)]
pub struct OcrViewModelRust {
    is_processing: bool,
    ocr_data_json: QString,
}

impl qobject::OcrViewModel {
    pub fn recognize_image(mut self: Pin<&mut Self>, image_path: QString) {
        if *self.is_processing() {
            info!("OCR is already processing");
            return;
        }

        let path_str = image_path.to_string();
        info!("recognize_image called with path: {}", path_str);

        self.as_mut().set_is_processing(true);
        self.as_mut().set_ocr_data_json(QString::from("[]"));

        crate::spawn_qt_task!(self, async move {
            let res = async {
                let clean_path = crate::core::io::storage::clean_url_path(&path_str);
                info!("Loading image from: {}", clean_path);
                
                let image = tokio::task::spawn_blocking(move || {
                    image::open(&clean_path).map_err(|e| e.to_string())
                }).await.map_err(|e| e.to_string())??;
                
                let (img_w, img_h) = (image.width() as f64, image.height() as f64);

                let mut context = OcrContext::new(
                    None::<std::path::PathBuf>,
                    Some(OcrModelType::Mobile),
                    None,
                    None,
                    None,
                    None,
                ).await.map_err(|e| e.to_string())?;

                info!("Starting OCR recognition...");
                let ocr_results = tokio::task::spawn_blocking(move || {
                    context.recognize(&image).map_err(|e| e.to_string())
                }).await.map_err(|e| e.to_string())??;
                
                info!("OCR finished. Found {} blocks", ocr_results.len());

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

                let json = serde_json::to_string(&blocks).map_err(|e| e.to_string())?;
                Ok::<String, String>(json)
            }.await;

            match res {
                Ok(json) => {
                    info!("OCR success, JSON length: {}", json.len());
                    json
                }
                Err(e) => {
                    error!("OCR processing failed: {}", e);
                    "[]".to_string()
                }
            }
        }, |mut qobject: Pin<&mut qobject::OcrViewModel>, json_output| {
            qobject.as_mut().set_ocr_data_json(QString::from(&json_output));
            qobject.as_mut().set_is_processing(false);
        });
    }
}

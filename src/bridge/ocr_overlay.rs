use cxx_qt::Threading;
use cxx_qt_lib::QString;
use log::{error, info};
use ocr::{OcrContext, OcrModelType};
use serde::Serialize;
use std::f64::consts::PI;
use std::pin::Pin;
use std::thread;

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

        let qt_thread = self.qt_thread();

        thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let clean_path = if path_str.starts_with("file://") { &path_str[7..] } else { &path_str };

                info!("Loading image from: {}", clean_path);
                let image = image::open(clean_path).map_err(|e| e.to_string())?;
                let (img_w, img_h) = (image.width() as f64, image.height() as f64);

                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| e.to_string())?;

                let mut context = rt
                    .block_on(OcrContext::new(
                        None::<std::path::PathBuf>,
                        Some(OcrModelType::Mobile),
                        None,
                        None,
                        None,
                        None,
                    ))
                    .map_err(|e| e.to_string())?;

                info!("Starting OCR recognition...");
                let ocr_results = context.recognize(&image).map_err(|e| e.to_string())?;
                info!("OCR finished. Found {} blocks", ocr_results.len());

                let blocks: Vec<OcrBlock> = ocr_results
                    .into_iter()
                    .map(|res| {
                        let pts = &res.box_points;
                        if pts.len() != 4 {
                            let mut min_x = i32::MAX;
                            let mut min_y = i32::MAX;
                            let mut max_x = i32::MIN;
                            let mut max_y = i32::MIN;
                            for (x, y) in pts {
                                if *x < min_x {
                                    min_x = *x;
                                }
                                if *x > max_x {
                                    max_x = *x;
                                }
                                if *y < min_y {
                                    min_y = *y;
                                }
                                if *y > max_y {
                                    max_y = *y;
                                }
                            }
                            let x = min_x as f64;
                            let y = min_y as f64;
                            let w = (max_x - min_x) as f64;
                            let h = (max_y - min_y) as f64;

                            return OcrBlock {
                                text: res.text,
                                cx: (x + w / 2.0) / img_w,
                                cy: (y + h / 2.0) / img_h,
                                width: w / img_w,
                                height: h / img_h,
                                angle: 0.0,
                                percentage_coordinates: true,
                            };
                        }

                        let p0 = (pts[0].0 as f64, pts[0].1 as f64);
                        let p1 = (pts[1].0 as f64, pts[1].1 as f64);
                        let p2 = (pts[2].0 as f64, pts[2].1 as f64);
                        let p3 = (pts[3].0 as f64, pts[3].1 as f64);

                        let w_top = ((p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2)).sqrt();
                        let w_bot = ((p2.0 - p3.0).powi(2) + (p2.1 - p3.1).powi(2)).sqrt();
                        let w = (w_top + w_bot) / 2.0;

                        let h_left = ((p3.0 - p0.0).powi(2) + (p3.1 - p0.1).powi(2)).sqrt();
                        let h_right = ((p2.0 - p1.0).powi(2) + (p2.1 - p1.1).powi(2)).sqrt();
                        let h = (h_left + h_right) / 2.0;

                        let cx = (p0.0 + p1.0 + p2.0 + p3.0) / 4.0;
                        let cy = (p0.1 + p1.1 + p2.1 + p3.1) / 4.0;

                        let dx = p1.0 - p0.0;
                        let dy = p1.1 - p0.1;
                        let angle_rad = dy.atan2(dx);
                        let angle_deg = angle_rad * 180.0 / PI;

                        OcrBlock {
                            text: res.text,
                            cx: cx / img_w,
                            cy: cy / img_h,
                            width: w / img_w,
                            height: h / img_h,
                            angle: angle_deg,
                            percentage_coordinates: true,
                        }
                    })
                    .collect();

                let json = serde_json::to_string(&blocks).map_err(|e| e.to_string())?;
                Ok(json)
            })();

            let json_output = match result {
                Ok(json) => {
                    info!("OCR success, JSON length: {}", json.len());
                    json
                }
                Err(e) => {
                    error!("OCR processing failed: {}", e);
                    "[]".to_string()
                }
            };

            let _ = qt_thread.queue(move |mut qobject: Pin<&mut qobject::OcrViewModel>| {
                qobject.as_mut().set_ocr_data_json(QString::from(&json_output));
                qobject.as_mut().set_is_processing(false);
            });
        });
    }
}

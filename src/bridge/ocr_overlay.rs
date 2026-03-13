use crate::core::geometry::normalize_polygon;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use ocr::{OcrContext, OcrModelType};
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tracing::error;

#[derive(Serialize, Deserialize, Clone)]
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

                    if indices.is_empty() || blocks.is_empty() {
                        return None;
                    }

                    let mut selected_blocks: Vec<OcrBlock> = indices.iter().filter_map(|&i| blocks.get(i).cloned()).collect();

                    selected_blocks.sort_by(|a, b| {
                        if (a.cy - b.cy).abs() < 0.01 {
                            a.cx.partial_cmp(&b.cx).unwrap()
                        } else {
                            a.cy.partial_cmp(&b.cy).unwrap()
                        }
                    });

                    let mut result = String::new();
                    let mut prev_block: Option<OcrBlock> = None;

                    for curr_block in selected_blocks {
                        if let Some(prev) = prev_block {
                            let prev_bottom = prev.cy + prev.height / 2.0;
                            let curr_top = curr_block.cy - curr_block.height / 2.0;
                            let gap = curr_top - prev_bottom;
                            let avg_height = (prev.height + curr_block.height) / 2.0;

                            let is_list_item = curr_block
                                .text
                                .trim_start()
                                .starts_with(|c: char| c.is_digit(10) || c == '-' || c == '•' || c == '*');

                            if prev.text.ends_with('-') {
                                result.pop();
                                result.push_str(&curr_block.text);
                            } else if gap > avg_height * 0.5 || is_list_item {
                                result.push('\n');
                                result.push_str(&curr_block.text);
                            } else {
                                let last_char = prev.text.chars().last().unwrap_or(' ');
                                let first_char = curr_block.text.chars().next().unwrap_or(' ');
                                if is_cjk(last_char) && is_cjk(first_char) {
                                    result.push_str(&curr_block.text);
                                } else {
                                    result.push(' ');
                                    result.push_str(&curr_block.text);
                                }
                            }
                        } else {
                            result.push_str(&curr_block.text);
                        }
                        prev_block = Some(curr_block);
                    }
                    Some(result)
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

fn is_cjk(c: char) -> bool {
    (c >= '\u{3000}' && c <= '\u{303f}')
        || (c >= '\u{3040}' && c <= '\u{309f}')
        || (c >= '\u{30a0}' && c <= '\u{30ff}')
        || (c >= '\u{ff00}' && c <= '\u{ff9f}')
        || (c >= '\u{4e00}' && c <= '\u{9faf}')
        || (c >= '\u{3400}' && c <= '\u{4dbf}')
}

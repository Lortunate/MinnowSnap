use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum OcrModelType {
    #[default]
    Server,
    Mobile,
}

pub struct ModelSource<'a> {
    pub det_url: &'a str,
    pub det_name: &'a str,
    pub rec_url: &'a str,
    pub rec_name: &'a str,
}

pub const APP_DATA_DIR: &str = "MinnowSnap";
pub const MODEL_DIR: &str = "ocr_models";

pub const KEYS_URL: &str = "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/main/ppocr/utils/dict/ppocrv5_dict.txt";
pub const KEYS_NAME: &str = "ppocrv5_dict.txt";

pub const SERVER_MODELS: ModelSource = ModelSource {
    det_url: "https://modelscope.cn/models/RapidAI/RapidOCR/resolve/master/onnx/PP-OCRv5/det/ch_PP-OCRv5_server_det.onnx",
    det_name: "ch_PP-OCRv5_server_det.onnx",
    rec_url: "https://modelscope.cn/models/RapidAI/RapidOCR/resolve/master/onnx/PP-OCRv5/rec/ch_PP-OCRv5_rec_server_infer.onnx",
    rec_name: "ch_PP-OCRv5_rec_server_infer.onnx",
};

pub const MOBILE_MODELS: ModelSource = ModelSource {
    det_url: "https://modelscope.cn/models/RapidAI/RapidOCR/resolve/master/onnx/PP-OCRv5/det/ch_PP-OCRv5_mobile_det.onnx",
    det_name: "ch_PP-OCRv5_mobile_det.onnx",
    rec_url: "https://modelscope.cn/models/RapidAI/RapidOCR/resolve/master/onnx/PP-OCRv5/rec/ch_PP-OCRv5_rec_mobile_infer.onnx",
    rec_name: "ch_PP-OCRv5_rec_mobile_infer.onnx",
};

#[derive(Clone, Debug)]
pub struct OcrConfig {
    pub model_type: OcrModelType,

    pub det_model_path: PathBuf,
    pub rec_model_path: PathBuf,
    pub keys_path: PathBuf,

    pub threads: usize,

    pub limit_side_len: f32,
    pub det_thresh: f32,
    pub det_box_padding: i32,

    pub rec_img_h: u32,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            model_type: OcrModelType::default(),
            det_model_path: PathBuf::default(),
            rec_model_path: PathBuf::default(),
            keys_path: PathBuf::default(),
            threads: 4,
            limit_side_len: 960.0,
            det_thresh: 0.3,
            det_box_padding: 2,
            rec_img_h: 48,
        }
    }
}

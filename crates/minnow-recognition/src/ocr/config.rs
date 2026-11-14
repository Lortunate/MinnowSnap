use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct OcrConfig {
    pub det_model_path: PathBuf,
    pub rec_model_path: PathBuf,
    pub keys_path: PathBuf,
    pub dictionary_entries: usize,
    pub dictionary_sha256: String,

    pub threads: usize,

    pub limit_side_len: f32,
    pub det_thresh: f32,
    pub det_box_padding: i32,

    pub rec_img_h: u32,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            det_model_path: PathBuf::default(),
            rec_model_path: PathBuf::default(),
            keys_path: PathBuf::default(),
            dictionary_entries: 0,
            dictionary_sha256: String::new(),
            threads: 4,
            limit_side_len: 960.0,
            det_thresh: 0.3,
            det_box_padding: 2,
            rec_img_h: 48,
        }
    }
}

pub mod config;
pub mod detector;
pub mod engine;
pub mod model_manager;
pub mod preprocess;
pub mod recognizer;
pub mod service;
pub mod visualization;

pub use config::{OcrConfig, OcrModelType};
pub use engine::{OcrEngine, OcrResult};
pub use model_manager::ModelManager;

use anyhow::Result;
use image::DynamicImage;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

type SharedProgressCallback = Arc<dyn Fn(f32) + Send + Sync>;

fn model_source(model_type: OcrModelType) -> config::ModelSource<'static> {
    match model_type {
        OcrModelType::Server => config::SERVER_MODELS,
        OcrModelType::Mobile => config::MOBILE_MODELS,
    }
}

fn model_progress_callback(
    on_progress: Option<SharedProgressCallback>,
    progress_state: Arc<Mutex<[f32; 3]>>,
    index: usize,
) -> Option<model_manager::ProgressCallback> {
    on_progress.map(|main_cb| {
        Box::new(move |progress: f32| {
            if let Ok(mut guard) = progress_state.lock() {
                guard[index] = progress;
                let avg = guard.iter().sum::<f32>() / 3.0;
                main_cb(avg);
            }
        }) as model_manager::ProgressCallback
    })
}

async fn ensure_required_models(
    manager: &ModelManager,
    source: config::ModelSource<'static>,
    force: bool,
    on_progress: Option<SharedProgressCallback>,
) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let progress_state = Arc::new(Mutex::new([0.0f32; 3]));
    let det_callback = model_progress_callback(on_progress.clone(), progress_state.clone(), 0);
    let rec_callback = model_progress_callback(on_progress.clone(), progress_state.clone(), 1);
    let keys_callback = model_progress_callback(on_progress, progress_state, 2);

    tokio::try_join!(
        manager.ensure_model(source.det_url, source.det_name, force, det_callback),
        manager.ensure_model(source.rec_url, source.rec_name, force, rec_callback),
        manager.ensure_model(config::KEYS_URL, config::KEYS_NAME, force, keys_callback)
    )
}

pub fn build_ocr_blocks(ocr_results: Vec<OcrResult>, img_w: f64, img_h: f64) -> Vec<OcrBlock> {
    ocr_results
        .into_iter()
        .map(|res| {
            let rect = crate::services::geometry::normalize_polygon(&res.box_points, img_w, img_h);
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
        .collect()
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct OcrBlock {
    pub text: String,
    pub cx: f64,
    pub cy: f64,
    pub width: f64,
    pub height: f64,
    pub angle: f64,
    pub percentage_coordinates: bool,
}

pub fn is_cjk(c: char) -> bool {
    ('\u{3000}'..='\u{303f}').contains(&c)
        || ('\u{3040}'..='\u{309f}').contains(&c)
        || ('\u{30a0}'..='\u{30ff}').contains(&c)
        || ('\u{ff00}'..='\u{ff9f}').contains(&c)
        || ('\u{4e00}'..='\u{9faf}').contains(&c)
        || ('\u{3400}'..='\u{4dbf}').contains(&c)
}

pub fn format_selected_blocks(blocks: &[OcrBlock], indices: &[usize]) -> Option<String> {
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
                .starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '•' || c == '*');

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
}

pub fn default_model_manager() -> Result<ModelManager> {
    Ok(ModelManager::new(crate::services::paths::app_paths().ocr_models_dir()))
}

pub struct OcrContext {
    engine: OcrEngine,
}

impl OcrContext {
    pub async fn new<P: AsRef<Path>>(models_dir: Option<P>, model_type: OcrModelType, on_progress: Option<SharedProgressCallback>) -> Result<Self> {
        let save_dir = models_dir
            .map(|p| p.as_ref().to_path_buf())
            .unwrap_or(crate::services::paths::app_paths().ocr_models_dir().to_path_buf());
        let manager = ModelManager::new(&save_dir);
        let source = model_source(model_type);

        let (det_path, rec_path, keys_path) = ensure_required_models(&manager, source, false, on_progress).await?;

        let config = OcrConfig {
            model_type,
            det_model_path: det_path,
            rec_model_path: rec_path,
            keys_path,
            ..Default::default()
        };

        let engine = tokio::task::spawn_blocking(move || OcrEngine::new(config)).await??;

        Ok(Self { engine })
    }

    pub fn recognize(&mut self, image: &DynamicImage) -> Result<Vec<OcrResult>> {
        self.engine.ocr(image)
    }
}

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

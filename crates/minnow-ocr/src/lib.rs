pub mod config;
pub mod detector;
pub mod engine;
pub mod model_manager;
pub mod preprocess;
pub mod recognizer;
pub mod visualization;

pub use config::{OcrConfig, OcrModelType};
pub use engine::{OcrEngine, OcrResult};
pub use model_manager::ModelManager;

use crate::config::{KEYS_NAME, KEYS_URL, MOBILE_MODELS, ModelSource, SERVER_MODELS};
use crate::model_manager::ProgressCallback;
use anyhow::Result;
use image::DynamicImage;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

type SharedProgressCallback = Arc<dyn Fn(f32) + Send + Sync>;

fn model_source(model_type: OcrModelType) -> ModelSource<'static> {
    match model_type {
        OcrModelType::Server => SERVER_MODELS,
        OcrModelType::Mobile => MOBILE_MODELS,
    }
}

fn model_progress_callback(
    on_progress: Option<SharedProgressCallback>,
    progress_state: Arc<Mutex<[f32; 3]>>,
    index: usize,
) -> Option<ProgressCallback> {
    on_progress.map(|main_cb| {
        Box::new(move |progress: f32| {
            if let Ok(mut guard) = progress_state.lock() {
                guard[index] = progress;
                let avg = guard.iter().sum::<f32>() / 3.0;
                main_cb(avg);
            }
        }) as ProgressCallback
    })
}

async fn ensure_required_models(
    manager: &ModelManager,
    source: ModelSource<'static>,
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
        manager.ensure_model(KEYS_URL, KEYS_NAME, force, keys_callback)
    )
}

pub fn check_models_ready(model_type: OcrModelType) -> bool {
    let save_dir = match ModelManager::default_dir() {
        Ok(d) => d,
        Err(_) => return false,
    };
    let manager = ModelManager::new(save_dir);
    let source = model_source(model_type);

    manager.check_models_existence(&[source.det_name, source.rec_name, KEYS_NAME])
}

pub async fn download_models(model_type: OcrModelType, force: bool, on_progress: Option<SharedProgressCallback>) -> Result<()> {
    let save_dir = ModelManager::default_dir()?;
    let manager = ModelManager::new(save_dir);
    let source = model_source(model_type);

    let _ = ensure_required_models(&manager, source, force, on_progress).await?;

    Ok(())
}

pub struct OcrContext {
    engine: OcrEngine,
}

impl OcrContext {
    pub async fn new<P: AsRef<Path>>(models_dir: Option<P>, model_type: OcrModelType, on_progress: Option<SharedProgressCallback>) -> Result<Self> {
        let save_dir = models_dir.map(|p| p.as_ref().to_path_buf()).unwrap_or(ModelManager::default_dir()?);
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

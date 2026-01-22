pub mod config;
pub mod detector;
pub mod engine;
pub mod model_manager;
pub mod recognizer;
pub mod utils;

pub use config::{OcrConfig, OcrModelType};
pub use engine::{OcrEngine, OcrResult};
pub use model_manager::ModelManager;

use crate::config::{ModelSource, KEYS_NAME, KEYS_URL, MOBILE_MODELS, SERVER_MODELS};
use anyhow::Result;
use image::DynamicImage;
use std::path::Path;
use std::sync::{Arc, Mutex};

fn get_model_source(model_type: OcrModelType) -> ModelSource<'static> {
    match model_type {
        OcrModelType::Server => SERVER_MODELS,
        OcrModelType::Mobile => MOBILE_MODELS,
    }
}

pub fn check_models_ready(model_type: OcrModelType) -> bool {
    let save_dir = match ModelManager::default_dir() {
        Ok(d) => d,
        Err(_) => return false,
    };
    let manager = ModelManager::new(save_dir);
    let source = get_model_source(model_type);

    manager.check_models_existence(&[source.det_name, source.rec_name, KEYS_NAME])
}

pub async fn download_models(model_type: OcrModelType, force: bool, on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>) -> Result<()> {
    let save_dir = ModelManager::default_dir()?;
    let manager = ModelManager::new(&save_dir);
    let source = get_model_source(model_type);

    let progress_state = Arc::new(Mutex::new([0.0f32; 3]));
    let create_cb = |idx: usize| {
        on_progress.clone().map(|main_cb| {
            let state = progress_state.clone();
            Box::new(move |p: f32| {
                if let Ok(mut guard) = state.lock() {
                    guard[idx] = p;
                    let avg = guard.iter().sum::<f32>() / 3.0;
                    main_cb(avg);
                }
            }) as Box<dyn Fn(f32) + Send + Sync>
        })
    };

    let _ = tokio::try_join!(
        manager.ensure_model(source.det_url, source.det_name, force, create_cb(0)),
        manager.ensure_model(source.rec_url, source.rec_name, force, create_cb(1)),
        manager.ensure_model(KEYS_URL, KEYS_NAME, force, create_cb(2))
    )?;

    Ok(())
}

pub struct OcrContext {
    engine: OcrEngine,
}

impl OcrContext {
    pub async fn new<P: AsRef<Path>>(
        models_dir: Option<P>,
        model_type: Option<OcrModelType>,
        det_url: Option<&str>,
        rec_url: Option<&str>,
        keys_url: Option<&str>,
        on_progress: Option<Arc<dyn Fn(f32) + Send + Sync>>,
    ) -> Result<Self> {
        let save_dir = models_dir.map(|p| p.as_ref().to_path_buf()).unwrap_or(ModelManager::default_dir()?);
        let manager = ModelManager::new(&save_dir);
        let model_type = model_type.unwrap_or_default();
        let source = get_model_source(model_type);

        let det_url = det_url.unwrap_or(source.det_url);
        let rec_url = rec_url.unwrap_or(source.rec_url);
        let keys_url = keys_url.unwrap_or(KEYS_URL);

        let progress_state = Arc::new(Mutex::new([0.0f32; 3]));
        let create_cb = |idx: usize| {
            on_progress.clone().map(|main_cb| {
                let state = progress_state.clone();
                Box::new(move |p: f32| {
                    if let Ok(mut guard) = state.lock() {
                        guard[idx] = p;
                        let avg = guard.iter().sum::<f32>() / 3.0;
                        main_cb(avg);
                    }
                }) as Box<dyn Fn(f32) + Send + Sync>
            })
        };

        let (det_path, rec_path, keys_path) = tokio::try_join!(
            manager.ensure_model(det_url, source.det_name, false, create_cb(0)),
            manager.ensure_model(rec_url, source.rec_name, false, create_cb(1)),
            manager.ensure_model(keys_url, KEYS_NAME, false, create_cb(2))
        )?;

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

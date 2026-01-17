pub mod config;
pub mod detector;
pub mod engine;
pub mod model_manager;
pub mod recognizer;
pub mod utils;

pub use config::{OcrConfig, OcrModelType};
pub use engine::{OcrEngine, OcrResult};
pub use model_manager::ModelManager;

use crate::config::{KEYS_NAME, KEYS_URL, MOBILE_MODELS, SERVER_MODELS};
use anyhow::Result;
use image::DynamicImage;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub fn are_models_present() -> bool {
    let save_dir = match ModelManager::default_dir() {
        Ok(d) => d,
        Err(_) => return false,
    };
    let manager = ModelManager::new(save_dir);
    manager.check_models_existence(&[SERVER_MODELS.det_name, SERVER_MODELS.rec_name, KEYS_NAME])
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

        let (def_det_url, def_det_name, def_rec_url, def_rec_name) = match model_type {
            OcrModelType::Server => (
                SERVER_MODELS.det_url,
                SERVER_MODELS.det_name,
                SERVER_MODELS.rec_url,
                SERVER_MODELS.rec_name,
            ),
            OcrModelType::Mobile => (
                MOBILE_MODELS.det_url,
                MOBILE_MODELS.det_name,
                MOBILE_MODELS.rec_url,
                MOBILE_MODELS.rec_name,
            ),
        };

        let det_url = det_url.unwrap_or(def_det_url);
        let rec_url = rec_url.unwrap_or(def_rec_url);
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
            manager.ensure_model(det_url, def_det_name, create_cb(0)),
            manager.ensure_model(rec_url, def_rec_name, create_cb(1)),
            manager.ensure_model(keys_url, KEYS_NAME, create_cb(2))
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

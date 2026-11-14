pub(crate) mod config;
mod contracts;
mod detector;
mod engine;
mod plan;
mod preprocess;
mod recognizer;
mod session;

use image::RgbaImage;
use minnow_application::ports::{ErrorKind, NormalizedOcrGeometry, OcrBlock, OperationError, OperationResult, RecognitionAvailability};
use minnow_imaging::geometry::normalize_polygon;
use ort::environment::Environment;
use std::sync::OnceLock;

use crate::InferenceBudget;
use crate::models::{ArtifactValidator, ModelPaths};

pub(crate) trait InferenceEngine: Send {
    fn recognize(&mut self, image: &RgbaImage) -> OperationResult<Vec<OcrBlock>>;
}

pub(crate) trait EngineFactory: ArtifactValidator {
    fn availability(&self) -> RecognitionAvailability {
        RecognitionAvailability::Available
    }

    fn build(&self, paths: &ModelPaths) -> OperationResult<Box<dyn InferenceEngine>>;
}

pub(crate) struct OnnxEngineFactory {
    threads: usize,
    budget: InferenceBudget,
    environment: OnceLock<OperationResult<Environment>>,
}

impl OnnxEngineFactory {
    pub(crate) fn new(threads: usize, budget: InferenceBudget) -> Self {
        Self {
            threads,
            budget,
            environment: OnceLock::new(),
        }
    }

    fn environment(&self) -> OperationResult<&Environment> {
        // Keep the environment for the recognition service's lifetime. Capability
        // checks initialize it before any model scan or download can begin.
        self.environment
            .get_or_init(|| {
                #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
                {
                    // load-dynamic does not supply a runtime. Although
                    // init_from is fallible, subsequent API setup can panic.
                    // Do not enter ORT until Intel distributions have a
                    // verified runtime and dependency packaging contract.
                    Err(OperationError::new(
                        ErrorKind::Unavailable,
                        "Intel macOS OCR is unavailable: this distribution has no verified ONNX Runtime library and dependency bundle",
                    ))
                }
                #[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
                {
                    ort::init()
                        .with_name("MinnowSnap OCR")
                        .build()
                        .map_err(|error| OperationError::new(ErrorKind::Unavailable, format!("could not initialize ONNX Runtime: {error}")))
                }
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    fn create(&self, paths: &ModelPaths) -> OperationResult<engine::OcrEngine> {
        engine::OcrEngine::new(
            config::OcrConfig {
                det_model_path: paths.detector.clone(),
                rec_model_path: paths.recognizer.clone(),
                keys_path: paths.keys.clone(),
                dictionary_entries: paths.spec.dictionary_entries(),
                dictionary_sha256: paths.spec.dictionary().expected_sha256().into(),
                threads: self.threads,
                ..Default::default()
            },
            self.environment()?,
            self.budget,
        )
        .map_err(|error| {
            OperationError::new(
                ErrorKind::Validation,
                format!("could not load OCR model spec {}: {error}", paths.spec.identity()),
            )
        })
    }
}

impl ArtifactValidator for OnnxEngineFactory {
    fn validate(&self, paths: &ModelPaths) -> OperationResult<()> {
        self.create(paths).map(|_| ())
    }
}

impl EngineFactory for OnnxEngineFactory {
    fn availability(&self) -> RecognitionAvailability {
        match self.environment() {
            Ok(_) => RecognitionAvailability::Available,
            Err(error) => RecognitionAvailability::Unavailable { reason: error.message },
        }
    }

    fn build(&self, paths: &ModelPaths) -> OperationResult<Box<dyn InferenceEngine>> {
        self.create(paths).map(|engine| Box::new(OnnxEngine(engine)) as Box<dyn InferenceEngine>)
    }
}

struct OnnxEngine(engine::OcrEngine);

impl InferenceEngine for OnnxEngine {
    fn recognize(&mut self, image: &RgbaImage) -> OperationResult<Vec<OcrBlock>> {
        let (width, height) = (f64::from(image.width()), f64::from(image.height()));
        if width == 0.0 || height == 0.0 {
            return Err(OperationError::new(ErrorKind::InvalidInput, "OCR input image has no pixels"));
        }
        let results = self
            .0
            .ocr(image)
            .map_err(|error| OperationError::new(ErrorKind::Inference, error.to_string()))?;
        let mut blocks =
            plan::reserved(results.len(), "normalized OCR blocks").map_err(|error| OperationError::new(ErrorKind::Inference, error.to_string()))?;
        for result in results {
            let bounds = normalize_polygon(&result.box_points, width, height);
            let geometry = NormalizedOcrGeometry::new(bounds.cx, bounds.cy, bounds.width, bounds.height, bounds.angle)
                .map_err(|error| OperationError::new(ErrorKind::Inference, format!("invalid OCR output geometry: {error}")))?;
            blocks.push(OcrBlock::new(result.text, geometry));
        }
        Ok(blocks)
    }
}

#[cfg(all(test, not(all(target_os = "macos", target_arch = "x86_64"))))]
mod tests {
    use super::*;

    #[test]
    fn native_runtime_supports_repeated_session_builders_in_one_service() {
        let factory = OnnxEngineFactory::new(1, InferenceBudget::default());
        let first = ort::session::Session::builder(factory.environment().unwrap()).unwrap();
        let second = ort::session::Session::builder(factory.environment().unwrap()).unwrap();
        // Session builders retain the native environment beyond the factory.
        drop(factory);
        first.with_intra_threads(1).unwrap();
        second.with_intra_threads(1).unwrap();
    }
}

#[cfg(all(test, target_os = "macos", target_arch = "x86_64"))]
mod intel_tests {
    use super::*;

    #[test]
    fn intel_ocr_returns_unavailable_before_entering_an_unverified_native_runtime() {
        // This must also work without ORT_DYLIB_PATH or a runtime dylib. There
        // is no catch_unwind fallback: release builds use panic=abort.
        let factory = OnnxEngineFactory::new(1, InferenceBudget::default());
        let error = factory.environment().unwrap_err();
        assert_eq!(error.kind, ErrorKind::Unavailable);
        assert!(error.message.contains("no verified ONNX Runtime"));
        assert_eq!(factory.environment().unwrap_err(), error);
    }
}

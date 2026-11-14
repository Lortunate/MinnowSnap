use crate::local::LocalRecognitionService as RecognitionService;
use crate::models::{ArtifactFetcher, ArtifactValidator, HttpFetcher, ModelPaths, ModelStore, Progress};
use crate::ocr::{EngineFactory, InferenceEngine};
use crate::{ArtifactSpec, ModelSources, OcrModelType, RecognitionOptions};
use futures_util::poll;
use image::RgbaImage;
use minnow_application::ports::{
    ErrorKind, ModelDownloadStatus, NormalizedOcrGeometry, OcrBlock, OperationError, OperationFuture, OperationResult, RecognitionAvailability,
    RecognitionModels, TextRecognition,
};
use minnow_imaging::ImageAsset;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use tokio::sync::{Semaphore, watch};
use tokio_util::sync::CancellationToken;

fn artifact(name: &str, url: &str, bytes: &[u8]) -> ArtifactSpec {
    ArtifactSpec::new(
        name,
        url,
        bytes.len() as u64,
        Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
    )
    .unwrap()
}

fn sources() -> ModelSources {
    ModelSources::verified(
        "test-ppocr-v1",
        artifact("det.onnx", "detector", b"valid detector v1"),
        artifact("rec.onnx", "recognizer", b"valid recognizer v1"),
        artifact("keys.txt", "dictionary", b"valid dictionary v1"),
        1,
    )
    .unwrap()
}

struct MemoryFetcher {
    files: Mutex<HashMap<String, Vec<u8>>>,
    fail_url: Mutex<Option<String>>,
    gate_next: AtomicBool,
    gate: Semaphore,
    calls: AtomicUsize,
    started: watch::Sender<usize>,
}

impl MemoryFetcher {
    fn new() -> Arc<Self> {
        let (started, _) = watch::channel(0);
        Arc::new(Self {
            files: Mutex::new(HashMap::from([
                ("detector".into(), b"valid detector v1".to_vec()),
                ("recognizer".into(), b"valid recognizer v1".to_vec()),
                ("dictionary".into(), b"valid dictionary v1".to_vec()),
            ])),
            fail_url: Mutex::new(None),
            gate_next: AtomicBool::new(false),
            gate: Semaphore::new(0),
            calls: AtomicUsize::new(0),
            started,
        })
    }

    async fn wait_for_calls(&self, expected: usize) {
        let mut started = self.started.subscribe();
        while *started.borrow() < expected {
            started.changed().await.unwrap();
        }
    }
}

impl ArtifactFetcher for MemoryFetcher {
    fn fetch<'a>(&'a self, url: &'a str, destination: &'a Path, cancel: CancellationToken, progress: Progress) -> OperationFuture<'a, ()> {
        Box::pin(async move {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            self.started.send_replace(call);
            if self.gate_next.swap(false, Ordering::SeqCst) {
                tokio::select! {
                    _ = cancel.cancelled() => return Err(OperationError::cancelled()),
                    permit = self.gate.acquire() => permit.unwrap().forget(),
                }
            }
            if self.fail_url.lock().unwrap().as_deref() == Some(url) {
                return Err(OperationError::new(ErrorKind::Download, "injected transfer failure"));
            }
            let bytes = self.files.lock().unwrap().get(url).unwrap().clone();
            tokio::fs::write(destination, bytes).await.unwrap();
            progress(1.0);
            Ok(())
        })
    }
}

struct TestFactory {
    runtime_available: AtomicBool,
    builds: AtomicUsize,
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    block_inference: Arc<(Mutex<bool>, Condvar)>,
    inference_started: watch::Sender<usize>,
    input_pointer: Arc<AtomicUsize>,
    block_validation: (Mutex<bool>, Condvar),
    validation_started: watch::Sender<bool>,
}

impl TestFactory {
    fn new() -> Arc<Self> {
        let (inference_started, _) = watch::channel(0);
        let (validation_started, _) = watch::channel(false);
        Arc::new(Self {
            runtime_available: AtomicBool::new(true),
            builds: AtomicUsize::new(0),
            active: Arc::new(AtomicUsize::new(0)),
            peak: Arc::new(AtomicUsize::new(0)),
            block_inference: Arc::new((Mutex::new(false), Condvar::new())),
            inference_started,
            input_pointer: Arc::new(AtomicUsize::new(0)),
            block_validation: (Mutex::new(false), Condvar::new()),
            validation_started,
        })
    }

    fn release(&self) {
        *self.block_inference.0.lock().unwrap() = false;
        self.block_inference.1.notify_all();
    }

    fn release_validation(&self) {
        *self.block_validation.0.lock().unwrap() = false;
        self.block_validation.1.notify_all();
    }

    async fn wait_for_validation(&self) {
        let mut started = self.validation_started.subscribe();
        while !*started.borrow() {
            started.changed().await.unwrap();
        }
    }

    async fn wait_for_inference(&self) {
        let mut started = self.inference_started.subscribe();
        while *started.borrow() == 0 {
            started.changed().await.unwrap();
        }
    }
}

impl ArtifactValidator for TestFactory {
    fn validate(&self, paths: &ModelPaths) -> OperationResult<()> {
        for path in [&paths.detector, &paths.recognizer, &paths.keys] {
            let bytes = fs::read(path).map_err(crate::models::io_error)?;
            if !bytes.starts_with(b"valid ") {
                return Err(OperationError::new(ErrorKind::Validation, "injected invalid model artifact"));
            }
        }
        let mut blocked = self.block_validation.0.lock().unwrap();
        if *blocked {
            self.validation_started.send_replace(true);
        }
        while *blocked {
            blocked = self.block_validation.1.wait(blocked).unwrap();
        }
        Ok(())
    }
}

impl EngineFactory for TestFactory {
    fn availability(&self) -> RecognitionAvailability {
        if self.runtime_available.load(Ordering::SeqCst) {
            RecognitionAvailability::Available
        } else {
            RecognitionAvailability::Unavailable {
                reason: "injected missing runtime".into(),
            }
        }
    }

    fn build(&self, paths: &ModelPaths) -> OperationResult<Box<dyn InferenceEngine>> {
        self.validate(paths)?;
        self.builds.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(TestEngine {
            active: self.active.clone(),
            peak: self.peak.clone(),
            gate: self.block_inference.clone(),
            started: self.inference_started.clone(),
            input_pointer: self.input_pointer.clone(),
        }))
    }
}

struct TestEngine {
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    gate: Arc<(Mutex<bool>, Condvar)>,
    started: watch::Sender<usize>,
    input_pointer: Arc<AtomicUsize>,
}

impl InferenceEngine for TestEngine {
    fn recognize(&mut self, image: &RgbaImage) -> OperationResult<Vec<OcrBlock>> {
        self.input_pointer.store(image.as_raw().as_ptr() as usize, Ordering::SeqCst);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(active, Ordering::SeqCst);
        self.started.send_modify(|count| *count += 1);
        let mut blocked = self.gate.0.lock().unwrap();
        while *blocked {
            blocked = self.gate.1.wait(blocked).unwrap();
        }
        self.active.fetch_sub(1, Ordering::SeqCst);
        Ok(vec![OcrBlock::new(
            "recognized",
            NormalizedOcrGeometry::new(0.5, 0.5, 0.5, 0.2, 0.0).unwrap(),
        )])
    }
}

async fn update_store(
    store: &ModelStore,
    fetcher: &dyn ArtifactFetcher,
    validator: Arc<dyn ArtifactValidator>,
    cancel: CancellationToken,
    progress: Progress,
) -> OperationResult<Arc<crate::models::PreparedModels>> {
    super::update_models(store, &tokio::runtime::Handle::current(), fetcher, validator, cancel, progress).await
}

fn service(root: &Path, fetcher: Arc<MemoryFetcher>, factory: Arc<TestFactory>) -> RecognitionService {
    RecognitionService::with_adapters(
        root.to_path_buf(),
        tokio::runtime::Handle::current(),
        RecognitionOptions {
            sources: Some(sources()),
            ..Default::default()
        },
        fetcher,
        factory,
    )
    .unwrap()
}

async fn finish_offline_scan(service: &RecognitionService) {
    let flight = service.inner.flight.lock().unwrap().clone().unwrap();
    let mut result = flight.result.subscribe();
    while result.borrow().is_none() {
        result.changed().await.unwrap();
    }
}

#[tokio::test]
async fn concurrent_model_requests_share_update_and_one_waiter_cannot_cancel_others() {
    let root = tempfile::tempdir().unwrap();
    let fetcher = MemoryFetcher::new();
    let service = service(root.path(), fetcher.clone(), TestFactory::new());
    finish_offline_scan(&service).await;
    fetcher.gate_next.store(true, Ordering::SeqCst);
    let cancelled_waiter = CancellationToken::new();
    let mut first = Box::pin(service.ensure_models(true, cancelled_waiter.clone()));
    let mut second = Box::pin(service.ensure_models(true, CancellationToken::new()));
    assert!(poll!(&mut first).is_pending());
    assert!(poll!(&mut second).is_pending());
    fetcher.wait_for_calls(1).await;
    cancelled_waiter.cancel();
    assert_eq!(first.await.unwrap_err().kind, ErrorKind::Cancelled);
    assert!(service.model_status().is_downloading());
    fetcher.gate.add_permits(1);
    second.await.unwrap();
    assert_eq!(fetcher.calls.load(Ordering::SeqCst), 3);
    assert!(service.model_status().is_ready());
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn failed_replacement_keeps_active_generation_and_never_exposes_partial_artifacts() {
    let root = tempfile::tempdir().unwrap();
    let fetcher = MemoryFetcher::new();
    let service = service(root.path(), fetcher.clone(), TestFactory::new());
    service.ensure_models(false, CancellationToken::new()).await.unwrap();
    let original = service.model_status().active_generation.unwrap();
    let pointer = fs::read(root.path().join("mobile/active.json")).unwrap();
    *fetcher.fail_url.lock().unwrap() = Some("recognizer".into());
    let error = service.ensure_models(true, CancellationToken::new()).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Download);
    let status = service.model_status();
    assert_eq!(status.active_generation.as_deref(), Some(original.as_str()));
    assert!(matches!(status.download, ModelDownloadStatus::Failed { .. }));
    assert_eq!(fs::read(root.path().join("mobile/active.json")).unwrap(), pointer);
    assert!(
        fs::read_dir(root.path().join("mobile"))
            .unwrap()
            .all(|entry| !entry.unwrap().file_name().to_string_lossy().starts_with(".staging-"))
    );
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn invalid_replacement_cannot_publish_over_valid_models() {
    let root = tempfile::tempdir().unwrap();
    let fetcher = MemoryFetcher::new();
    let service = service(root.path(), fetcher.clone(), TestFactory::new());
    service.ensure_models(false, CancellationToken::new()).await.unwrap();
    let active = service.model_status().active_generation;
    fetcher.files.lock().unwrap().insert("recognizer".into(), b"invalid recognizer".to_vec());
    assert_eq!(
        service.ensure_models(true, CancellationToken::new()).await.unwrap_err().kind,
        ErrorKind::Validation
    );
    assert_eq!(service.model_status().active_generation, active);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn service_shutdown_cancels_update_and_preserves_last_good_generation() {
    let root = tempfile::tempdir().unwrap();
    let fetcher = MemoryFetcher::new();
    let service = service(root.path(), fetcher.clone(), TestFactory::new());
    service.ensure_models(false, CancellationToken::new()).await.unwrap();
    let active = service.model_status().active_generation;
    fetcher.gate_next.store(true, Ordering::SeqCst);
    let mut update = Box::pin(service.ensure_models(true, CancellationToken::new()));
    assert!(poll!(&mut update).is_pending());
    fetcher.wait_for_calls(4).await;
    service.shutdown().await.unwrap();
    assert_eq!(update.await.unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(service.model_status().active_generation, active);
    assert_eq!(service.model_status().download, ModelDownloadStatus::Idle);
}

#[tokio::test]
async fn intact_legacy_files_are_imported_without_network_and_remain_untouched() {
    let root = tempfile::tempdir().unwrap();
    for (name, value) in [
        ("det.onnx", "valid detector v1"),
        ("rec.onnx", "valid recognizer v1"),
        ("keys.txt", "valid dictionary v1"),
    ] {
        fs::write(root.path().join(name), value).unwrap();
    }
    let fetcher = MemoryFetcher::new();
    let service = service(root.path(), fetcher.clone(), TestFactory::new());
    finish_offline_scan(&service).await;
    assert!(service.model_status().is_ready());
    assert_eq!(fetcher.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fs::read_to_string(root.path().join("det.onnx")).unwrap(), "valid detector v1");
    assert!(root.path().join("mobile/active.json").is_file());
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn committed_artifact_checksum_is_verified_and_a_new_generation_can_repair_corruption() {
    let root = tempfile::tempdir().unwrap();
    let factory = TestFactory::new();
    let store = ModelStore::new(root.path().to_path_buf(), OcrModelType::Mobile, sources(), 1024).unwrap();
    let fetcher = MemoryFetcher::new();
    let models = update_store(&store, fetcher.as_ref(), factory.clone(), CancellationToken::new(), Arc::new(|_| {}))
        .await
        .unwrap();
    fs::write(&models.paths.detector, b"valid but changed detector").unwrap();
    let error = store.load_existing(factory.as_ref(), &CancellationToken::new()).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Validation);
    let repaired = update_store(&store, fetcher.as_ref(), factory.clone(), CancellationToken::new(), Arc::new(|_| {}))
        .await
        .unwrap();
    assert_ne!(models.generation, repaired.generation);
    let loaded = store.load_existing(factory.as_ref(), &CancellationToken::new()).unwrap().unwrap();
    assert_eq!(loaded.generation, repaired.generation);
    let duplicate = update_store(&store, fetcher.as_ref(), factory, CancellationToken::new(), Arc::new(|_| {}))
        .await
        .unwrap();
    assert_eq!(duplicate.generation, repaired.generation);
}

#[tokio::test]
async fn generation_retention_deduplicates_content_and_preserves_only_rollback_live_leases_and_unknown_files() {
    let root = tempfile::tempdir().unwrap();
    let factory = TestFactory::new();
    let store = ModelStore::new(root.path().to_path_buf(), OcrModelType::Mobile, sources(), 1024).unwrap();
    let fetcher = MemoryFetcher::new();
    let first = update_store(&store, fetcher.as_ref(), factory.clone(), CancellationToken::new(), Arc::new(|_| {}))
        .await
        .unwrap();
    let duplicate = update_store(&store, fetcher.as_ref(), factory.clone(), CancellationToken::new(), Arc::new(|_| {}))
        .await
        .unwrap();
    assert_eq!(first.generation, duplicate.generation);
    let generations = root.path().join("mobile/generations");
    assert_eq!(fs::read_dir(&generations).unwrap().count(), 1);
    let unknown = generations.join("user-owned");
    fs::create_dir(&unknown).unwrap();
    fs::write(unknown.join("notes.txt"), "preserve me").unwrap();
    fs::write(root.path().join("det.onnx"), "historical file").unwrap();
    fs::write(&first.paths.detector, "corrupt first generation").unwrap();
    let second = update_store(&store, fetcher.as_ref(), factory.clone(), CancellationToken::new(), Arc::new(|_| {}))
        .await
        .unwrap();
    fs::write(&second.paths.detector, "corrupt second generation").unwrap();
    let third = update_store(&store, fetcher.as_ref(), factory, CancellationToken::new(), Arc::new(|_| {}))
        .await
        .unwrap();
    // GC follows the persisted retention pointer even if a retained payload
    // subsequently became corrupt. It must never race a lease or user files.
    let pointer_path = root.path().join("mobile/active.json");
    let mut pointer: serde_json::Value = serde_json::from_slice(&fs::read(&pointer_path).unwrap()).unwrap();
    pointer["previous"] = second.generation.clone().into();
    fs::write(pointer_path, serde_json::to_vec(&pointer).unwrap()).unwrap();
    let second_generation = second.generation.clone();
    drop(second);
    let first_directory = generations.join(&first.generation);
    assert!(first_directory.is_dir(), "a live inference lease protects an older generation");
    drop(first);
    drop(duplicate);
    fs::write(first_directory.join("user-notes.txt"), "keep this unknown file").unwrap();
    store.prune_unleased();
    assert!(
        first_directory.join("user-notes.txt").is_file(),
        "unknown files prevent directory cleanup"
    );
    fs::remove_file(first_directory.join("user-notes.txt")).unwrap();
    store.prune_unleased();
    assert!(!first_directory.exists());
    assert!(generations.join(&second_generation).is_dir(), "one rollback generation is retained");
    assert!(generations.join(&third.generation).is_dir(), "active generation is retained");
    assert_eq!(fs::read_dir(&generations).unwrap().count(), 3);
    assert_eq!(fs::read_to_string(unknown.join("notes.txt")).unwrap(), "preserve me");
    assert_eq!(fs::read_to_string(root.path().join("det.onnx")).unwrap(), "historical file");
}

#[tokio::test]
async fn old_inference_completion_cannot_repopulate_cache_after_model_switches() {
    let root = tempfile::tempdir().unwrap();
    let factory = TestFactory::new();
    let fetcher = MemoryFetcher::new();
    let service = service(root.path(), fetcher.clone(), factory.clone());
    service.ensure_models(false, CancellationToken::new()).await.unwrap();
    let first_generation = service.model_status().active_generation.unwrap();
    *factory.block_inference.0.lock().unwrap() = true;
    let mut inference = Box::pin(service.recognize(ImageAsset::new(RgbaImage::new(8, 6)), CancellationToken::new()));
    assert!(poll!(&mut inference).is_pending());
    factory.wait_for_inference().await;
    for _ in 0..2 {
        let current = service.model_status().active_generation.unwrap();
        fs::write(
            root.path().join("mobile/generations").join(current).join("det.onnx"),
            "corrupt current generation",
        )
        .unwrap();
        service.ensure_models(true, CancellationToken::new()).await.unwrap();
    }
    let old_directory = root.path().join("mobile/generations").join(&first_generation);
    assert!(old_directory.is_dir());
    factory.release();
    inference.await.unwrap();
    let active_generation = service.model_status().active_generation.unwrap();
    assert!(
        service
            .inner
            .engines
            .lock()
            .unwrap()
            .iter()
            .all(|cached| cached.models.generation == active_generation),
        "an outdated worker cannot return its engine to the current cache"
    );
    service.shutdown().await.unwrap();
    assert!(
        !old_directory.exists(),
        "shutdown drains retirement after the worker's last lease is released"
    );
}

#[tokio::test]
async fn inference_returns_its_permit_while_retirement_is_blocked_and_shutdown_drains_it() {
    let root = tempfile::tempdir().unwrap();
    let service = service(root.path(), MemoryFetcher::new(), TestFactory::new());
    let image = ImageAsset::new(RgbaImage::new(8, 6));
    service.recognize(image.clone(), CancellationToken::new()).await.unwrap();
    // Deterministically stand in for a slow filesystem retirement transaction.
    let store = service.inner.store.clone();
    let (locked, lock_ready) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let maintenance_lock = tokio::task::spawn_blocking(move || {
        let _transaction = store.lock_transaction_for_test();
        locked.send(()).unwrap();
        released.recv().unwrap();
    });
    lock_ready.await.unwrap();
    service.inner.retirement.requested.notify_one();
    let first = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        service.recognize(image.clone(), CancellationToken::new()),
    )
    .await;
    let second = tokio::time::timeout(std::time::Duration::from_secs(2), service.recognize(image, CancellationToken::new())).await;
    let mut shutdown = Box::pin(service.shutdown());
    let draining = poll!(&mut shutdown).is_pending();
    let mut concurrent_shutdown = Box::pin(service.shutdown());
    let concurrent_draining = poll!(&mut concurrent_shutdown).is_pending();
    release.send(()).unwrap();
    maintenance_lock.await.unwrap();
    shutdown.await.unwrap();
    concurrent_shutdown.await.unwrap();
    assert!(
        draining && concurrent_draining,
        "all shutdown callers must acknowledge the owned retirement worker"
    );
    assert!(first.is_ok(), "inference must not await the filesystem maintenance lock");
    assert!(second.is_ok(), "the inference permit must be available before retirement finishes");
    assert_eq!(first.unwrap().unwrap()[0].text, "recognized");
    assert_eq!(second.unwrap().unwrap()[0].text, "recognized");
}

#[tokio::test]
async fn publication_validation_leases_survive_concurrent_retirement() {
    let root = tempfile::tempdir().unwrap();
    let factory = TestFactory::new();
    let store = ModelStore::new(root.path().to_path_buf(), OcrModelType::Mobile, sources(), 1024).unwrap();
    let active = update_store(
        &store,
        MemoryFetcher::new().as_ref(),
        factory.clone(),
        CancellationToken::new(),
        Arc::new(|_| {}),
    )
    .await
    .unwrap();
    let generations = root.path().join("mobile/generations");
    let mut retired = Vec::new();
    for suffix in ["first", "second"] {
        let path = generations.join(format!("{}-repair-{suffix}", active.generation));
        fs::create_dir(&path).unwrap();
        for name in ["det.onnx", "rec.onnx", "keys.txt", "manifest.json"] {
            fs::copy(generations.join(&active.generation).join(name), path.join(name)).unwrap();
        }
        retired.push(path);
    }
    let staging = store.staging().unwrap();
    for (source, name) in [
        (&active.paths.detector, "det.onnx"),
        (&active.paths.recognizer, "rec.onnx"),
        (&active.paths.keys, "keys.txt"),
    ] {
        fs::copy(source, staging.path().join(name)).unwrap();
    }
    let candidate = store.prepare_candidate(staging, factory.as_ref(), &CancellationToken::new()).unwrap();
    *factory.block_validation.0.lock().unwrap() = true;
    let publisher_store = store.clone();
    let publisher_factory = factory.clone();
    let publication =
        tokio::task::spawn_blocking(move || publisher_store.publish_candidate(candidate, publisher_factory.as_ref(), &CancellationToken::new()));
    factory.wait_for_validation().await;
    let retirement_store = store.clone();
    let mut retirement = tokio::task::spawn_blocking(move || retirement_store.prune_unleased());
    let completed = match tokio::time::timeout(std::time::Duration::from_secs(2), &mut retirement).await {
        Ok(result) => {
            result.unwrap();
            true
        }
        Err(_) => false,
    };
    let leases_preserved = retired.iter().all(|path| path.is_dir());
    factory.release_validation();
    let published = publication.await.unwrap().unwrap();
    if !completed {
        retirement.await.unwrap();
    }
    assert!(completed, "native candidate validation must not occupy the publication transaction");
    assert!(
        leases_preserved,
        "candidate snapshot leases protect inactive generations during validation"
    );
    store.prune_unleased();
    assert!(published.paths.detector.is_file());
    assert!(
        fs::read_dir(generations).unwrap().count() <= 2,
        "completed validation does not leak transient leases"
    );
}

#[tokio::test]
async fn simultaneous_prepared_publications_deduplicate_without_overwriting_leased_files() {
    let root = tempfile::tempdir().unwrap();
    let store = ModelStore::new(root.path().to_path_buf(), OcrModelType::Mobile, sources(), 1024).unwrap();
    let factory = TestFactory::new();
    let prepare = || {
        let staging = store.staging().unwrap();
        for (name, bytes) in [
            ("det.onnx", b"valid detector v1".as_slice()),
            ("rec.onnx", b"valid recognizer v1"),
            ("keys.txt", b"valid dictionary v1"),
        ] {
            fs::write(staging.path().join(name), bytes).unwrap();
        }
        store.prepare_candidate(staging, factory.as_ref(), &CancellationToken::new()).unwrap()
    };
    let first = prepare();
    let second = prepare();
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let publish = |candidate| {
        let store = store.clone();
        let factory = factory.clone();
        let barrier = barrier.clone();
        tokio::task::spawn_blocking(move || {
            barrier.wait();
            store.publish_candidate(candidate, factory.as_ref(), &CancellationToken::new())
        })
    };
    let first = publish(first);
    let second = publish(second);
    barrier.wait();
    let first = first.await.unwrap().unwrap();
    let second = second.await.unwrap().unwrap();
    assert_eq!(first.generation, second.generation);
    assert_eq!(fs::read_dir(root.path().join("mobile/generations")).unwrap().count(), 1);
    let current = store.load_existing(factory.as_ref(), &CancellationToken::new()).unwrap().unwrap();
    assert_eq!(current.generation, first.generation);
    assert_eq!(fs::read(&first.paths.detector).unwrap(), b"valid detector v1");
}

#[tokio::test]
async fn inference_reuses_engine_for_same_committed_generation() {
    let root = tempfile::tempdir().unwrap();
    let factory = TestFactory::new();
    let service = service(root.path(), MemoryFetcher::new(), factory.clone());
    let image = ImageAsset::new(RgbaImage::new(8, 6));
    for _ in 0..2 {
        let blocks = service.recognize(image.clone(), CancellationToken::new()).await.unwrap();
        assert_eq!(blocks[0].text, "recognized");
    }
    assert_eq!(factory.builds.load(Ordering::SeqCst), 1);
    assert_eq!(
        factory.input_pointer.load(Ordering::SeqCst),
        image.pixels().as_raw().as_ptr() as usize,
        "inference borrows the shared source pixels instead of cloning the full image"
    );
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn inference_admission_is_bounded_and_a_queued_request_can_cancel() {
    let root = tempfile::tempdir().unwrap();
    let factory = TestFactory::new();
    let service = service(root.path(), MemoryFetcher::new(), factory.clone());
    service.ensure_models(false, CancellationToken::new()).await.unwrap();
    *factory.block_inference.0.lock().unwrap() = true;
    let image = ImageAsset::new(RgbaImage::new(8, 6));
    let mut first = Box::pin(service.recognize(image.clone(), CancellationToken::new()));
    assert!(poll!(&mut first).is_pending());
    factory.wait_for_inference().await;
    let queued_cancel = CancellationToken::new();
    let mut second = Box::pin(service.recognize(image, queued_cancel.clone()));
    assert!(poll!(&mut second).is_pending());
    queued_cancel.cancel();
    assert_eq!(second.await.unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(factory.active.load(Ordering::SeqCst), 1);
    assert_eq!(factory.builds.load(Ordering::SeqCst), 1);
    factory.release();
    first.await.unwrap();
    assert_eq!(factory.peak.load(Ordering::SeqCst), 1);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn dropped_inference_waiter_is_drained_before_shutdown_acknowledges_completion() {
    let root = tempfile::tempdir().unwrap();
    let factory = TestFactory::new();
    let service = service(root.path(), MemoryFetcher::new(), factory.clone());
    service.ensure_models(false, CancellationToken::new()).await.unwrap();
    *factory.block_inference.0.lock().unwrap() = true;
    let mut inference = Box::pin(service.recognize(ImageAsset::new(RgbaImage::new(8, 6)), CancellationToken::new()));
    assert!(poll!(&mut inference).is_pending());
    factory.wait_for_inference().await;
    drop(inference);
    let mut shutdown = Box::pin(service.shutdown());
    assert!(poll!(&mut shutdown).is_pending());
    assert_eq!(factory.active.load(Ordering::SeqCst), 1);
    factory.release();
    shutdown.await.unwrap();
    assert_eq!(factory.active.load(Ordering::SeqCst), 0);
    assert_eq!(factory.peak.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancelled_inference_is_not_reported_finished_before_native_worker_returns() {
    let root = tempfile::tempdir().unwrap();
    let factory = TestFactory::new();
    let service = service(root.path(), MemoryFetcher::new(), factory.clone());
    service.ensure_models(false, CancellationToken::new()).await.unwrap();
    *factory.block_inference.0.lock().unwrap() = true;
    let cancel = CancellationToken::new();
    let mut inference = Box::pin(service.recognize(ImageAsset::new(RgbaImage::new(8, 6)), cancel.clone()));
    assert!(poll!(&mut inference).is_pending());
    factory.wait_for_inference().await;
    cancel.cancel();
    assert!(poll!(&mut inference).is_pending());
    assert_eq!(factory.active.load(Ordering::SeqCst), 1);
    factory.release();
    assert_eq!(inference.await.unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(factory.active.load(Ordering::SeqCst), 0);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancelled_or_invalid_input_requests_do_not_start_downloads() {
    let root = tempfile::tempdir().unwrap();
    let fetcher = MemoryFetcher::new();
    let service = RecognitionService::with_adapters(
        root.path().to_path_buf(),
        tokio::runtime::Handle::current(),
        RecognitionOptions {
            sources: Some(sources()),
            inference_budget: crate::InferenceBudget {
                max_input_pixels: 4,
                ..Default::default()
            },
            ..Default::default()
        },
        fetcher.clone(),
        TestFactory::new(),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert_eq!(service.ensure_models(true, cancel).await.unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(fetcher.calls.load(Ordering::SeqCst), 0);
    for (width, height) in [(0, 1), (3, 2)] {
        assert_eq!(
            service
                .recognize(ImageAsset::new(RgbaImage::new(width, height)), CancellationToken::new())
                .await
                .unwrap_err()
                .kind,
            ErrorKind::InvalidInput
        );
    }
    assert_eq!(
        fetcher.calls.load(Ordering::SeqCst),
        0,
        "invalid input is rejected before implicit model download"
    );
    service.shutdown().await.unwrap();
}

fn http_response(response: &'static [u8]) -> (String, std::thread::JoinHandle<()>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut request = [0u8; 2048];
        let _ = stream.read(&mut request);
        let _ = stream.write_all(response);
    });
    (format!("http://{address}/model"), server)
}

#[tokio::test]
async fn http_transfer_rejects_truncated_artifacts_and_unknown_length_over_budget() {
    let root = tempfile::tempdir().unwrap();
    let fetcher = HttpFetcher::new(16).unwrap();
    let cases: &[(&[u8], &str)] = &[
        (b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\nshort", "truncated"),
        (b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\npayload-exceeds-sixteen-bytes", "oversized"),
    ];
    for &(response, filename) in cases {
        let (url, server) = http_response(response);
        let result = fetcher
            .fetch(&url, &root.path().join(filename), CancellationToken::new(), Arc::new(|_| {}))
            .await;
        assert!(result.is_err(), "{filename} transfer must not be accepted");
        server.join().unwrap();
    }
}

#[tokio::test]
async fn http_transfer_accepts_complete_artifact_and_rejects_empty_body() {
    let root = tempfile::tempdir().unwrap();
    let fetcher = HttpFetcher::new(128).unwrap();
    let (url, server) = http_response(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nvalid");
    let destination = root.path().join("complete");
    fetcher
        .fetch(&url, &destination, CancellationToken::new(), Arc::new(|_| {}))
        .await
        .unwrap();
    server.join().unwrap();
    assert_eq!(fs::read(destination).unwrap(), b"valid");
    let (url, server) = http_response(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let error = fetcher
        .fetch(&url, &root.path().join("empty"), CancellationToken::new(), Arc::new(|_| {}))
        .await
        .unwrap_err();
    server.join().unwrap();
    assert_eq!(error.kind, ErrorKind::Validation);
    assert!(!root.path().join("empty").exists());
}

struct UnavailableFactory;

impl ArtifactValidator for UnavailableFactory {
    fn validate(&self, _: &ModelPaths) -> OperationResult<()> {
        panic!("unavailable runtime must not validate model artifacts")
    }
}

impl EngineFactory for UnavailableFactory {
    fn availability(&self) -> RecognitionAvailability {
        RecognitionAvailability::Unavailable {
            reason: "injected missing runtime".into(),
        }
    }

    fn build(&self, _: &ModelPaths) -> OperationResult<Box<dyn InferenceEngine>> {
        panic!("unavailable runtime must not construct an inference engine")
    }
}

#[tokio::test]
async fn unavailable_runtime_is_rejected_before_model_scan_or_download() {
    let root = tempfile::tempdir().unwrap();
    let models = root.path().join("not-created");
    let fetcher = MemoryFetcher::new();
    let result = RecognitionService::with_adapters(
        models.clone(),
        tokio::runtime::Handle::current(),
        RecognitionOptions::default(),
        fetcher.clone(),
        Arc::new(UnavailableFactory),
    );
    let error = result.err().expect("missing runtime rejects local service construction");
    assert_eq!(error.kind, ErrorKind::Unavailable);
    assert_eq!(fetcher.calls.load(Ordering::SeqCst), 0);
    assert!(!models.exists());
}

#[tokio::test]
async fn runtime_capability_is_checked_before_recognition_or_explicit_download() {
    let root = tempfile::tempdir().unwrap();
    let fetcher = MemoryFetcher::new();
    let factory = TestFactory::new();
    let service = service(root.path(), fetcher.clone(), factory.clone());
    finish_offline_scan(&service).await;
    let status = service.model_status();
    factory.runtime_available.store(false, Ordering::SeqCst);
    assert_eq!(
        service
            .recognize(ImageAsset::new(RgbaImage::new(8, 6)), CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Unavailable
    );
    assert_eq!(
        service.ensure_models(true, CancellationToken::new()).await.unwrap_err().kind,
        ErrorKind::Unavailable
    );
    assert_eq!(fetcher.calls.load(Ordering::SeqCst), 0);
    assert_eq!(service.model_status(), status);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn valid_looking_unpinned_replacement_preserves_ready_generation_and_reports_artifact() {
    let root = tempfile::tempdir().unwrap();
    let fetcher = MemoryFetcher::new();
    let service = service(root.path(), fetcher.clone(), TestFactory::new());
    service.ensure_models(false, CancellationToken::new()).await.unwrap();
    let active = service.model_status().active_generation;
    let pointer = fs::read(root.path().join("mobile/active.json")).unwrap();
    fetcher.files.lock().unwrap().insert("recognizer".into(), b"valid recognizer v2".to_vec());
    let error = service.ensure_models(true, CancellationToken::new()).await.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Validation);
    assert!(error.message.contains("test-ppocr-v1") && error.message.contains("rec.onnx"));
    assert_eq!(service.model_status().active_generation, active);
    assert!(service.model_status().is_ready());
    assert!(matches!(service.model_status().download, ModelDownloadStatus::Failed { .. }));
    assert_eq!(fs::read(root.path().join("mobile/active.json")).unwrap(), pointer);
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn unknown_legacy_dictionary_is_preserved_and_never_declared_ready() {
    let root = tempfile::tempdir().unwrap();
    for (name, bytes) in [
        ("det.onnx", b"valid detector v1".as_slice()),
        ("rec.onnx", b"valid recognizer v1"),
        ("keys.txt", b"valid dictionary v2"),
    ] {
        fs::write(root.path().join(name), bytes).unwrap();
    }
    let fetcher = MemoryFetcher::new();
    let service = service(root.path(), fetcher.clone(), TestFactory::new());
    finish_offline_scan(&service).await;
    assert!(!service.model_status().is_ready());
    assert!(matches!(service.model_status().download, ModelDownloadStatus::Failed { .. }));
    assert_eq!(fetcher.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fs::read(root.path().join("keys.txt")).unwrap(), b"valid dictionary v2");
    assert!(!root.path().join("mobile/active.json").exists());
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn old_self_consistent_manifest_is_not_an_authority_for_unknown_artifact_hashes() {
    let root = tempfile::tempdir().unwrap();
    let factory = TestFactory::new();
    let store = ModelStore::new(root.path().to_path_buf(), OcrModelType::Mobile, sources(), 1024).unwrap();
    let models = update_store(
        &store,
        MemoryFetcher::new().as_ref(),
        factory.clone(),
        CancellationToken::new(),
        Arc::new(|_| {}),
    )
    .await
    .unwrap();
    let directory = root.path().join("mobile/generations").join(&models.generation);
    let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    manifest["format"] = 1.into();
    manifest.as_object_mut().unwrap().remove("catalog");
    for unknown in [false, true] {
        if unknown {
            let bytes = b"valid recognizer v2";
            fs::write(directory.join("rec.onnx"), bytes).unwrap();
            manifest["files"][1]["sha256"] = sources_digest(bytes).into();
        }
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let generation = sources_digest(&bytes);
        let new_directory = root.path().join("mobile/generations").join(&generation);
        fs::create_dir_all(&new_directory).unwrap();
        for name in ["det.onnx", "rec.onnx", "keys.txt"] {
            fs::copy(directory.join(name), new_directory.join(name)).unwrap();
        }
        fs::write(new_directory.join("manifest.json"), bytes).unwrap();
        fs::write(
            root.path().join("mobile/active.json"),
            serde_json::to_vec(&serde_json::json!({"generation":generation})).unwrap(),
        )
        .unwrap();
        let result = store.load_existing(factory.as_ref(), &CancellationToken::new());
        if unknown {
            assert_eq!(result.unwrap_err().kind, ErrorKind::Validation);
            assert!(new_directory.is_dir());
            update_store(
                &store,
                MemoryFetcher::new().as_ref(),
                factory.clone(),
                CancellationToken::new(),
                Arc::new(|_| {}),
            )
            .await
            .unwrap();
            assert!(
                new_directory.is_dir(),
                "GC must preserve an unknown old spec after a known replacement is published"
            );
        } else {
            assert!(result.unwrap().is_some());
        }
    }
}

fn sources_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

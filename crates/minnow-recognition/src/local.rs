use minnow_application::ports::{
    ErrorKind, ModelDownloadStatus, ModelStatus, OcrBlock, OperationError, OperationFuture, OperationResult, RecognitionAvailability,
    RecognitionModels, TextRecognition,
};
use minnow_imaging::ImageRef;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::runtime::Handle;
use tokio::sync::{Notify, Semaphore, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::InferenceBudget;
use crate::models::{
    ArtifactFetcher, ArtifactValidator, HttpFetcher, ModelSources, ModelStore, PreparedModels, Progress, check_cancelled, task_error,
};
use crate::ocr::{EngineFactory, InferenceEngine, OnnxEngineFactory};
use crate::service::RecognitionOptions;

/// Shared models, update state and bounded inference engines.
///
/// Construction schedules an offline scan of existing generations and legacy
/// files; it never starts a network download. Call [`RecognitionModels::ensure_models`]
/// to explicitly ensure availability. Cancelling one waiter only stops waiting
/// for the shared update. Service shutdown is the authority that cancels it.
#[derive(Clone)]
pub struct LocalRecognitionService {
    inner: Arc<Inner>,
    _lifetime: Arc<Lifetime>,
}

struct Lifetime {
    shutdown: CancellationToken,
    retirement_stop: CancellationToken,
}

impl Drop for Lifetime {
    fn drop(&mut self) {
        self.shutdown.cancel();
        self.retirement_stop.cancel();
    }
}

struct Inner {
    runtime: Handle,
    store: ModelStore,
    fetcher: Arc<dyn ArtifactFetcher>,
    factory: Arc<dyn EngineFactory>,
    status: watch::Sender<ModelStatus>,
    active: Mutex<Option<Arc<PreparedModels>>>,
    flight: Mutex<Option<Arc<Flight>>>,
    engines: Mutex<Vec<CachedEngine>>,
    permits: Arc<Semaphore>,
    inference_budget: InferenceBudget,
    shutdown: CancellationToken,
    work: Mutex<WorkState>,
    active_work: watch::Sender<usize>,
    retirement: Retirement,
}

struct Retirement {
    requested: Notify,
    stop: CancellationToken,
    result: watch::Sender<Option<OperationResult<()>>>,
    task: Mutex<Option<JoinHandle<()>>>,
}

struct RetirementCompletion(watch::Sender<Option<OperationResult<()>>>);

impl Drop for RetirementCompletion {
    fn drop(&mut self) {
        if self.0.borrow().is_none() {
            self.0.send_replace(Some(Err(OperationError::new(
                ErrorKind::Task,
                "model retirement ended without a completion result",
            ))));
        }
    }
}

struct CachedEngine {
    // Drop the native engine before releasing its model-file lease.
    engine: Box<dyn InferenceEngine>,
    models: Arc<PreparedModels>,
}

#[derive(Default)]
struct WorkState {
    closing: bool,
    active: usize,
}

struct WorkGuard(Arc<Inner>);

impl Drop for WorkGuard {
    fn drop(&mut self) {
        let mut state = lock(&self.0.work);
        state.active -= 1;
        self.0.active_work.send_replace(state.active);
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum FlightKind {
    OfflineScan,
    Update,
}

type FlightResult = OperationResult<Option<Arc<PreparedModels>>>;

struct Flight {
    kind: FlightKind,
    // Completion waiters need no model lease; activation owns the generation.
    result: watch::Sender<Option<OperationResult<()>>>,
    task: Mutex<Option<JoinHandle<()>>>,
}

struct CompletionGuard {
    inner: Arc<Inner>,
    flight: Arc<Flight>,
    completed: bool,
}

impl CompletionGuard {
    fn complete(mut self, result: FlightResult) {
        self.inner.complete_flight(&self.flight, result);
        self.completed = true;
    }
}

impl Drop for CompletionGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.inner.complete_flight(
                &self.flight,
                Err(OperationError::new(ErrorKind::Task, "model operation ended without a completion result")),
            );
        }
    }
}

impl LocalRecognitionService {
    pub fn new(models_dir: impl Into<PathBuf>, runtime: Handle, options: RecognitionOptions) -> OperationResult<Self> {
        let factory = Arc::new(OnnxEngineFactory::new(options.inference_threads, options.inference_budget));
        factory.availability().require_available()?;
        let fetcher = Arc::new(HttpFetcher::new(options.max_artifact_bytes)?);
        Self::with_adapters(models_dir.into(), runtime, options, fetcher, factory)
    }

    fn with_adapters(
        models_dir: PathBuf,
        runtime: Handle,
        options: RecognitionOptions,
        fetcher: Arc<dyn ArtifactFetcher>,
        factory: Arc<dyn EngineFactory>,
    ) -> OperationResult<Self> {
        options.inference_budget.validate()?;
        if options.max_concurrent_inference == 0 || options.inference_threads == 0 || options.max_artifact_bytes == 0 {
            return Err(OperationError::new(
                ErrorKind::InvalidInput,
                "recognition resource budgets must be positive",
            ));
        }
        factory.availability().require_available()?;
        let store = ModelStore::new(
            models_dir,
            options.model_type,
            options.sources.unwrap_or_else(|| ModelSources::for_model(options.model_type)),
            options.max_artifact_bytes,
        )?;
        let (status, _) = watch::channel(ModelStatus::default());
        let (active_work, _) = watch::channel(0);
        let (retirement_result, _) = watch::channel(None);
        let retirement_stop = CancellationToken::new();
        let shutdown = options.shutdown.child_token();
        let inner = Arc::new(Inner {
            runtime,
            store,
            fetcher,
            factory,
            status,
            active: Mutex::new(None),
            flight: Mutex::new(None),
            engines: Mutex::new(Vec::new()),
            permits: Arc::new(Semaphore::new(options.max_concurrent_inference)),
            inference_budget: options.inference_budget,
            shutdown: shutdown.clone(),
            work: Mutex::new(WorkState::default()),
            active_work,
            retirement: Retirement {
                requested: Notify::new(),
                stop: retirement_stop.clone(),
                result: retirement_result,
                task: Mutex::new(None),
            },
        });
        let service = Self {
            inner,
            _lifetime: Arc::new(Lifetime { shutdown, retirement_stop }),
        };
        service.inner.start_retirement();
        // The scan is service-owned, so a settings window can close and reopen
        // without restarting it or losing its authoritative status.
        service.inner.start_or_join(FlightKind::OfflineScan)?;
        Ok(service)
    }

    /// Cancels service-owned work and acknowledges completion of blocking inference.
    /// No native inference is claimed to have stopped before its worker returns.
    pub async fn shutdown(&self) -> OperationResult<()> {
        {
            let mut state = lock(&self.inner.work);
            state.closing = true;
        }
        self.inner.shutdown.cancel();
        let flight = lock(&self.inner.flight).clone();
        if let Some(flight) = flight {
            let task = lock(&flight.task).take();
            if let Some(task) = task {
                task.await.map_err(task_error)?;
            } else {
                let mut result = flight.result.subscribe();
                while result.borrow().is_none() {
                    result.changed().await.map_err(task_error)?;
                }
            }
        }
        let mut work = self.inner.active_work.subscribe();
        while *work.borrow() != 0 {
            work.changed().await.map_err(task_error)?;
        }
        lock(&self.inner.engines).clear();
        // Stop only after publishers and inference have released their leases.
        // The owned worker performs one final pass even when shutdown cancelled
        // the ordinary work token, and concurrent callers observe the same result.
        self.inner.retirement.stop.cancel();
        let task = lock(&self.inner.retirement.task).take();
        if let Some(task) = task {
            task.await.map_err(task_error)?;
        }
        let mut result = self.inner.retirement.result.subscribe();
        loop {
            if let Some(result) = result.borrow().clone() {
                return result;
            }
            result.changed().await.map_err(task_error)?;
        }
    }

    async fn ensure(&self, force: bool, cancel: CancellationToken) -> OperationResult<()> {
        check_cancelled(&cancel)?;
        self.availability().require_available()?;
        loop {
            check_cancelled(&self.inner.shutdown)?;
            if !force && lock(&self.inner.active).is_some() {
                return Ok(());
            }
            let flight = self.inner.start_or_join(FlightKind::Update)?;
            let kind = flight.kind;
            let result = wait_for_flight(&flight, &cancel).await?;
            match (kind, result) {
                (FlightKind::OfflineScan, _) => {
                    // A forced update still follows the completed offline scan.
                    // A missing/corrupt legacy set does not prevent repair.
                }
                (FlightKind::Update, result) => return result,
            }
        }
    }
}

impl Inner {
    fn start_retirement(self: &Arc<Self>) {
        let inner = self.clone();
        let completion = RetirementCompletion(self.retirement.result.clone());
        let task = self.runtime.spawn(async move {
            let _completion = completion;
            let result = inner.run_retirement().await;
            inner.retirement.result.send_replace(Some(result));
        });
        *lock(&self.retirement.task) = Some(task);
    }

    async fn run_retirement(&self) -> OperationResult<()> {
        loop {
            // Notify retains only one pending request, coalescing publication
            // and old-lease releases while one filesystem pass is running.
            let stopping = tokio::select! {
                biased;
                _ = self.retirement.stop.cancelled() => true,
                _ = self.retirement.requested.notified() => false,
            };
            let store = self.store.clone();
            self.runtime.spawn_blocking(move || store.prune_unleased()).await.map_err(task_error)?;
            if stopping {
                return Ok(());
            }
        }
    }

    fn start_or_join(self: &Arc<Self>, kind: FlightKind) -> OperationResult<Arc<Flight>> {
        let state = lock(&self.work);
        if state.closing || self.shutdown.is_cancelled() {
            return Err(OperationError::cancelled());
        }
        let mut slot = lock(&self.flight);
        if let Some(flight) = slot.as_ref()
            && flight.result.borrow().is_none()
        {
            return Ok(flight.clone());
        }
        let (result, _) = watch::channel(None);
        let flight = Arc::new(Flight {
            kind,
            result,
            task: Mutex::new(None),
        });
        *slot = Some(flight.clone());
        if kind == FlightKind::Update {
            self.status.send_modify(|status| {
                status.download = ModelDownloadStatus::Downloading { progress_percent: 0 };
            });
        }
        let inner = self.clone();
        let task_flight = flight.clone();
        let completion = CompletionGuard {
            inner: inner.clone(),
            flight: task_flight,
            completed: false,
        };
        let task = self.runtime.spawn(async move {
            let completion = completion;
            let result = match kind {
                FlightKind::OfflineScan => {
                    let store = inner.store.clone();
                    let validator = inner.factory.clone();
                    let cancel = inner.shutdown.clone();
                    inner
                        .runtime
                        .spawn_blocking(move || store.load_existing(validator.as_ref(), &cancel))
                        .await
                        .map_err(task_error)
                        .and_then(|result| result)
                }
                FlightKind::Update => {
                    let status = inner.status.clone();
                    let progress = Arc::new(move |fraction: f32| {
                        let progress_percent = (fraction * 100.0).round().clamp(0.0, 100.0) as u8;
                        status.send_modify(|state| {
                            state.download = ModelDownloadStatus::Downloading { progress_percent };
                        });
                    });
                    update_models(
                        &inner.store,
                        &inner.runtime,
                        inner.fetcher.as_ref(),
                        inner.factory.clone(),
                        inner.shutdown.clone(),
                        progress,
                    )
                    .await
                    .map(Some)
                }
            };
            completion.complete(result);
        });
        *lock(&flight.task) = Some(task);
        drop(state);
        Ok(flight)
    }

    fn complete_flight(&self, flight: &Flight, result: FlightResult) {
        match &result {
            Ok(Some(prepared)) => {
                // Both generation publication and returned-engine admission use
                // active -> engines. An old worker cannot reinsert after this
                // generation's cache eviction.
                let retired = {
                    let mut active = lock(&self.active);
                    let mut engines = lock(&self.engines);
                    *active = Some(prepared.clone());
                    let (current, retired): (Vec<_>, Vec<_>) = std::mem::take(&mut *engines)
                        .into_iter()
                        .partition(|engine| engine.models.generation == prepared.generation);
                    *engines = current;
                    retired
                };
                // Native destruction can take time; it retains its own lease
                // and happens after both cache/publication locks are released.
                drop(retired);
                self.status.send_modify(|status| {
                    status.active_generation = Some(prepared.generation.clone());
                    status.download = ModelDownloadStatus::Idle;
                });
                self.retirement.requested.notify_one();
            }
            Ok(None) => {}
            Err(error) => {
                self.status.send_modify(|status| {
                    status.download = if error.kind == ErrorKind::Cancelled {
                        ModelDownloadStatus::Idle
                    } else {
                        ModelDownloadStatus::Failed {
                            message: error.message.clone(),
                        }
                    };
                });
            }
        }
        flight.result.send_replace(Some(result.map(|_| ())));
    }

    fn begin_work(self: &Arc<Self>) -> OperationResult<WorkGuard> {
        let mut state = lock(&self.work);
        if state.closing || self.shutdown.is_cancelled() {
            return Err(OperationError::cancelled());
        }
        state.active += 1;
        self.active_work.send_replace(state.active);
        Ok(WorkGuard(self.clone()))
    }
}

/// Download orchestration and runtime scheduling belong to the application
/// service; ModelStore performs only synchronous identity and file transactions.
async fn update_models(
    store: &ModelStore,
    runtime: &Handle,
    fetcher: &dyn ArtifactFetcher,
    validator: Arc<dyn ArtifactValidator>,
    cancel: CancellationToken,
    progress: Progress,
) -> OperationResult<Arc<PreparedModels>> {
    check_cancelled(&cancel)?;
    let staging = store.staging()?;
    for (index, artifact) in store.sources().artifacts().into_iter().enumerate() {
        let total_progress = progress.clone();
        let callback: Progress = Arc::new(move |current| total_progress((index as f32 + current.clamp(0.0, 1.0)) / 3.0));
        fetcher
            .fetch(artifact.url(), &staging.path().join(artifact.name()), cancel.clone(), callback)
            .await?;
    }
    let store = store.clone();
    runtime
        .spawn_blocking(move || {
            let candidate = store.prepare_candidate(staging, validator.as_ref(), &cancel)?;
            store.publish_candidate(candidate, validator.as_ref(), &cancel)
        })
        .await
        .map_err(task_error)?
}

async fn wait_for_flight(flight: &Flight, cancel: &CancellationToken) -> OperationResult<OperationResult<()>> {
    let mut result = flight.result.subscribe();
    loop {
        check_cancelled(cancel)?;
        if let Some(result) = result.borrow().clone() {
            return Ok(result);
        }
        tokio::select! {
            _ = cancel.cancelled() => return Err(OperationError::cancelled()),
            changed = result.changed() => changed.map_err(task_error)?,
        }
    }
}

impl TextRecognition for LocalRecognitionService {
    fn availability(&self) -> RecognitionAvailability {
        self.inner.factory.availability()
    }

    fn recognize(&self, image: ImageRef, cancel: CancellationToken) -> OperationFuture<'_, Vec<OcrBlock>> {
        Box::pin(async move {
            self.inner.inference_budget.check_input(image.width(), image.height())?;
            self.ensure(false, cancel.clone()).await?;
            let permit = tokio::select! {
                _ = cancel.cancelled() => return Err(OperationError::cancelled()),
                _ = self.inner.shutdown.cancelled() => return Err(OperationError::cancelled()),
                permit = self.inner.permits.clone().acquire_owned() => permit.map_err(task_error)?,
            };
            let work = self.inner.begin_work()?;
            let prepared = lock(&self.inner.active)
                .clone()
                .ok_or_else(|| OperationError::new(ErrorKind::ModelUnavailable, "OCR models are not available"))?;
            let inner = self.inner.clone();
            let worker_cancel = cancel.clone();
            let task = self.inner.runtime.spawn_blocking(move || {
                let _work = work;
                let result = (|| {
                    check_cancelled(&worker_cancel)?;
                    check_cancelled(&inner.shutdown)?;
                    let cached = {
                        let mut engines = lock(&inner.engines);
                        engines
                            .iter()
                            .position(|engine| engine.models.generation == prepared.generation)
                            .map(|index| engines.swap_remove(index).engine)
                    };
                    let mut engine = match cached {
                        Some(engine) => engine,
                        None => inner.factory.build(&prepared.paths)?,
                    };
                    // The worker borrows shared source pixels; only derived
                    // detector inputs and text crops allocate pixel buffers.
                    let result = engine.recognize(image.pixels());
                    let mut engine = Some(engine);
                    {
                        let active = lock(&inner.active);
                        if result.is_ok()
                            && !inner.shutdown.is_cancelled()
                            && active.as_ref().is_some_and(|active| active.generation == prepared.generation)
                        {
                            lock(&inner.engines).push(CachedEngine {
                                engine: engine.take().expect("engine is returned at most once"),
                                models: prepared.clone(),
                            });
                        }
                    }
                    // Destruction of an uncached native engine still owns the
                    // generation lease and does not occupy the active lock.
                    drop(engine);
                    result
                })();
                let retired = {
                    let active = lock(&inner.active);
                    let current = active.as_ref().is_some_and(|active| active.generation == prepared.generation);
                    // Either publication queues a pass after this lease drop,
                    // or this already-retired worker queues it below. This also
                    // covers cancellation and engine-construction failure.
                    drop(prepared);
                    !current
                };
                drop(permit);
                if retired {
                    inner.retirement.requested.notify_one();
                }
                result
            });
            // A running spawn_blocking task is acknowledged only after it ends.
            // If this future is dropped, WorkGuard still lets shutdown drain it.
            let result = task.await.map_err(task_error)?;
            check_cancelled(&cancel)?;
            check_cancelled(&self.inner.shutdown)?;
            result
        })
    }
}

impl RecognitionModels for LocalRecognitionService {
    fn model_status(&self) -> ModelStatus {
        self.inner.status.borrow().clone()
    }

    fn subscribe_model_status(&self) -> watch::Receiver<ModelStatus> {
        self.inner.status.subscribe()
    }

    fn ensure_models(&self, force: bool, cancel: CancellationToken) -> OperationFuture<'_, ()> {
        Box::pin(self.ensure(force, cancel))
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

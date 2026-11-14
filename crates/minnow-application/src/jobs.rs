use crate::sessions::{JobScope, SessionId, SessionRegistry, lock};
use crate::{ErrorKind, OperationError, OperationResult};
use futures_util::FutureExt;
use std::collections::{HashMap, VecDeque};
use std::future::{Future, poll_fn};
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

const HISTORY_LIMIT: usize = 128;
const CAPTURE_OUTSTANDING_LIMIT: usize = 4;
const LONG_CAPTURE_OUTSTANDING_LIMIT: usize = 2;
const IMAGE_OUTPUT_OUTSTANDING_LIMIT: usize = 4;
const FEEDBACK_OUTSTANDING_LIMIT: usize = 8;
const RECOGNITION_OUTSTANDING_LIMIT: usize = 8;

/// Accepted work includes queued, prepared and provider-waiting jobs. Its
/// lifetime is independent of native execution capacity.
#[derive(Clone, Copy)]
pub(crate) enum JobAdmission {
    InteractiveCapture,
    QuickCapture,
    LongCapture,
    ImageOutput,
    Feedback,
    Recognition,
}

impl JobAdmission {
    fn resource(self) -> Option<JobResource> {
        match self {
            Self::InteractiveCapture => Some(JobResource::InteractiveCapture),
            Self::LongCapture => Some(JobResource::LongCapture),
            Self::ImageOutput => Some(JobResource::ImageOutput),
            Self::Feedback => Some(JobResource::Feedback),
            Self::QuickCapture | Self::Recognition => None,
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::InteractiveCapture | Self::QuickCapture => "interactive capture",
            Self::LongCapture => "long capture",
            Self::ImageOutput => "image output",
            Self::Feedback => "feedback",
            Self::Recognition => "recognition",
        }
    }
}

struct OutstandingAdmission {
    capture: Arc<Semaphore>,
    long_capture: Arc<Semaphore>,
    image_output: Arc<Semaphore>,
    feedback: Arc<Semaphore>,
    recognition: Arc<Semaphore>,
}

impl Default for OutstandingAdmission {
    fn default() -> Self {
        Self {
            capture: Arc::new(Semaphore::new(CAPTURE_OUTSTANDING_LIMIT)),
            long_capture: Arc::new(Semaphore::new(LONG_CAPTURE_OUTSTANDING_LIMIT)),
            image_output: Arc::new(Semaphore::new(IMAGE_OUTPUT_OUTSTANDING_LIMIT)),
            feedback: Arc::new(Semaphore::new(FEEDBACK_OUTSTANDING_LIMIT)),
            recognition: Arc::new(Semaphore::new(RECOGNITION_OUTSTANDING_LIMIT)),
        }
    }
}

impl OutstandingAdmission {
    fn try_accept(&self, admission: JobAdmission) -> OperationResult<OwnedSemaphorePermit> {
        let semaphore = match admission {
            JobAdmission::InteractiveCapture | JobAdmission::QuickCapture => &self.capture,
            JobAdmission::LongCapture => &self.long_capture,
            JobAdmission::ImageOutput => &self.image_output,
            JobAdmission::Feedback => &self.feedback,
            JobAdmission::Recognition => &self.recognition,
        };
        semaphore.clone().try_acquire_owned().map_err(|_| {
            OperationError::new(
                ErrorKind::Busy,
                format!(
                    "too many outstanding {} jobs; wait for current work to finish and try again",
                    admission.description()
                ),
            )
        })
    }
}

/// Admission follows the resource which can block, independently of job ownership.
#[derive(Clone, Copy)]
pub(crate) enum JobResource {
    InteractiveCapture,
    LongCapture,
    ImageOutput,
    Feedback,
}

impl From<JobResource> for JobAdmission {
    fn from(resource: JobResource) -> Self {
        match resource {
            JobResource::InteractiveCapture => Self::InteractiveCapture,
            JobResource::LongCapture => Self::LongCapture,
            JobResource::ImageOutput => Self::ImageOutput,
            JobResource::Feedback => Self::Feedback,
        }
    }
}

struct ResourceAdmission {
    capture: Arc<Semaphore>,
    long_capture: Arc<Semaphore>,
    image_output: Arc<Semaphore>,
    feedback: Arc<Semaphore>,
}

impl ResourceAdmission {
    fn new(capture: usize, long_capture: usize, image_output: usize, feedback: usize) -> Self {
        Self {
            capture: Arc::new(Semaphore::new(capture.max(1))),
            long_capture: Arc::new(Semaphore::new(long_capture.max(1))),
            image_output: Arc::new(Semaphore::new(image_output.max(1))),
            feedback: Arc::new(Semaphore::new(feedback.max(1))),
        }
    }

    fn semaphore(&self, resource: JobResource) -> Arc<Semaphore> {
        match resource {
            JobResource::InteractiveCapture => self.capture.clone(),
            JobResource::LongCapture => self.long_capture.clone(),
            JobResource::ImageOutput => self.image_output.clone(),
            JobResource::Feedback => self.feedback.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct JobId(u64);

impl JobId {
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobPhase {
    Queued,
    Running,
    CancellationRequested,
    Succeeded,
    Cancelled,
    Superseded,
    Failed(OperationError),
}

impl JobPhase {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeeded | Self::Cancelled | Self::Superseded | Self::Failed(_))
    }
}

/// Derived presentation data may be discarded; committed effects remain observable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionPolicy {
    RequireCurrentSession,
    PreserveResult,
}

#[derive(Clone, Debug)]
pub struct JobRecord {
    pub id: JobId,
    pub label: String,
    pub scope: JobScope,
    pub phase: JobPhase,
    pub ui_applicable: bool,
}

struct Entry {
    record: JobRecord,
    policy: CompletionPolicy,
    cancel: CancellationToken,
    phase: watch::Sender<JobPhase>,
    task: Option<JoinHandle<()>>,
    task_attached: bool,
}

#[derive(Default)]
struct SupervisorState {
    accepting: bool,
    entries: HashMap<JobId, Entry>,
    history: VecDeque<JobRecord>,
}

struct Inner {
    runtime: Handle,
    sessions: SessionRegistry,
    root_cancel: CancellationToken,
    sequence: AtomicU64,
    state: Mutex<SupervisorState>,
    admission: ResourceAdmission,
    outstanding: OutstandingAdmission,
    changes: watch::Sender<u64>,
    shutdown_lock: tokio::sync::Mutex<()>,
}

#[derive(Clone)]
pub(crate) struct JobSupervisor(Arc<Inner>);

/// Owns the single terminal notification, including an unexpected future drop.
struct JobCompletion<T> {
    owner: JobSupervisor,
    id: JobId,
    result: Option<oneshot::Sender<OperationResult<T>>>,
}

impl<T> JobCompletion<T> {
    fn finish(mut self, result: OperationResult<T>) {
        let result = self.owner.complete(self.id, result);
        if let Some(sender) = self.result.take() {
            let _ = sender.send(result);
        }
    }
}

impl<T> Drop for JobCompletion<T> {
    fn drop(&mut self) {
        if let Some(sender) = self.result.take() {
            let result = self.owner.complete(
                self.id,
                Err(OperationError::new(
                    ErrorKind::Task,
                    "application worker ended without a completion result",
                )),
            );
            let _ = sender.send(result);
        }
    }
}

pub struct JobHandle<T> {
    id: JobId,
    scope: JobScope,
    receiver: oneshot::Receiver<OperationResult<T>>,
    phase: watch::Receiver<JobPhase>,
    owner: JobSupervisor,
    rejection: Option<OperationError>,
}

impl<T> JobHandle<T> {
    pub fn id(&self) -> JobId {
        self.id
    }

    pub fn scope(&self) -> JobScope {
        self.scope
    }

    pub fn cancel(&self) {
        self.owner.cancel(self.id);
    }

    pub fn phase(&self) -> JobPhase {
        self.phase.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<JobPhase> {
        self.phase.clone()
    }

    pub fn is_current(&self) -> bool {
        self.owner.0.sessions.accepts(self.scope)
    }

    pub(crate) fn admission_error(&self) -> Option<&OperationError> {
        self.rejection.as_ref()
    }
}

impl<T> Future for JobHandle<T> {
    type Output = OperationResult<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        Pin::new(&mut this.receiver)
            .poll(cx)
            .map(|result| result.unwrap_or_else(|_| Err(OperationError::new(ErrorKind::Task, "application job ended without a completion result"))))
    }
}

#[derive(Clone, Debug)]
pub struct ShutdownReport {
    pub drained: bool,
    pub unfinished: Vec<JobRecord>,
}

impl JobSupervisor {
    pub fn new(runtime: Handle, sessions: SessionRegistry) -> Self {
        Self::with_admission(runtime, sessions, ResourceAdmission::new(1, 1, 2, 2))
    }

    fn with_admission(runtime: Handle, sessions: SessionRegistry, admission: ResourceAdmission) -> Self {
        let (changes, _) = watch::channel(0);
        Self(Arc::new(Inner {
            runtime,
            sessions,
            root_cancel: CancellationToken::new(),
            sequence: AtomicU64::new(1),
            state: Mutex::new(SupervisorState {
                accepting: true,
                ..SupervisorState::default()
            }),
            admission,
            outstanding: OutstandingAdmission::default(),
            changes,
            shutdown_lock: tokio::sync::Mutex::new(()),
        }))
    }

    /// Admission is synchronous: rejected work never gets a worker or retains
    /// its input snapshot. Every accepted job counts until its task ends.
    pub fn spawn<T, F, Fut>(
        &self,
        label: impl Into<String>,
        scope: JobScope,
        policy: CompletionPolicy,
        admission: JobAdmission,
        run: F,
    ) -> JobHandle<T>
    where
        T: Send + 'static,
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: Future<Output = OperationResult<T>> + Send + 'static,
    {
        self.spawn_after(label, scope, policy, admission, std::future::ready(Ok(())), run)
    }

    /// Track prepared work immediately, but acquire execution capacity only once
    /// its owner has completed the handoff represented by `ready`.
    pub(crate) fn spawn_after<T, F, Fut, Ready>(
        &self,
        label: impl Into<String>,
        scope: JobScope,
        policy: CompletionPolicy,
        admission: JobAdmission,
        ready: Ready,
        run: F,
    ) -> JobHandle<T>
    where
        T: Send + 'static,
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: Future<Output = OperationResult<T>> + Send + 'static,
        Ready: Future<Output = OperationResult<()>> + Send + 'static,
    {
        let id = JobId(self.0.sequence.fetch_add(1, Ordering::Relaxed));
        let (result_tx, result_rx) = oneshot::channel();
        let (phase_tx, phase_rx) = watch::channel(JobPhase::Queued);
        let mut handle = JobHandle {
            id,
            scope,
            receiver: result_rx,
            phase: phase_rx,
            owner: self.clone(),
            rejection: None,
        };
        let cancel = self.0.root_cancel.child_token();
        let record = JobRecord {
            id,
            label: label.into(),
            scope,
            phase: JobPhase::Queued,
            ui_applicable: self.0.sessions.accepts(scope),
        };
        let outstanding = {
            let mut state = lock(&self.0.state);
            Self::prune_finished(&mut state);
            let accepted = if state.accepting {
                self.0.outstanding.try_accept(admission)
            } else {
                Err(OperationError::new(ErrorKind::Unavailable, "application is shutting down"))
            };
            let outstanding = match accepted {
                Ok(outstanding) => outstanding,
                Err(error) => {
                    handle.rejection = Some(error.clone());
                    phase_tx.send_replace(JobPhase::Failed(error.clone()));
                    let _ = result_tx.send(Err(error));
                    return handle;
                }
            };
            state.entries.insert(
                id,
                Entry {
                    record,
                    policy,
                    cancel: cancel.clone(),
                    phase: phase_tx,
                    task: None,
                    task_attached: false,
                },
            );
            outstanding
        };
        let owner = self.clone();
        let completion = JobCompletion {
            owner: self.clone(),
            id,
            result: Some(result_tx),
        };
        let task = self.0.runtime.spawn(async move {
            // This outer guard also outlives unused input and undelivered-result
            // cleanup in the inner future, not only the completion notification.
            let _outstanding = outstanding;
            async move {
                let mut execution = None;
                // Only unwinding panics can be converted to task failures.
                // A panic=abort build terminates the process instead.
                let outcome = AssertUnwindSafe(async {
                    tokio::select! {
                        biased;
                        _ = cancel.cancelled() => return Err(OperationError::cancelled()),
                        ready = ready => ready?,
                    }
                    if let Some(resource) = admission.resource() {
                        execution = Some(owner.acquire(resource, &cancel).await?);
                    }
                    if cancel.is_cancelled() {
                        return Err(OperationError::cancelled());
                    }
                    if policy == CompletionPolicy::RequireCurrentSession && !owner.0.sessions.accepts(scope) {
                        return Err(OperationError::new(
                            ErrorKind::Conflict,
                            "job input belongs to an obsolete document session",
                        ));
                    }
                    owner.set_phase(id, JobPhase::Running);
                    run(cancel).await
                })
                .catch_unwind()
                .await;
                let result = match outcome {
                    Ok(result) => result,
                    Err(payload) => {
                        let text = payload
                            .downcast_ref::<String>()
                            .cloned()
                            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                            .unwrap_or_else(|| "unknown panic".to_string());
                        Err(OperationError::new(ErrorKind::Task, format!("application job panicked: {text}")))
                    }
                };
                // Await the operation itself even after cancellation is requested: a running
                // blocking operation can still commit an external effect.
                completion.finish(result);
                drop(execution);
            }
            .await;
        });
        if let Some(entry) = lock(&self.0.state).entries.get_mut(&id) {
            entry.task = Some(task);
            entry.task_attached = true;
        }
        self.0.changes.send_modify(|sequence| *sequence = sequence.wrapping_add(1));
        handle
    }

    pub fn spawn_blocking<T, F>(
        &self,
        label: impl Into<String>,
        scope: JobScope,
        policy: CompletionPolicy,
        resource: JobResource,
        run: F,
    ) -> JobHandle<T>
    where
        T: Send + 'static,
        F: FnOnce(CancellationToken) -> OperationResult<T> + Send + 'static,
    {
        let runtime = self.0.runtime.clone();
        self.spawn(label, scope, policy, resource.into(), move |cancel| async move {
            crate::services::blocking(&runtime, move || run(cancel)).await
        })
    }

    /// Stage admission is useful when one command moves between resources.
    /// The caller keeps the permit until its real blocking worker has joined.
    pub(crate) async fn acquire(&self, resource: JobResource, cancel: &CancellationToken) -> OperationResult<OwnedSemaphorePermit> {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(OperationError::cancelled()),
            permit = self.0.admission.semaphore(resource).acquire_owned() =>
                permit.map_err(|_| OperationError::new(ErrorKind::Unavailable, "application resource is unavailable")),
        }
    }

    fn prune_finished(state: &mut SupervisorState) {
        state
            .entries
            .retain(|_, entry| !entry.record.phase.is_terminal() || entry.task.as_ref().is_none_or(|task| !task.is_finished()));
    }

    fn set_phase(&self, id: JobId, phase: JobPhase) {
        if let Some(entry) = lock(&self.0.state).entries.get_mut(&id)
            && !entry.record.phase.is_terminal()
            && !matches!(entry.record.phase, JobPhase::CancellationRequested)
        {
            entry.record.phase = phase.clone();
            entry.phase.send_replace(phase);
        }
    }

    fn complete<T>(&self, id: JobId, result: OperationResult<T>) -> OperationResult<T> {
        let mut state = lock(&self.0.state);
        let Some(entry) = state.entries.get_mut(&id) else {
            return result;
        };
        if entry.record.phase.is_terminal() {
            // A failed join or closed waiter must not rewrite a committed result
            // or append a second terminal record.
            return result;
        }
        let applicable = self.0.sessions.accepts(entry.record.scope);
        let superseded = !applicable
            && entry.policy == CompletionPolicy::RequireCurrentSession
            && (result.is_ok() || result.as_ref().is_err_and(|error| error.kind == ErrorKind::Conflict));
        let phase = match &result {
            _ if superseded => JobPhase::Superseded,
            Ok(_) => JobPhase::Succeeded,
            Err(error) if error.kind == ErrorKind::Cancelled => JobPhase::Cancelled,
            Err(error) => JobPhase::Failed(error.clone()),
        };
        entry.record.phase = phase.clone();
        entry.record.ui_applicable = applicable;
        entry.phase.send_replace(phase);
        let record = entry.record.clone();
        state.history.push_back(record);
        while state.history.len() > HISTORY_LIMIT {
            state.history.pop_front();
        }
        self.0.changes.send_modify(|sequence| *sequence = sequence.wrapping_add(1));
        if superseded {
            Err(OperationError::new(
                ErrorKind::Conflict,
                "job result belongs to an obsolete document session",
            ))
        } else {
            result
        }
    }

    pub fn cancel(&self, id: JobId) {
        if let Some(entry) = lock(&self.0.state).entries.get_mut(&id)
            && !entry.record.phase.is_terminal()
        {
            entry.cancel.cancel();
            entry.record.phase = JobPhase::CancellationRequested;
            entry.phase.send_replace(JobPhase::CancellationRequested);
        }
    }

    /// Closing or editing a document invalidates derived work, not an export
    /// which the user already requested for an immutable snapshot.
    pub fn cancel_derived_session_jobs(&self, session: SessionId) {
        self.cancel_session_jobs(session, |_| true);
    }

    /// A delayed invalidation for revision N must never cancel work for N or a
    /// newer revision admitted by a concurrent update.
    pub(crate) fn cancel_obsolete_session_jobs(&self, session: SessionId, revision: u64) {
        self.cancel_session_jobs(session, |job_revision| job_revision < revision);
    }

    fn cancel_session_jobs(&self, session: SessionId, obsolete: impl Fn(u64) -> bool) {
        let ids: Vec<_> = lock(&self.0.state)
            .entries
            .iter()
            .filter(|(_, entry)| {
                entry.policy == CompletionPolicy::RequireCurrentSession
                    && matches!(entry.record.scope, JobScope::Session { id, revision } if id == session && obsolete(revision))
            })
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.cancel(id);
        }
    }

    pub fn records(&self) -> Vec<JobRecord> {
        lock(&self.0.state).history.iter().cloned().collect()
    }

    fn begin_shutdown(&self) {
        lock(&self.0.state).accepting = false;
        self.0.root_cancel.cancel();
        let ids: Vec<_> = lock(&self.0.state).entries.keys().copied().collect();
        for id in ids {
            self.cancel(id);
        }
    }

    async fn drain(&self) {
        let mut changes = self.0.changes.subscribe();
        loop {
            let joined = poll_fn(|cx| {
                let mut drained = true;
                let mut failures = Vec::new();
                for (id, entry) in lock(&self.0.state).entries.iter_mut() {
                    if !entry.task_attached {
                        drained = false;
                    }
                    if let Some(task) = entry.task.as_mut() {
                        match Pin::new(task).poll(cx) {
                            Poll::Ready(joined) => {
                                // Retain ownership until join confirms completion,
                                // including when an unexpected worker exit failed.
                                entry.task = None;
                                if !entry.record.phase.is_terminal() {
                                    let message = match joined {
                                        Err(error) => format!("application worker join failed: {error}"),
                                        Ok(()) => "application worker ended without a terminal completion".to_string(),
                                    };
                                    failures.push((*id, OperationError::new(ErrorKind::Task, message)));
                                }
                            }
                            Poll::Pending => drained = false,
                        }
                    }
                    drained &= entry.record.phase.is_terminal();
                }
                if !failures.is_empty() {
                    Poll::Ready(Err(failures))
                } else if drained {
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            });
            tokio::select! {
                outcome = joined => match outcome {
                    Ok(()) => return,
                    Err(failures) => for (id, error) in failures {
                        let _ = self.complete::<()>(id, Err(error));
                    },
                },
                changed = changes.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
            }
        }
    }

    /// Stop admission and join every owned worker. Call from the executable or
    /// UI lifecycle, never from a job owned by this supervisor: it would await itself.
    pub async fn shutdown(&self) -> ShutdownReport {
        self.begin_shutdown();
        let _guard = self.0.shutdown_lock.lock().await;
        self.drain().await;
        ShutdownReport {
            drained: true,
            unfinished: Vec::new(),
        }
    }

    pub async fn shutdown_with_timeout(&self, timeout: Duration) -> ShutdownReport {
        let deadline = tokio::time::Instant::now() + timeout;
        self.begin_shutdown();
        let drained = tokio::time::timeout_at(deadline, async {
            let _guard = self.0.shutdown_lock.lock().await;
            self.drain().await;
        })
        .await
        .is_ok();
        ShutdownReport {
            drained,
            unfinished: lock(&self.0.state)
                .entries
                .values()
                .filter(|entry| !entry.record.phase.is_terminal() || !entry.task_attached || entry.task.is_some())
                .map(|entry| entry.record.clone())
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;
    use minnow_imaging::ImageAsset;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::sync::Notify;

    fn supervisor(concurrency: usize) -> (JobSupervisor, SessionRegistry) {
        let sessions = SessionRegistry::default();
        (
            JobSupervisor::with_admission(
                Handle::current(),
                sessions.clone(),
                ResourceAdmission::new(concurrency, concurrency, concurrency, concurrency),
            ),
            sessions,
        )
    }

    #[cfg(panic = "unwind")]
    #[tokio::test]
    async fn a_panicking_ready_future_is_reported_and_drained() {
        let (jobs, _) = supervisor(1);
        let job = jobs.spawn_after(
            "prepare",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            poll_fn(|_| -> Poll<OperationResult<()>> { panic!("ready failed") }),
            |_| async { Ok(()) },
        );
        let id = job.id();
        let error = job.await.unwrap_err();
        assert_eq!(error.kind, ErrorKind::Task);
        assert!(
            error.message.contains("ready failed"),
            "the ready future must use the same panic boundary as run"
        );
        assert!(jobs.shutdown_with_timeout(Duration::from_secs(1)).await.drained);
        assert_eq!(jobs.records().iter().filter(|record| record.id == id).count(), 1);
        assert!(matches!(jobs.records().last().unwrap().phase, JobPhase::Failed(_)));
    }

    #[tokio::test]
    async fn failed_execution_admission_settles_without_running_the_operation() {
        let (jobs, _) = supervisor(1);
        jobs.0.admission.capture.close();
        let ran = Arc::new(AtomicBool::new(false));
        let observed = ran.clone();
        let job = jobs.spawn(
            "closed execution resource",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            move |_| async move {
                observed.store(true, Ordering::SeqCst);
                Ok(())
            },
        );
        assert_eq!(job.await.unwrap_err().kind, ErrorKind::Unavailable);
        assert!(!ran.load(Ordering::SeqCst));
        assert!(jobs.shutdown().await.drained);
        assert_eq!(jobs.0.outstanding.capture.available_permits(), CAPTURE_OUTSTANDING_LIMIT);
    }

    #[tokio::test]
    async fn a_failed_join_reaches_terminal_state_whether_or_not_the_waiter_observes_it_first() {
        for observe_first in [false, true] {
            let (jobs, _) = supervisor(1);
            let job = jobs.spawn_after(
                "prepared worker",
                JobScope::Application,
                CompletionPolicy::PreserveResult,
                JobAdmission::InteractiveCapture,
                std::future::pending(),
                |_| async { Ok(()) },
            );
            let id = job.id();
            let mut phase = job.subscribe();
            // Test an unexpected async worker exit before any native work starts.
            lock(&jobs.0.state).entries.get(&id).unwrap().task.as_ref().unwrap().abort();
            let mut waiter = Some(job);
            if observe_first {
                assert_eq!(waiter.take().unwrap().await.unwrap_err().kind, ErrorKind::Task);
            } else {
                tokio::time::timeout(Duration::from_secs(1), phase.changed()).await.unwrap().unwrap();
                assert!(
                    phase.borrow().is_terminal(),
                    "a worker exit must settle its record without a waiter or shutdown driving it"
                );
            }
            let report = jobs.shutdown_with_timeout(Duration::from_secs(1)).await;
            assert!(report.drained);
            assert!(report.unfinished.is_empty());
            if let Some(job) = waiter {
                assert!(matches!(job.phase(), JobPhase::Failed(error) if error.kind == ErrorKind::Task));
                assert_eq!(job.await.unwrap_err().kind, ErrorKind::Task);
            }
            let records = jobs.records();
            assert_eq!(records.iter().filter(|record| record.id == id).count(), 1);
            assert!(matches!(records.last().unwrap().phase, JobPhase::Failed(_)));
            assert_eq!(jobs.0.outstanding.capture.available_permits(), CAPTURE_OUTSTANDING_LIMIT);
        }
    }

    #[cfg(panic = "unwind")]
    #[tokio::test]
    async fn a_cleanup_join_failure_does_not_rewrite_a_committed_completion() {
        struct PanicOnDrop;
        impl Drop for PanicOnDrop {
            fn drop(&mut self) {
                panic!("result cleanup failed");
            }
        }
        let (jobs, _) = supervisor(1);
        let (started_tx, started_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let job = jobs.spawn(
            "committed result",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            move |_| async move {
                let _ = started_tx.send(());
                let _ = finish_rx.await;
                Ok(PanicOnDrop)
            },
        );
        let id = job.id();
        started_rx.await.unwrap();
        drop(job);
        finish_tx.send(()).unwrap();
        assert!(jobs.shutdown_with_timeout(Duration::from_secs(1)).await.drained);
        assert_eq!(jobs.records().iter().filter(|record| record.id == id).count(), 1);
        assert_eq!(jobs.records().last().unwrap().phase, JobPhase::Succeeded);
        assert_eq!(jobs.0.outstanding.capture.available_permits(), CAPTURE_OUTSTANDING_LIMIT);
    }

    #[tokio::test]
    async fn every_purpose_rejects_excess_prepared_work_without_retaining_its_input_and_recovers_after_completion() {
        for (admission, limit) in [
            (JobAdmission::InteractiveCapture, CAPTURE_OUTSTANDING_LIMIT),
            (JobAdmission::QuickCapture, CAPTURE_OUTSTANDING_LIMIT),
            (JobAdmission::LongCapture, LONG_CAPTURE_OUTSTANDING_LIMIT),
            (JobAdmission::ImageOutput, IMAGE_OUTPUT_OUTSTANDING_LIMIT),
            (JobAdmission::Feedback, FEEDBACK_OUTSTANDING_LIMIT),
            (JobAdmission::Recognition, RECOGNITION_OUTSTANDING_LIMIT),
        ] {
            let (jobs, _) = supervisor(1);
            let mut accepted = Vec::new();
            for _ in 0..limit {
                accepted.push(jobs.spawn_after(
                    "prepared input",
                    JobScope::Application,
                    CompletionPolicy::PreserveResult,
                    admission,
                    std::future::pending(),
                    |_| async { Ok(()) },
                ));
            }
            let image = ImageAsset::new(RgbaImage::new(2, 2));
            let rejected_image = Arc::downgrade(&image);
            let rejected = jobs.spawn(
                "excess input",
                JobScope::Application,
                CompletionPolicy::PreserveResult,
                admission,
                move |_| async move { Ok(image) },
            );
            assert!(
                rejected_image.upgrade().is_none(),
                "a rejected request must release its snapshot synchronously"
            );
            assert_eq!(
                lock(&jobs.0.state).entries.len(),
                limit,
                "rejected work must not create a registry entry or worker"
            );
            assert!(matches!(rejected.phase(), JobPhase::Failed(error) if error.kind == ErrorKind::Busy));
            assert_eq!(rejected.await.unwrap_err().kind, ErrorKind::Busy);

            let cancelled = accepted.pop().unwrap();
            cancelled.cancel();
            assert_eq!(cancelled.await.unwrap_err().kind, ErrorKind::Cancelled);
            let recovered = jobs.spawn(
                "accepted after completion",
                JobScope::Application,
                CompletionPolicy::PreserveResult,
                admission,
                |_| async { Ok(7) },
            );
            assert_eq!(
                recovered.await.unwrap(),
                7,
                "prepared jobs count against outstanding work without taking execution capacity"
            );
            for job in accepted {
                job.cancel();
                assert_eq!(job.await.unwrap_err().kind, ErrorKind::Cancelled);
            }
            assert!(jobs.shutdown().await.drained);
        }
    }

    #[tokio::test]
    async fn stale_derived_results_are_not_published_to_a_new_revision() {
        let (jobs, sessions) = supervisor(2);
        let session = sessions.create(ImageAsset::new(RgbaImage::new(1, 1)), 1);
        let (started_tx, started_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let job = jobs.spawn(
            "recognize",
            session.scope().unwrap(),
            CompletionPolicy::RequireCurrentSession,
            JobAdmission::InteractiveCapture,
            move |_| async {
                let _ = started_tx.send(());
                let _ = finish_rx.await;
                Ok(7u8)
            },
        );
        started_rx.await.unwrap();
        sessions.publish_revision(session.id(), 2).unwrap();
        finish_tx.send(()).unwrap();
        assert_eq!(job.await.unwrap_err().kind, ErrorKind::Conflict);
        assert!(matches!(jobs.records().last().unwrap().phase, JobPhase::Superseded));
        jobs.shutdown().await;
    }

    #[tokio::test]
    async fn delayed_revision_invalidation_keeps_newer_work_running() {
        let (jobs, sessions) = supervisor(3);
        let session = sessions.create(ImageAsset::new(RgbaImage::new(1, 1)), 1);
        let (old_started_tx, old_started_rx) = oneshot::channel();
        let old = jobs.spawn(
            "old OCR",
            session.scope().unwrap(),
            CompletionPolicy::RequireCurrentSession,
            JobAdmission::InteractiveCapture,
            move |cancel| async move {
                let _ = old_started_tx.send(());
                cancel.cancelled().await;
                Err::<(), _>(OperationError::cancelled())
            },
        );
        old_started_rx.await.unwrap();
        sessions.publish_revision(session.id(), 2).unwrap();
        sessions.publish_revision(session.id(), 3).unwrap();
        let (current_started_tx, current_started_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let current = jobs.spawn(
            "current OCR",
            session.scope().unwrap(),
            CompletionPolicy::RequireCurrentSession,
            JobAdmission::InteractiveCapture,
            move |cancel| async move {
                let _ = current_started_tx.send(());
                tokio::select! {
                    _ = cancel.cancelled() => Err(OperationError::cancelled()),
                    _ = finish_rx => Ok(()),
                }
            },
        );
        current_started_rx.await.unwrap();
        // Revision 3's invalidation can finish before revision 2's delayed one.
        jobs.cancel_obsolete_session_jobs(session.id(), 3);
        jobs.cancel_obsolete_session_jobs(session.id(), 2);
        assert_eq!(old.await.unwrap_err().kind, ErrorKind::Cancelled);
        assert_eq!(current.phase(), JobPhase::Running);
        finish_tx.send(()).unwrap();
        current.await.unwrap();
        assert!(jobs.shutdown().await.drained);
    }

    #[tokio::test]
    async fn prepared_work_does_not_reserve_execution_capacity() {
        let (jobs, _) = supervisor(1);
        let (waiting_tx, waiting_rx) = oneshot::channel();
        let (_start_tx, start_rx) = oneshot::channel::<()>();
        let prepared = jobs.spawn_after(
            "prepared",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            async move {
                let _ = waiting_tx.send(());
                start_rx.await.map_err(|_| OperationError::cancelled())
            },
            |_| async { Ok(()) },
        );
        waiting_rx.await.unwrap();
        assert_eq!(prepared.phase(), JobPhase::Queued);
        let quick = jobs.spawn(
            "quick",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            |_| async { Ok(7) },
        );
        assert_eq!(tokio::time::timeout(Duration::from_secs(1), quick).await.unwrap().unwrap(), 7);
        prepared.cancel();
        assert_eq!(prepared.await.unwrap_err().kind, ErrorKind::Cancelled);
        assert!(jobs.shutdown().await.drained);
    }

    #[tokio::test]
    async fn shutdown_deadline_includes_waiting_for_another_drainer() {
        let (jobs, _) = supervisor(1);
        let (started_tx, started_rx) = oneshot::channel();
        let (cancelled_tx, cancelled_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let job = jobs.spawn(
            "uninterruptible operation",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            move |cancel| async move {
                let _ = started_tx.send(());
                cancel.cancelled().await;
                let _ = cancelled_tx.send(());
                let _ = release_rx.await;
                Ok(())
            },
        );
        started_rx.await.unwrap();
        let first_owner = jobs.clone();
        let first = tokio::spawn(async move { first_owner.shutdown().await });
        cancelled_rx.await.unwrap();
        let report = tokio::time::timeout(Duration::from_secs(1), jobs.shutdown_with_timeout(Duration::from_millis(10)))
            .await
            .expect("the shutdown deadline must include the mutex wait");
        assert!(!report.drained);
        assert_eq!(report.unfinished.len(), 1);
        assert_eq!(report.unfinished[0].id, job.id());
        release_tx.send(()).unwrap();
        assert!(first.await.unwrap().drained);
        job.await.unwrap();
        assert!(jobs.shutdown().await.drained);
    }

    #[tokio::test]
    async fn closing_a_view_does_not_suppress_the_requested_export_result() {
        let (jobs, sessions) = supervisor(2);
        let session = sessions.create(ImageAsset::new(RgbaImage::new(1, 1)), 1);
        let scope = session.scope().unwrap();
        let (finish_tx, finish_rx) = oneshot::channel();
        let job = jobs.spawn(
            "export",
            scope,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            move |_| async move {
                let _ = finish_rx.await;
                Ok(12u64)
            },
        );
        sessions.close(session.id());
        finish_tx.send(()).unwrap();
        assert_eq!(job.await.unwrap(), 12);
        let record = jobs.records().last().unwrap().clone();
        assert_eq!(record.phase, JobPhase::Succeeded);
        assert!(!record.ui_applicable);
        jobs.shutdown().await;
    }

    #[tokio::test]
    async fn queued_cancelled_work_is_never_executed() {
        let (jobs, _) = supervisor(1);
        let (started_tx, started_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let first = jobs.spawn(
            "hold permit",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            move |_| async {
                let _ = started_tx.send(());
                let _ = finish_rx.await;
                Ok(())
            },
        );
        started_rx.await.unwrap();
        let executed = Arc::new(AtomicBool::new(false));
        let flag = executed.clone();
        let queued = jobs.spawn(
            "queued",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            move |_| async move {
                flag.store(true, Ordering::SeqCst);
                Ok(())
            },
        );
        queued.cancel();
        assert_eq!(queued.await.unwrap_err().kind, ErrorKind::Cancelled);
        assert!(!executed.load(Ordering::SeqCst));
        finish_tx.send(()).unwrap();
        first.await.unwrap();
        jobs.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn running_blocking_work_is_tracked_until_it_really_finishes() {
        let (jobs, _) = supervisor(1);
        let release = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let worker_release = release.clone();
        let (started_tx, started_rx) = oneshot::channel();
        let job = jobs.spawn(
            "native write",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            move |_| async move {
                tokio::task::spawn_blocking(move || {
                    let _ = started_tx.send(());
                    let (mutex, condvar) = &*worker_release;
                    let mut done = mutex.lock().unwrap();
                    while !*done {
                        done = condvar.wait(done).unwrap();
                    }
                    9u8
                })
                .await
                .map_err(|error| OperationError::new(ErrorKind::Task, error.to_string()))
            },
        );
        started_rx.await.unwrap();
        job.cancel();
        assert_eq!(job.phase(), JobPhase::CancellationRequested);
        let available_after_cancellation = jobs.0.outstanding.capture.available_permits();
        let timed_out = jobs.shutdown_with_timeout(Duration::from_millis(10)).await;
        assert!(!timed_out.drained);
        assert_eq!(timed_out.unfinished.len(), 1);
        let (mutex, condvar) = &*release;
        *mutex.lock().unwrap() = true;
        condvar.notify_all();
        assert_eq!(job.await.unwrap(), 9);
        assert!(jobs.shutdown().await.drained);
        assert_eq!(available_after_cancellation, CAPTURE_OUTSTANDING_LIMIT - 1);
        assert_eq!(jobs.0.outstanding.capture.available_permits(), CAPTURE_OUTSTANDING_LIMIT);
        assert_eq!(jobs.records().last().unwrap().phase, JobPhase::Succeeded);
    }

    #[tokio::test]
    async fn dropped_waiters_do_not_drop_owned_work_and_shutdown_rejects_new_jobs() {
        let (jobs, _) = supervisor(1);
        let (started_tx, started_rx) = oneshot::channel();
        let release = Arc::new(Notify::new());
        let worker_release = release.clone();
        let completed = Arc::new(AtomicBool::new(false));
        let worker_completed = completed.clone();
        let job = jobs.spawn(
            "owned",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            move |_| async move {
                let _ = started_tx.send(());
                worker_release.notified().await;
                worker_completed.store(true, Ordering::SeqCst);
                Ok(())
            },
        );
        started_rx.await.unwrap();
        drop(job);
        release.notify_one();
        assert!(jobs.shutdown().await.drained);
        assert!(completed.load(Ordering::SeqCst));
        let rejected = jobs.spawn(
            "after shutdown",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            |_| async { Ok(()) },
        );
        assert_eq!(rejected.await.unwrap_err().kind, ErrorKind::Unavailable);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn timed_shutdown_retains_a_terminal_worker_until_result_cleanup_has_finished() {
        struct BlockingDrop {
            entered: Option<oneshot::Sender<()>>,
            release: Arc<(Mutex<bool>, std::sync::Condvar)>,
        }
        impl Drop for BlockingDrop {
            fn drop(&mut self) {
                let _ = self.entered.take().unwrap().send(());
                let (mutex, condition) = &*self.release;
                let mut released = mutex.lock().unwrap();
                while !*released {
                    released = condition.wait(released).unwrap();
                }
            }
        }

        let (jobs, _) = supervisor(1);
        let release = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let (running_tx, running_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let (cleanup_tx, cleanup_rx) = oneshot::channel();
        let worker_release = release.clone();
        let job = jobs.spawn(
            "result cleanup",
            JobScope::Application,
            CompletionPolicy::PreserveResult,
            JobAdmission::InteractiveCapture,
            move |_| async move {
                let _ = running_tx.send(());
                let _ = finish_rx.await;
                Ok(BlockingDrop {
                    entered: Some(cleanup_tx),
                    release: worker_release,
                })
            },
        );
        running_rx.await.unwrap();
        drop(job);
        finish_tx.send(()).unwrap();
        cleanup_rx.await.unwrap();

        let available_during_cleanup = jobs.0.outstanding.capture.available_permits();
        let first = jobs.shutdown_with_timeout(Duration::from_millis(10)).await;
        let second = jobs.shutdown_with_timeout(Duration::from_millis(10)).await;
        let (mutex, condition) = &*release;
        *mutex.lock().unwrap() = true;
        condition.notify_all();
        assert!(jobs.shutdown().await.drained);
        assert_eq!(available_during_cleanup, CAPTURE_OUTSTANDING_LIMIT - 1);
        assert_eq!(jobs.0.outstanding.capture.available_permits(), CAPTURE_OUTSTANDING_LIMIT);
        for report in [first, second] {
            assert!(!report.drained);
            assert_eq!(report.unfinished.len(), 1);
            assert_eq!(report.unfinished[0].phase, JobPhase::Succeeded);
        }
    }
}

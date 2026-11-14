use crate::{AppSettings, PersistenceStage, PersistenceStatus, SettingsError, SettingsRevision, lock_state};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) struct PersistenceProgress {
    persisted_revision: Option<SettingsRevision>,
    requested_revision: Option<SettingsRevision>,
    completed_revision: Option<SettingsRevision>,
    last_error: Option<SettingsError>,
}

impl PersistenceProgress {
    pub(crate) fn new(file_exists: bool) -> Self {
        Self {
            persisted_revision: file_exists.then_some(SettingsRevision::default()),
            requested_revision: None,
            completed_revision: None,
            last_error: None,
        }
    }

    pub(crate) fn status(&self, revision: SettingsRevision) -> PersistenceStatus {
        if self.persisted_revision.is_some_and(|persisted| persisted >= revision) {
            return PersistenceStatus::Persisted { revision };
        }
        if self.completed_revision == Some(revision)
            && let Some(error) = &self.last_error
        {
            return PersistenceStatus::Failed {
                revision,
                persisted_revision: self.persisted_revision,
                error: error.clone(),
            };
        }
        if self.requested_revision.is_none() && self.completed_revision.is_none() {
            return PersistenceStatus::Unwritten { revision };
        }
        PersistenceStatus::Pending {
            revision,
            persisted_revision: self.persisted_revision,
        }
    }

    pub(crate) fn requested(&mut self, revision: SettingsRevision) {
        self.requested_revision = Some(self.requested_revision.map_or(revision, |current| current.max(revision)));
    }

    fn record(&mut self, revision: SettingsRevision, result: &Result<(), SettingsError>) {
        self.completed_revision = Some(revision);
        self.last_error = result.as_ref().err().cloned();
        if result.is_ok() {
            self.persisted_revision = Some(revision);
        }
    }
}

struct SaveRequest {
    revision: SettingsRevision,
    config: AppSettings,
}

enum PersistenceCommand {
    Save(Box<SaveRequest>),
    Flush {
        latest: Box<SaveRequest>,
        reply: Sender<Result<(), SettingsError>>,
    },
    Shutdown {
        latest: Box<SaveRequest>,
    },
    #[cfg(test)]
    Pause {
        started: Sender<()>,
        resume: Receiver<()>,
    },
}

#[derive(Default)]
struct ShutdownCompletion {
    result: Mutex<Option<Result<(), SettingsError>>>,
    changed: Condvar,
}

impl ShutdownCompletion {
    fn complete(&self, result: Result<(), SettingsError>) {
        let mut completed = lock_state(&self.result);
        if completed.is_none() {
            *completed = Some(result);
            self.changed.notify_all();
        }
    }

    fn wait(&self, timeout: Option<Duration>, revision: SettingsRevision) -> Result<(), SettingsError> {
        let started = Instant::now();
        let mut result = lock_state(&self.result);
        loop {
            if let Some(result) = &*result {
                return result.clone();
            }
            result = match timeout {
                Some(timeout) => {
                    let remaining = timeout.saturating_sub(started.elapsed());
                    if remaining.is_zero() {
                        return Err(SettingsError::ShutdownTimeout { revision });
                    }
                    self.changed
                        .wait_timeout(result, remaining)
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .0
                }
                None => self.changed.wait(result).unwrap_or_else(|poisoned| poisoned.into_inner()),
            };
        }
    }
}

pub(crate) struct SettingsWriter {
    sender: Sender<PersistenceCommand>,
    worker: Option<JoinHandle<()>>,
    shutdown: Arc<ShutdownCompletion>,
}

impl SettingsWriter {
    pub(crate) fn start(path: PathBuf, progress: Arc<Mutex<PersistenceProgress>>) -> Result<Self, SettingsError> {
        let (sender, receiver) = mpsc::channel();
        let shutdown = Arc::new(ShutdownCompletion::default());
        let worker_shutdown = shutdown.clone();
        let worker = thread::Builder::new()
            .name("minnow-settings-writer".to_string())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| writer_loop(receiver, &path, &progress)))
                    .unwrap_or(Err(SettingsError::WriterUnavailable));
                worker_shutdown.complete(result);
            })
            .map_err(|error| SettingsError::WriterStart { message: error.to_string() })?;
        Ok(Self {
            sender,
            worker: Some(worker),
            shutdown,
        })
    }

    pub(crate) fn enqueue(&self, revision: SettingsRevision, config: AppSettings) -> Result<(), SettingsError> {
        self.sender
            .send(PersistenceCommand::Save(Box::new(SaveRequest { revision, config })))
            .map_err(|_| SettingsError::WriterUnavailable)
    }

    pub(crate) fn flush_barrier(
        &self,
        revision: SettingsRevision,
        config: AppSettings,
    ) -> Result<Receiver<Result<(), SettingsError>>, SettingsError> {
        let (reply, receiver) = mpsc::channel();
        self.sender
            .send(PersistenceCommand::Flush {
                latest: Box::new(SaveRequest { revision, config }),
                reply,
            })
            .map_err(|_| SettingsError::WriterUnavailable)?;
        Ok(receiver)
    }

    pub(crate) fn flush_and_stop(&mut self, revision: SettingsRevision, config: AppSettings) -> Result<(), SettingsError> {
        let result = self
            .request_shutdown(revision, config)
            .and_then(|()| self.wait_for_shutdown(None, revision));
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            return Err(SettingsError::WriterUnavailable);
        }
        result
    }

    pub(crate) fn request_shutdown(&self, revision: SettingsRevision, config: AppSettings) -> Result<(), SettingsError> {
        self.sender
            .send(PersistenceCommand::Shutdown {
                latest: Box::new(SaveRequest { revision, config }),
            })
            .map_err(|_| {
                self.shutdown.complete(Err(SettingsError::WriterUnavailable));
                SettingsError::WriterUnavailable
            })
    }

    pub(crate) fn wait_for_shutdown(&self, timeout: Option<Duration>, revision: SettingsRevision) -> Result<(), SettingsError> {
        self.shutdown.wait(timeout, revision)
    }

    #[cfg(test)]
    pub(crate) fn pause(&self) -> Sender<()> {
        let (started, ready) = mpsc::channel();
        let (resume, continued) = mpsc::channel();
        self.sender
            .send(PersistenceCommand::Pause { started, resume: continued })
            .expect("pause writer");
        ready.recv().expect("writer paused");
        resume
    }
}

fn writer_loop(receiver: Receiver<PersistenceCommand>, path: &Path, progress: &Mutex<PersistenceProgress>) -> Result<(), SettingsError> {
    while let Ok(command) = receiver.recv() {
        match command {
            PersistenceCommand::Save(request) => {
                // The progress value retains asynchronous failures for callers.
                let _ = commit_request(&request, path, progress);
            }
            PersistenceCommand::Flush { latest, reply } => {
                let _ = reply.send(commit_request(&latest, path, progress));
            }
            PersistenceCommand::Shutdown { latest } => return commit_request(&latest, path, progress),
            #[cfg(test)]
            PersistenceCommand::Pause { started, resume } => {
                let _ = started.send(());
                let _ = resume.recv();
            }
        }
    }
    Err(SettingsError::WriterUnavailable)
}

fn commit_request(request: &SaveRequest, path: &Path, progress: &Mutex<PersistenceProgress>) -> Result<(), SettingsError> {
    // A barrier may repeat an already committed revision. Never rewrite a newer
    // snapshot with an earlier one, including during retry and shutdown.
    if lock_state(progress)
        .persisted_revision
        .is_some_and(|revision| revision >= request.revision)
    {
        return Ok(());
    }
    let result = persist_snapshot(&request.config, path);
    lock_state(progress).record(request.revision, &result);
    if let Err(error) = &result {
        tracing::error!("Failed to persist settings revision {}: {error}", request.revision.get());
    }
    result
}

fn persist_snapshot(config: &AppSettings, path: &Path) -> Result<(), SettingsError> {
    let contents = toml::to_string_pretty(config).map_err(|error| SettingsError::Serialize { message: error.to_string() })?;
    let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|error| persist_error(path, PersistenceStage::CreateParent, error))?;

    let permissions = match fs::metadata(path) {
        Ok(metadata) => {
            let permissions = metadata.permissions();
            if permissions.readonly() {
                return Err(persist_error(
                    path,
                    PersistenceStage::InspectTarget,
                    io::Error::new(io::ErrorKind::PermissionDenied, "settings file is read-only"),
                ));
            }
            Some(permissions)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(persist_error(path, PersistenceStage::InspectTarget, error)),
    };

    let mut temporary = tempfile::Builder::new()
        .prefix(".minnow-settings-")
        .tempfile_in(parent)
        .map_err(|error| persist_error(path, PersistenceStage::CreateTemporary, error))?;
    temporary
        .write_all(contents.as_bytes())
        .and_then(|_| temporary.flush())
        .map_err(|error| persist_error(path, PersistenceStage::WriteTemporary, error))?;
    if let Some(permissions) = permissions {
        temporary
            .as_file()
            .set_permissions(permissions)
            .map_err(|error| persist_error(path, PersistenceStage::WriteTemporary, error))?;
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| persist_error(path, PersistenceStage::SyncTemporary, error))?;
    // The temporary lives beside the target, so this remains a same-filesystem
    // atomic replacement. No fallible operation follows the commit point.
    temporary
        .persist(path)
        .map_err(|error| persist_error(path, PersistenceStage::Commit, error.error))?;
    Ok(())
}

fn persist_error(path: &Path, stage: PersistenceStage, error: io::Error) -> SettingsError {
    SettingsError::Persist {
        path: path.to_path_buf(),
        stage,
        kind: error.kind(),
        message: error.to_string(),
    }
}

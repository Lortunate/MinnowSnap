//! Desired application preferences with an explicit path and one serialized writer.
//!
//! Native registration and activation state belong to the application and platform
//! adapters. Accepting a setting here does not imply that an OS operation succeeded.

mod directory;
mod persistence;
mod schema;

pub use directory::{SaveDirectory, SaveDirectoryError};
pub use schema::*;

use persistence::{PersistenceProgress, SettingsWriter};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// A monotonically increasing revision within one store.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct SettingsRevision(u64);

impl SettingsRevision {
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistenceStage {
    CreateParent,
    InspectTarget,
    CreateTemporary,
    WriteTemporary,
    SyncTemporary,
    Commit,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SettingsError {
    #[error("failed to read settings at {path}: {message}")]
    Read { path: PathBuf, kind: io::ErrorKind, message: String },
    #[error("failed to parse settings at {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("failed to serialize settings: {message}")]
    Serialize { message: String },
    #[error("failed to persist settings at {path} during {stage:?}: {message}")]
    Persist {
        path: PathBuf,
        stage: PersistenceStage,
        kind: io::ErrorKind,
        message: String,
    },
    #[error("failed to start the settings writer: {message}")]
    WriterStart { message: String },
    #[error("the settings writer is no longer available")]
    WriterUnavailable,
    #[error("settings revision counter is exhausted")]
    RevisionExhausted,
    #[error("settings no longer accept changes after shutdown has started")]
    Closed,
    #[error("shutdown timed out before persistence of settings revision {revision:?} was confirmed; the writer may still complete")]
    ShutdownTimeout { revision: SettingsRevision },
}

/// Persistence state for the latest accepted desired-settings revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistenceStatus {
    /// No filesystem is associated with this explicitly constructed store.
    InMemory {
        revision: SettingsRevision,
    },
    /// Defaults are in memory, but no file has been created or write requested yet.
    Unwritten {
        revision: SettingsRevision,
    },
    Pending {
        revision: SettingsRevision,
        persisted_revision: Option<SettingsRevision>,
    },
    Persisted {
        revision: SettingsRevision,
    },
    Failed {
        revision: SettingsRevision,
        persisted_revision: Option<SettingsRevision>,
        error: SettingsError,
    },
}

/// How the initial desired preferences were obtained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingsLoadStatus {
    Loaded,
    Missing,
    InMemory,
    /// Defaults are usable, but the saved user intent remains unknown.
    Degraded {
        error: SettingsError,
    },
}

/// Constructors for an owned settings store. Cloned handles share its single writer.
pub struct SettingsStore;

/// Without explicit shutdown, the final handle flushes and joins its writer.
///
/// After [`Self::shutdown`] starts, dropping handles does not wait again: an OS
/// write that exceeded its deadline retains its own state and may finish later.
#[derive(Clone)]
pub struct SettingsHandle(Arc<StoreInner>);

struct StoreInner {
    state: Mutex<StoreState>,
    path: Option<PathBuf>,
    progress: Arc<Mutex<PersistenceProgress>>,
    writer: Option<SettingsWriter>,
    load_status: SettingsLoadStatus,
}

struct StoreState {
    config: AppSettings,
    revision: SettingsRevision,
    accepting: bool,
}

impl SettingsStore {
    /// Open a file-backed store. Missing files use defaults without writing.
    ///
    /// Read and parse failures are returned intact. In particular, a corrupt file
    /// is never silently replaced with defaults. A caller may explicitly choose
    /// [`Self::in_memory`] as a degraded mode after presenting the error.
    pub fn open(path: impl Into<PathBuf>) -> Result<SettingsHandle, SettingsError> {
        let path = path.into();
        let (config, file_exists) = match fs::read_to_string(&path) {
            Ok(contents) => {
                let config = toml::from_str(&contents).map_err(|error| SettingsError::Parse {
                    path: path.clone(),
                    message: error.to_string(),
                })?;
                (config, true)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (AppSettings::default(), false),
            Err(error) => {
                return Err(SettingsError::Read {
                    path,
                    kind: error.kind(),
                    message: error.to_string(),
                });
            }
        };
        let progress = Arc::new(Mutex::new(PersistenceProgress::new(file_exists)));
        let writer = SettingsWriter::start(path.clone(), progress.clone())?;
        Ok(SettingsHandle(Arc::new(StoreInner {
            state: Mutex::new(StoreState {
                config,
                revision: SettingsRevision::default(),
                accepting: true,
            }),
            path: Some(path),
            progress,
            writer: Some(writer),
            load_status: if file_exists {
                SettingsLoadStatus::Loaded
            } else {
                SettingsLoadStatus::Missing
            },
        })))
    }

    /// Create an explicitly non-persistent store, useful for degraded operation.
    pub fn in_memory(initial: AppSettings) -> SettingsHandle {
        Self::memory_store(initial, SettingsLoadStatus::InMemory)
    }

    /// Preserve a load failure while providing explicitly non-persistent defaults.
    pub fn degraded(initial: AppSettings, error: SettingsError) -> SettingsHandle {
        Self::memory_store(initial, SettingsLoadStatus::Degraded { error })
    }

    fn memory_store(initial: AppSettings, load_status: SettingsLoadStatus) -> SettingsHandle {
        SettingsHandle(Arc::new(StoreInner {
            state: Mutex::new(StoreState {
                config: initial,
                revision: SettingsRevision::default(),
                accepting: true,
            }),
            path: None,
            progress: Arc::new(Mutex::new(PersistenceProgress::new(false))),
            writer: None,
            load_status,
        }))
    }
}

impl SettingsHandle {
    pub fn load_status(&self) -> SettingsLoadStatus {
        self.0.load_status.clone()
    }

    pub fn snapshot(&self) -> AppSettings {
        lock_state(&self.0.state).config.clone()
    }

    pub fn revision(&self) -> SettingsRevision {
        lock_state(&self.0.state).revision
    }

    /// Accept a desired change and enqueue its snapshot.
    ///
    /// Success acknowledges the in-memory revision and its queued write, not disk
    /// persistence. Observe [`Self::persistence_status`] or call [`Self::flush`]
    /// when persistence confirmation is needed.
    pub fn apply(&self, action: SettingsAction) -> Result<SettingsRevision, SettingsError> {
        self.apply_with_snapshot(action).map(|(revision, _)| revision)
    }

    /// Accept a change and return its exact desired snapshot under the same lock.
    ///
    /// Native activation can use this value without rebuilding the untouched
    /// fields from native state or racing a separate snapshot read.
    pub fn apply_with_snapshot(&self, action: SettingsAction) -> Result<(SettingsRevision, AppSettings), SettingsError> {
        let mut state = lock_state(&self.0.state);
        if !state.accepting {
            return Err(SettingsError::Closed);
        }
        let revision = SettingsRevision(state.revision.0.checked_add(1).ok_or(SettingsError::RevisionExhausted)?);
        let mut config = state.config.clone();
        config.apply(action);
        if let Some(writer) = &self.0.writer {
            // Keep enqueue order identical to accepted revision order across clones.
            writer.enqueue(revision, config.clone())?;
            lock_state(&self.0.progress).requested(revision);
        }
        state.config = config;
        state.revision = revision;
        Ok((revision, state.config.clone()))
    }

    /// Confirm persistence of every change accepted before this call's barrier.
    ///
    /// The returned status also reflects any concurrent later changes; success
    /// confirms the barrier revision, not a newer revision accepted afterwards.
    /// A previous failed revision is retried with the latest snapshot. In-memory
    /// stores never write files and return [`PersistenceStatus::InMemory`].
    pub fn flush(&self) -> Result<PersistenceStatus, SettingsError> {
        let reply = {
            let state = lock_state(&self.0.state);
            if !state.accepting {
                drop(state);
                if let Some(writer) = &self.0.writer {
                    writer.wait_for_shutdown(None, self.revision())?;
                }
                return Ok(self.persistence_status());
            }
            let reply = self
                .0
                .writer
                .as_ref()
                .map(|writer| writer.flush_barrier(state.revision, state.config.clone()))
                .transpose()?;
            if reply.is_some() {
                lock_state(&self.0.progress).requested(state.revision);
            }
            reply
        };
        if let Some(reply) = reply {
            reply.recv().map_err(|_| SettingsError::WriterUnavailable)??;
        }
        Ok(self.persistence_status())
    }

    /// Close mutation admission and confirm the final desired revision within a
    /// time budget. All cloned handles observe the same irreversible shutdown.
    ///
    /// A timeout means persistence is unconfirmed, not cancelled. The original
    /// writer retains its work and will exit after that final write. Subsequent
    /// calls can wait again; no additional writes are queued. After this method
    /// starts, final-handle Drop never performs an unbounded flush or join.
    /// [`Self::flush`] remains an unbounded wait for that same final result.
    pub fn shutdown(&self, timeout: Duration) -> Result<PersistenceStatus, SettingsError> {
        let started = Instant::now();
        let revision = {
            let mut state = lock_state(&self.0.state);
            if state.accepting {
                state.accepting = false;
                if let Some(writer) = &self.0.writer {
                    writer.request_shutdown(state.revision, state.config.clone())?;
                    lock_state(&self.0.progress).requested(state.revision);
                }
            }
            state.revision
        };
        if let Some(writer) = &self.0.writer {
            writer.wait_for_shutdown(Some(timeout.saturating_sub(started.elapsed())), revision)?;
        }
        Ok(self.persistence_status())
    }

    pub fn persistence_status(&self) -> PersistenceStatus {
        let revision = lock_state(&self.0.state).revision;
        if self.0.writer.is_none() {
            return PersistenceStatus::InMemory { revision };
        }
        lock_state(&self.0.progress).status(revision)
    }

    pub fn path(&self) -> Option<&Path> {
        self.0.path.as_deref()
    }
}

impl Drop for StoreInner {
    fn drop(&mut self) {
        if let Some(writer) = &mut self.writer {
            let state = lock_state(&self.state);
            if !state.accepting {
                return;
            }
            if let Err(error) = writer.flush_and_stop(state.revision, state.config.clone()) {
                tracing::error!("Failed to flush settings during final-handle shutdown: {error}");
            }
        }
    }
}

fn lock_state<T>(state: &Mutex<T>) -> MutexGuard<'_, T> {
    state.lock().unwrap_or_else(|poisoned| {
        tracing::error!("Settings state lock was poisoned; recovering the last owned value");
        poisoned.into_inner()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_handle_joins_the_worker_before_releasing_the_store() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let handle = SettingsStore::open(directory.path().join("config.toml")).expect("open preferences");
        let progress = handle.0.progress.clone();
        assert!(Arc::strong_count(&progress) >= 2);
        handle.apply(SettingsAction::Theme(THEME_DARK.to_string())).expect("accept theme");
        drop(handle);
        assert_eq!(Arc::strong_count(&progress), 1, "writer must have released its state before Drop returns");
    }

    #[test]
    fn shutdown_timeout_does_not_cancel_the_write_or_make_final_drop_wait_again() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("config.toml");
        let handle = SettingsStore::open(&path).expect("open preferences");
        let resume = handle.0.writer.as_ref().expect("file writer").pause();
        let progress = handle.0.progress.clone();
        let revision = handle.apply(SettingsAction::Theme(THEME_DARK.to_string())).expect("accept theme");

        let started = Instant::now();
        assert_eq!(
            handle.shutdown(Duration::from_millis(20)),
            Err(SettingsError::ShutdownTimeout { revision }),
        );
        assert!(started.elapsed() < Duration::from_secs(1), "shutdown must respect its time budget");
        assert_eq!(handle.apply(SettingsAction::Language("en_US".to_string())), Err(SettingsError::Closed));
        assert!(matches!(handle.persistence_status(), PersistenceStatus::Pending { .. }));
        assert!(!path.exists(), "timeout must not pretend that the blocked write committed");
        // A second waiter has its own budget, rather than queuing behind the
        // first call or admitting a second final write.
        assert_eq!(handle.shutdown(Duration::ZERO), Err(SettingsError::ShutdownTimeout { revision }));

        let (dropped, observed_drop) = std::sync::mpsc::channel();
        let dropper = std::thread::spawn(move || {
            drop(handle);
            dropped.send(()).expect("report final drop");
        });
        let drop_result = observed_drop.recv_timeout(Duration::from_secs(1));
        // Release even when the assertion fails, so a regression cannot leave
        // the test process stuck in an intentionally blocked writer.
        resume.send(()).expect("release blocked writer");
        drop_result.expect("final drop must not resume an unbounded shutdown wait");
        dropper.join().expect("drop completed");

        let deadline = Instant::now() + Duration::from_secs(2);
        while Arc::strong_count(&progress) != 1 {
            assert!(
                Instant::now() < deadline,
                "detached writer must eventually complete and release its state"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(lock_state(&progress).status(revision), PersistenceStatus::Persisted { revision });
        let persisted: AppSettings = toml::from_str(&fs::read_to_string(path).expect("read final settings")).expect("parse settings");
        assert_eq!(persisted.general.theme, THEME_DARK);
    }

    #[test]
    fn concurrent_shutdown_waiters_share_completion_without_sharing_their_timeout() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let handle = SettingsStore::open(directory.path().join("config.toml")).expect("open preferences");
        let resume = handle.0.writer.as_ref().expect("file writer").pause();
        let revision = handle.apply(SettingsAction::Theme(THEME_DARK.to_string())).expect("accept theme");
        let waiter = handle.clone();
        let waiting = std::thread::spawn(move || waiter.shutdown(Duration::from_secs(2)));
        let deadline = Instant::now() + Duration::from_secs(1);
        while lock_state(&handle.0.state).accepting {
            assert!(Instant::now() < deadline, "shutdown waiter must close admission");
            std::thread::yield_now();
        }
        assert_eq!(handle.shutdown(Duration::ZERO), Err(SettingsError::ShutdownTimeout { revision }));
        resume.send(()).expect("release blocked writer");
        assert_eq!(
            waiting.join().expect("long waiter returns"),
            Ok(PersistenceStatus::Persisted { revision })
        );
        assert_eq!(handle.flush(), Ok(PersistenceStatus::Persisted { revision }));
        assert_eq!(handle.shutdown(Duration::ZERO), Ok(PersistenceStatus::Persisted { revision }));
    }
}

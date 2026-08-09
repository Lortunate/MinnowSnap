use super::AppSettings;
use crate::services::paths::ensure_parent_dir;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use tracing::{error, info};

struct SaveRequest {
    config: AppSettings,
    path: PathBuf,
}

enum PersistenceCommand {
    Save(Box<SaveRequest>),
    Flush(Sender<Result<(), String>>),
    Shutdown,
}

/// Serializes settings writes behind one worker so older snapshots cannot
/// overtake newer ones. In-memory settings remain owned by `SettingsStore`;
/// this adapter owns only filesystem side effects.
pub(super) struct SettingsPersistence {
    sender: Option<Sender<PersistenceCommand>>,
    worker: Option<JoinHandle<()>>,
}

impl SettingsPersistence {
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        match thread::Builder::new()
            .name("minnow-settings-writer".to_string())
            .spawn(move || writer_loop(receiver))
        {
            Ok(worker) => Self {
                sender: Some(sender),
                worker: Some(worker),
            },
            Err(err) => {
                error!("Failed to start settings writer; writes will run inline: {err}");
                Self { sender: None, worker: None }
            }
        }
    }

    pub(super) fn enqueue(&self, config: AppSettings, path: PathBuf) {
        if let Some(sender) = &self.sender
            && sender
                .send(PersistenceCommand::Save(Box::new(SaveRequest {
                    config: config.clone(),
                    path: path.clone(),
                })))
                .is_ok()
        {
            return;
        }

        error!("Settings writer is unavailable; persisting the latest snapshot inline");
        if let Err(err) = persist_snapshot(&config, &path) {
            error!("Failed to persist settings inline: {err}");
        }
    }

    pub(super) fn flush_latest(&self, latest: &AppSettings, path: &Path) -> Result<(), String> {
        let worker_result = self.flush_worker();
        match worker_result {
            Ok(()) => Ok(()),
            Err(worker_error) => {
                error!("Settings writer flush failed; retrying the latest snapshot inline: {worker_error}");
                persist_snapshot(latest, path)
            }
        }
    }

    fn flush_worker(&self) -> Result<(), String> {
        let Some(sender) = &self.sender else {
            return Err("settings writer is not running".to_string());
        };
        let (result_tx, result_rx) = mpsc::channel();
        sender
            .send(PersistenceCommand::Flush(result_tx))
            .map_err(|_| "settings writer disconnected before flush".to_string())?;
        result_rx.recv().map_err(|_| "settings writer disconnected while flushing".to_string())?
    }
}

impl Drop for SettingsPersistence {
    fn drop(&mut self) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(PersistenceCommand::Shutdown);
        }
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            error!("Settings writer panicked during shutdown");
        }
    }
}

fn writer_loop(receiver: Receiver<PersistenceCommand>) {
    let mut last_result = Ok(());
    while let Ok(command) = receiver.recv() {
        match command {
            PersistenceCommand::Save(request) => {
                last_result = persist_snapshot(&request.config, &request.path);
                if let Err(err) = &last_result {
                    error!("Failed to persist settings: {err}");
                }
            }
            PersistenceCommand::Flush(result_tx) => {
                let _ = result_tx.send(last_result.clone());
            }
            PersistenceCommand::Shutdown => return,
        }
    }
}

fn persist_snapshot(config: &AppSettings, path: &Path) -> Result<(), String> {
    let contents = toml::to_string_pretty(config).map_err(|err| format!("failed to serialize settings: {err}"))?;
    ensure_parent_dir(path).map_err(|err| format!("failed to create config directory for {}: {err}", path.display()))?;
    fs::write(path, contents).map_err(|err| format!("failed to write config file {}: {err}", path.display()))?;
    info!("Settings saved to {:?}", path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_FILE_ID: AtomicU64 = AtomicU64::new(0);

    fn test_path() -> PathBuf {
        let id = TEST_FILE_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("minnowsnap-settings-writer-{}-{id}.toml", std::process::id()))
    }

    #[test]
    fn flush_persists_the_latest_enqueued_snapshot() {
        let path = test_path();
        let persistence = SettingsPersistence::new();
        let mut first = AppSettings::default();
        first.general.theme = "Dark".to_string();
        let mut latest = first.clone();
        latest.general.theme = "Light".to_string();
        latest.general.language = "zh-CN".to_string();

        persistence.enqueue(first, path.clone());
        persistence.enqueue(latest.clone(), path.clone());
        persistence.flush_latest(&latest, &path).expect("flush latest settings");

        let contents = fs::read_to_string(&path).expect("read persisted settings");
        let persisted: AppSettings = toml::from_str(&contents).expect("parse persisted settings");
        assert_eq!(persisted.general.theme, "Light");
        assert_eq!(persisted.general.language, "zh-CN");

        drop(persistence);
        let _ = fs::remove_file(path);
    }
}

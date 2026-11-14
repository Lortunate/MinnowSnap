//! Validated model generations, atomic publication, leases, and retention.
use super::{ArtifactValidator, ModelPaths, ModelSources, PreparedModels, check_cancelled, io_error};
use minnow_application::ports::{ErrorKind, OperationError, OperationResult};
use minnow_application::settings::OcrModelType;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub(crate) struct ModelStore {
    legacy_root: PathBuf,
    root: PathBuf,
    model_key: String,
    sources: Arc<ModelSources>,
    max_bytes: u64,
    leases: Arc<Mutex<HashMap<String, Weak<PreparedModels>>>>,
    transaction: Arc<Mutex<()>>,
}

/// A validated, immutable staging payload. Only ModelStore can construct it.
pub(crate) struct PreparedCandidate {
    staging: TempDir,
    content_id: String,
    transaction: Arc<Mutex<()>>,
}

struct PublicationSnapshot {
    pointer_bytes: Option<Vec<u8>>,
    pointer: Option<ActivePointer>,
    existing: Vec<Arc<PreparedModels>>,
    rollback: Vec<Arc<PreparedModels>>,
}

#[derive(Serialize, Deserialize)]
struct ActivePointer {
    generation: String,
    #[serde(default)]
    previous: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: u8,
    model: String,
    files: Vec<ArtifactRecord>,
    #[serde(default)]
    catalog: Option<CatalogRecord>,
}

#[derive(Serialize, Deserialize, PartialEq)]
struct CatalogRecord {
    identity: String,
    dictionary_entries: usize,
    decode: String,
}

#[derive(Serialize, Deserialize)]
struct ArtifactRecord {
    name: String,
    bytes: u64,
    sha256: String,
}

impl ModelStore {
    pub(crate) fn new(root: PathBuf, model: OcrModelType, sources: ModelSources, max_bytes: u64) -> OperationResult<Self> {
        sources.validate()?;
        let model_key = match model {
            OcrModelType::Mobile => "mobile",
            OcrModelType::Server => "server",
        }
        .to_string();
        Ok(Self {
            root: root.join(&model_key),
            legacy_root: root,
            model_key,
            sources: Arc::new(sources),
            max_bytes,
            leases: Arc::new(Mutex::new(HashMap::new())),
            transaction: Arc::new(Mutex::new(())),
        })
    }

    fn paths(&self, directory: &Path) -> ModelPaths {
        ModelPaths {
            detector: directory.join(self.sources.detector().name()),
            recognizer: directory.join(self.sources.recognizer().name()),
            keys: directory.join(self.sources.dictionary().name()),
            spec: self.sources.clone(),
        }
    }

    pub(crate) fn staging(&self) -> OperationResult<TempDir> {
        fs::create_dir_all(self.root.join("generations")).map_err(io_error)?;
        tempfile::Builder::new().prefix(".staging-").tempdir_in(&self.root).map_err(io_error)
    }

    pub(crate) fn sources(&self) -> &ModelSources {
        &self.sources
    }

    /// Snapshot a lease before expensive validation so retirement cannot remove
    /// its files. Retry if another publication changed the authoritative pointer.
    pub(crate) fn load_existing(
        &self,
        validator: &dyn ArtifactValidator,
        cancel: &CancellationToken,
    ) -> OperationResult<Option<Arc<PreparedModels>>> {
        loop {
            check_cancelled(cancel)?;
            let snapshot = {
                let _transaction = self.transaction.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                self.pointer_bytes()?
                    .map(|bytes| {
                        let pointer: ActivePointer = serde_json::from_slice(&bytes).map_err(validation_error)?;
                        let prepared = self.lease_generation(&pointer.generation)?;
                        Ok::<_, OperationError>((bytes, prepared))
                    })
                    .transpose()?
            };
            let Some((bytes, prepared)) = snapshot else { break };
            let validated = self.validate_generation(&prepared, validator, cancel);
            let _transaction = self.transaction.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if self.pointer_bytes()?.as_ref() != Some(&bytes) {
                continue;
            }
            validated?;
            return Ok(Some(prepared));
        }
        let present = self
            .sources
            .artifacts()
            .iter()
            .filter(|artifact| self.legacy_root.join(artifact.name()).exists())
            .count();
        if present == 0 {
            return Ok(None);
        }
        if present != 3
            || !self
                .sources
                .artifacts()
                .iter()
                .all(|artifact| self.legacy_root.join(artifact.name()).is_file())
        {
            return Err(OperationError::new(
                ErrorKind::Validation,
                format!(
                    "legacy model spec {} is incomplete or contains a non-file artifact",
                    self.sources.identity()
                ),
            ));
        }
        let staging = self.staging()?;
        for artifact in self.sources.artifacts() {
            check_cancelled(cancel)?;
            fs::copy(self.legacy_root.join(artifact.name()), staging.path().join(artifact.name())).map_err(io_error)?;
        }
        let candidate = self.prepare_candidate(staging, validator, cancel)?;
        self.publish_candidate(candidate, validator, cancel).map(Some)
    }

    /// All payload hashing and native validation happen before the publication
    /// transaction. The private staging directory remains owned by this value.
    pub(crate) fn prepare_candidate(
        &self,
        staging: TempDir,
        validator: &dyn ArtifactValidator,
        cancel: &CancellationToken,
    ) -> OperationResult<PreparedCandidate> {
        check_cancelled(cancel)?;
        let files = self
            .sources
            .artifacts()
            .iter()
            .map(|artifact| self.verified_record(staging.path(), artifact, cancel))
            .collect::<OperationResult<Vec<_>>>()?;
        validator.validate(&self.paths(staging.path()))?;
        check_cancelled(cancel)?;
        let manifest = Manifest {
            format: 2,
            model: self.model_key.clone(),
            files,
            catalog: Some(self.catalog_record()),
        };
        let bytes = serde_json::to_vec(&manifest).map_err(validation_error)?;
        let content_id = hex_digest(&Sha256::digest(&bytes));
        let mut file = File::create(staging.path().join("manifest.json")).map_err(io_error)?;
        file.write_all(&bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        Ok(PreparedCandidate {
            staging,
            content_id,
            transaction: self.transaction.clone(),
        })
    }

    pub(crate) fn publish_candidate(
        &self,
        candidate: PreparedCandidate,
        validator: &dyn ArtifactValidator,
        cancel: &CancellationToken,
    ) -> OperationResult<Arc<PreparedModels>> {
        if !Arc::ptr_eq(&self.transaction, &candidate.transaction) {
            return Err(OperationError::new(
                ErrorKind::InvalidInput,
                "model candidate belongs to a different store",
            ));
        }
        loop {
            check_cancelled(cancel)?;
            let snapshot = self.publication_snapshot(&candidate.content_id)?;
            // These leases outlive validation and the transaction. Neither a
            // concurrent prune nor publication can retire a selected directory.
            let existing = snapshot
                .existing
                .iter()
                .find(|prepared| self.validate_generation(prepared, validator, cancel).is_ok());
            let rollback = snapshot.rollback.iter().find(|prepared| {
                existing.is_some_and(|existing| existing.generation == prepared.generation)
                    || self.validate_generation(prepared, validator, cancel).is_ok()
            });
            let _transaction = self.transaction.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            check_cancelled(cancel)?;
            if self.pointer_bytes()? != snapshot.pointer_bytes {
                continue;
            }
            let generation = if let Some(existing) = existing {
                existing.generation.clone()
            } else if self.root.join("generations").join(&candidate.content_id).exists() {
                // Never overwrite a corrupt directory still leased by an engine.
                let nonce = candidate.staging.path().file_name().unwrap().to_string_lossy();
                format!("{}-repair-{}", candidate.content_id, nonce.trim_start_matches(".staging-"))
            } else {
                candidate.content_id.clone()
            };
            let final_path = self.root.join("generations").join(&generation);
            if existing.is_none() {
                fs::rename(candidate.staging.path(), &final_path).map_err(io_error)?;
            }
            let previous = snapshot.pointer.and_then(|pointer| {
                if pointer.generation == generation {
                    pointer.previous
                } else {
                    rollback.map(|prepared| prepared.generation.clone())
                }
            });
            let mut pointer = tempfile::NamedTempFile::new_in(&self.root).map_err(io_error)?;
            serde_json::to_writer(
                &mut pointer,
                &ActivePointer {
                    generation: generation.clone(),
                    previous,
                },
            )
            .map_err(validation_error)?;
            pointer.flush().map_err(io_error)?;
            pointer.as_file().sync_all().map_err(io_error)?;
            check_cancelled(cancel)?;
            pointer.persist(self.root.join("active.json")).map_err(|error| io_error(error.error))?;
            // Cancellation after this commit cannot report a cancelled update.
            return Ok(self.track(generation, self.paths(&final_path)));
        }
    }

    fn pointer_bytes(&self) -> OperationResult<Option<Vec<u8>>> {
        match fs::read(self.root.join("active.json")) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(io_error(error)),
        }
    }

    fn publication_snapshot(&self, content_id: &str) -> OperationResult<PublicationSnapshot> {
        // Directory enumeration and payload validation do not occupy the short
        // transaction. Acquiring leases below safely handles a concurrent prune.
        let names = fs::read_dir(self.root.join("generations"))
            .map_err(io_error)?
            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()).map_err(io_error))
            .collect::<OperationResult<Vec<_>>>()?;
        let _transaction = self.transaction.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let pointer_bytes = self.pointer_bytes()?;
        let pointer = pointer_bytes
            .as_deref()
            .and_then(|bytes| serde_json::from_slice::<ActivePointer>(bytes).ok());
        // The directory list can predate a concurrent commit. Always include
        // the current pointer and canonical name captured under this transaction.
        let mut seen = HashSet::new();
        let existing = pointer
            .iter()
            .map(|pointer| pointer.generation.as_str())
            .chain(std::iter::once(content_id))
            .chain(names.iter().map(String::as_str))
            .filter(|name| generation_content_id(name).ok() == Some(content_id) && seen.insert((*name).to_owned()))
            .map(|name| self.lease_generation(name))
            .collect::<OperationResult<Vec<_>>>()?;
        let rollback = pointer
            .iter()
            .flat_map(|pointer| std::iter::once(&pointer.generation).chain(pointer.previous.iter()))
            .filter_map(|name| self.lease_generation(name).ok())
            .collect();
        Ok(PublicationSnapshot {
            pointer_bytes,
            pointer,
            existing,
            rollback,
        })
    }

    fn lease_generation(&self, generation: &str) -> OperationResult<Arc<PreparedModels>> {
        generation_content_id(generation)?;
        Ok(self.track(generation.into(), self.paths(&self.root.join("generations").join(generation))))
    }

    fn validate_generation(&self, prepared: &PreparedModels, validator: &dyn ArtifactValidator, cancel: &CancellationToken) -> OperationResult<()> {
        let content_id = generation_content_id(&prepared.generation)?;
        let directory = self.root.join("generations").join(&prepared.generation);
        if !fs::symlink_metadata(&directory).is_ok_and(|metadata| metadata.file_type().is_dir()) {
            return Err(OperationError::new(ErrorKind::Validation, "model generation is not an owned directory"));
        }
        let bytes = fs::read(directory.join("manifest.json")).map_err(io_error)?;
        if hex_digest(&Sha256::digest(&bytes)) != content_id {
            return Err(OperationError::new(ErrorKind::Validation, "model manifest checksum mismatch"));
        }
        let manifest: Manifest = serde_json::from_slice(&bytes).map_err(validation_error)?;
        let expected = self.sources.artifacts();
        if !self.matches_catalog(&manifest) || manifest.files.len() != expected.len() {
            return Err(OperationError::new(
                ErrorKind::Validation,
                "model manifest does not describe this model set",
            ));
        }
        for (record, artifact) in manifest.files.iter().zip(expected) {
            let name = artifact.name();
            if record.name != name {
                return Err(OperationError::new(
                    ErrorKind::Validation,
                    "model manifest contains an unexpected artifact",
                ));
            }
            let actual = self.verified_record(&directory, artifact, cancel)?;
            if actual.bytes != record.bytes || actual.sha256 != record.sha256 {
                return Err(OperationError::new(
                    ErrorKind::Validation,
                    format!("model artifact checksum mismatch: {name}"),
                ));
            }
        }
        check_cancelled(cancel)?;
        let paths = self.paths(&directory);
        validator.validate(&paths)?;
        check_cancelled(cancel)?;
        Ok(())
    }

    fn track(&self, generation: String, paths: ModelPaths) -> Arc<PreparedModels> {
        let mut leases = self.leases.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(prepared) = leases.get(&generation).and_then(Weak::upgrade) {
            return prepared;
        }
        let prepared = Arc::new(PreparedModels { generation, paths });
        leases.insert(prepared.generation.clone(), Arc::downgrade(&prepared));
        prepared
    }

    /// Keep current, one rollback and every live inference lease. Unknown
    /// directories, additional files, symlinks and historical root files are
    /// never owned by this cleanup operation.
    pub(crate) fn prune_unleased(&self) {
        let _transaction = self.transaction.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        self.prune_locked();
    }

    #[cfg(test)]
    pub(crate) fn lock_transaction_for_test(&self) -> std::sync::MutexGuard<'_, ()> {
        self.transaction.lock().unwrap()
    }

    fn prune_locked(&self) {
        let Some(pointer) = fs::read(self.root.join("active.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<ActivePointer>(&bytes).ok())
        else {
            return;
        };
        let mut leases = self.leases.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        leases.retain(|_, lease| lease.strong_count() > 0);
        let mut keep: HashSet<String> = leases.keys().cloned().collect();
        keep.insert(pointer.generation);
        keep.extend(pointer.previous);
        let Ok(entries) = fs::read_dir(self.root.join("generations")) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if keep.contains(&name) || !self.owns_generation_directory(&entry.path(), &name) {
                continue;
            }
            if let Err(error) = fs::remove_dir_all(entry.path()) {
                tracing::warn!(generation = name, %error, "could not retire unused OCR model generation");
            }
        }
    }

    fn owns_generation_directory(&self, directory: &Path, generation: &str) -> bool {
        let Ok(content_id) = generation_content_id(generation) else {
            return false;
        };
        if !fs::symlink_metadata(directory).is_ok_and(|metadata| metadata.file_type().is_dir()) {
            return false;
        }
        let Ok(bytes) = fs::read(directory.join("manifest.json")) else {
            return false;
        };
        if hex_digest(&Sha256::digest(&bytes)) != content_id {
            return false;
        }
        let Ok(manifest) = serde_json::from_slice::<Manifest>(&bytes) else {
            return false;
        };
        let expected = self.sources.artifacts();
        if !self.matches_catalog(&manifest)
            || manifest.files.len() != expected.len()
            || !manifest.files.iter().zip(expected).all(|(record, artifact)| {
                record.name == artifact.name() && record.bytes == artifact.expected_bytes() && record.sha256 == artifact.expected_sha256()
            })
        {
            return false;
        }
        let allowed: HashSet<&str> = expected.iter().map(|artifact| artifact.name()).chain(["manifest.json"]).collect();
        fs::read_dir(directory).is_ok_and(|entries| {
            entries.into_iter().all(|entry| {
                entry.is_ok_and(|entry| {
                    allowed.contains(entry.file_name().to_string_lossy().as_ref()) && entry.file_type().is_ok_and(|kind| kind.is_file())
                })
            })
        })
    }

    fn catalog_record(&self) -> CatalogRecord {
        CatalogRecord {
            identity: self.sources.identity().into(),
            dictionary_entries: self.sources.dictionary_entries(),
            decode: "paddle-ctc-blank0-all-lines-append-space-v1".into(),
        }
    }

    fn matches_catalog(&self, manifest: &Manifest) -> bool {
        manifest.model == self.model_key
            && match manifest.format {
                // Old manifests establish no trusted identity. Their payloads must
                // still match every current expected hash before being accepted.
                1 => manifest.catalog.is_none(),
                2 => manifest.catalog.as_ref() == Some(&self.catalog_record()),
                _ => false,
            }
    }

    fn verified_record(&self, directory: &Path, artifact: &crate::ArtifactSpec, cancel: &CancellationToken) -> OperationResult<ArtifactRecord> {
        let record = self.record(directory, artifact.name(), cancel).map_err(|error| {
            OperationError::new(
                error.kind,
                format!("model spec {} artifact {}: {}", self.sources.identity(), artifact.name(), error.message),
            )
        })?;
        if record.bytes != artifact.expected_bytes() || record.sha256 != artifact.expected_sha256() {
            return Err(OperationError::new(
                ErrorKind::Validation,
                format!(
                    "model spec {} artifact {} does not match its expected size/SHA256",
                    self.sources.identity(),
                    artifact.name()
                ),
            ));
        }
        Ok(record)
    }

    fn record(&self, directory: &Path, name: &str, cancel: &CancellationToken) -> OperationResult<ArtifactRecord> {
        let path = directory.join(name);
        let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
        if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > self.max_bytes {
            return Err(OperationError::new(
                ErrorKind::Validation,
                format!("invalid model artifact size or type: {name}"),
            ));
        }
        let mut file = File::open(path).map_err(io_error)?;
        let mut hash = Sha256::new();
        let mut total = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            check_cancelled(cancel)?;
            let count = file.read(&mut buffer).map_err(io_error)?;
            if count == 0 {
                break;
            }
            total += count as u64;
            if total > self.max_bytes {
                return Err(OperationError::new(ErrorKind::Validation, "model artifact grew beyond its byte budget"));
            }
            hash.update(&buffer[..count]);
        }
        if total != metadata.len() {
            return Err(OperationError::new(
                ErrorKind::Validation,
                "model artifact changed while it was being validated",
            ));
        }
        Ok(ArtifactRecord {
            name: name.into(),
            bytes: total,
            sha256: hex_digest(&hash.finalize()),
        })
    }
}

fn generation_content_id(generation: &str) -> OperationResult<&str> {
    let (content_id, repair) = generation
        .split_once("-repair-")
        .map_or((generation, None), |(content_id, repair)| (content_id, Some(repair)));
    if content_id.len() != 64
        || !content_id.bytes().all(|byte| byte.is_ascii_hexdigit())
        || repair.is_some_and(|nonce| nonce.is_empty() || nonce.len() > 32 || !nonce.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    {
        return Err(OperationError::new(ErrorKind::Validation, "invalid model generation identifier"));
    }
    Ok(content_id)
}

fn validation_error(error: impl std::fmt::Display) -> OperationError {
    OperationError::new(ErrorKind::Validation, error.to_string())
}

fn hex_digest(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(text, "{byte:02x}").expect("writing into a String cannot fail");
    }
    text
}

//! Native acceptance is ignored by default and has two explicit asset modes.
//!
//! Run one model per process (ORT owns one process environment):
//! `MINNOW_NATIVE_OCR_MODEL=Mobile MINNOW_NATIVE_OCR_ASSETS=/local/model/files cargo test -p minnow-recognition --test native_ocr -- --ignored`
//! The source directory is read only; service work uses a TempDir. CI instead sets
//! `MINNOW_NATIVE_OCR_DOWNLOAD=1 MINNOW_NATIVE_OCR_CACHE_DIR=/absolute/ci/cache`
//! to let RecognitionService download and verify its fixed catalog. Never combine
//! the two modes. Repeat with `Server` in a separate process.
//! CI also needs an external process/job deadline: an already running native
//! call cannot be stopped by dropping a timed-out Rust future.
#![cfg(feature = "local-ocr")]

use minnow_application::ports::{ModelDownloadStatus, RecognitionModels, TextRecognition};
use minnow_imaging::ImageAsset;
use minnow_recognition::{ModelSources, OcrModelType, RecognitionOptions, RecognitionService};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
enum AssetMode {
    Offline(PathBuf),
    DownloadCache(PathBuf),
}

fn asset_mode(assets: Option<PathBuf>, cache: Option<PathBuf>, download: Option<OsString>) -> Result<AssetMode, String> {
    match (assets, cache, download) {
        (Some(assets), None, None) if !assets.as_os_str().is_empty() => Ok(AssetMode::Offline(assets)),
        (None, Some(cache), Some(download)) if download == "1" => {
            if !cache.is_absolute()
                || cache.parent().is_none()
                || cache
                    .components()
                    .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
            {
                return Err("MINNOW_NATIVE_OCR_CACHE_DIR must be an absolute non-root path without parent traversal".into());
            }
            Ok(AssetMode::DownloadCache(cache))
        }
        _ => Err("select exactly one mode: MINNOW_NATIVE_OCR_ASSETS, or MINNOW_NATIVE_OCR_DOWNLOAD=1 plus MINNOW_NATIVE_OCR_CACHE_DIR".into()),
    }
}

fn prepare_cache(root: &Path) -> Result<PathBuf, String> {
    const MARKER: &str = "MinnowSnap native OCR acceptance cache v1\n";
    if root.is_symlink() {
        return Err("native OCR cache root must not be a symlink".into());
    }
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    let marker = root.join(".native-ocr-cache-v1");
    match std::fs::read_to_string(&marker) {
        Ok(contents) if contents == MARKER => {}
        Ok(_) => return Err("native OCR cache has an unrecognized ownership marker".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if std::fs::read_dir(root).map_err(|error| error.to_string())?.next().is_some() {
                return Err("refuse a nonempty native OCR cache without its ownership marker".into());
            }
            std::fs::write(marker, MARKER).map_err(|error| error.to_string())?;
        }
        Err(error) => return Err(error.to_string()),
    }
    // RecognitionService already owns per-model directories and generations.
    Ok(root.to_path_buf())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires explicit model and offline assets or opt-in verified-download cache"]
async fn native_ocr_recognizes_committed_fixture() {
    let model_type: OcrModelType = std::env::var("MINNOW_NATIVE_OCR_MODEL")
        .expect("set MINNOW_NATIVE_OCR_MODEL to Mobile or Server")
        .parse()
        .expect("MINNOW_NATIVE_OCR_MODEL must be Mobile or Server");
    let mode = asset_mode(
        std::env::var_os("MINNOW_NATIVE_OCR_ASSETS").map(PathBuf::from),
        std::env::var_os("MINNOW_NATIVE_OCR_CACHE_DIR").map(PathBuf::from),
        std::env::var_os("MINNOW_NATIVE_OCR_DOWNLOAD"),
    )
    .expect("explicit native OCR asset mode");
    let sources = ModelSources::for_model(model_type);
    let temporary = matches!(&mode, AssetMode::Offline(_)).then(|| tempfile::tempdir().expect("create isolated model directory"));
    let (models_dir, allow_download) = match mode {
        AssetMode::Offline(assets) => {
            assert!(assets.is_dir(), "MINNOW_NATIVE_OCR_ASSETS must be an existing directory");
            let directory = temporary.as_ref().unwrap().path();
            for artifact in [sources.detector(), sources.recognizer(), sources.dictionary()] {
                std::fs::copy(assets.join(artifact.name()), directory.join(artifact.name()))
                    .unwrap_or_else(|error| panic!("copy local {}: {error}", artifact.name()));
            }
            (directory.to_path_buf(), false)
        }
        AssetMode::DownloadCache(root) => (prepare_cache(&root).expect("prepare owned CI model cache"), true),
    };
    let pixels = image::load_from_memory(include_bytes!("fixtures/minnowsnap-2026.png"))
        .expect("decode committed fixture")
        .to_rgba8();
    let service = RecognitionService::new(
        models_dir,
        tokio::runtime::Handle::current(),
        RecognitionOptions {
            model_type,
            ..Default::default()
        },
    )
    .expect("construct recognition service");
    let cancel = CancellationToken::new();
    let deadline = Duration::from_secs(90);
    let result = async {
        service.availability().require_available().map_err(|error| error.to_string())?;
        if allow_download {
            tokio::time::timeout(Duration::from_secs(8 * 60), service.ensure_models(false, cancel.clone()))
                .await
                .map_err(|error| format!("verified model download deadline: {error}"))?
                .map_err(|error| error.to_string())?;
        } else {
            let mut status = service.subscribe_model_status();
            // A failed offline import stops here. Calling recognize before
            // readiness would authorize downloads, forbidden in this mode.
            tokio::time::timeout(deadline, async {
                loop {
                    let snapshot = status.borrow_and_update().clone();
                    if snapshot.is_ready() {
                        return Ok::<(), String>(());
                    }
                    if let ModelDownloadStatus::Failed { message } = snapshot.download {
                        return Err(message);
                    }
                    status.changed().await.map_err(|error| error.to_string())?;
                }
            })
            .await
            .map_err(|error| format!("offline import deadline: {error}"))??;
        }
        if !service.model_status().is_ready() {
            return Err("model preparation did not publish a verified generation".into());
        }
        tokio::time::timeout(deadline, service.recognize(ImageAsset::new(pixels), cancel.clone()))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())
    }
    .await;
    cancel.cancel();
    // A bad recognition result must still drain inference and release leases.
    tokio::time::timeout(deadline, service.shutdown())
        .await
        .expect("shutdown deadline")
        .expect("shutdown service");
    drop(service);
    let blocks = result.expect("verified model preparation and native recognition");
    let text = blocks.iter().map(|block| block.text.as_str()).collect::<Vec<_>>().join(" ");
    eprintln!("native OCR ({model_type}): {} block(s), text={text:?}", blocks.len());
    let normalized: String = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    assert_eq!(normalized, "minnowsnap2026", "model {model_type}: unexpected native OCR output {text:?}");
}

#[test]
fn native_asset_modes_require_an_unambiguous_network_opt_in() {
    let assets = PathBuf::from("offline-assets");
    let cache = std::env::temp_dir().join("native-ocr-mode-contract");
    assert!(matches!(asset_mode(Some(assets.clone()), None, None), Ok(AssetMode::Offline(_))));
    assert!(matches!(
        asset_mode(None, Some(cache.clone()), Some("1".into())),
        Ok(AssetMode::DownloadCache(_))
    ));
    for (source, destination, download) in [
        (None, None, None),
        (Some(PathBuf::new()), None, None),
        (Some(assets.clone()), Some(cache.clone()), Some("1".into())),
        (Some(assets), None, Some("1".into())),
        (None, Some(cache.clone()), None),
        (None, None, Some("1".into())),
        (None, Some(cache.clone()), Some("true".into())),
        (None, Some(cache), Some("0".into())),
        (None, Some(PathBuf::from("relative-cache")), Some("1".into())),
    ] {
        assert!(asset_mode(source, destination, download).is_err());
    }
}

#[test]
fn native_cache_does_not_adopt_unowned_model_directories() {
    let temporary = tempfile::tempdir().unwrap();
    let foreign = temporary.path().join("existing-model");
    std::fs::write(&foreign, b"leave existing files untouched").unwrap();
    assert!(prepare_cache(temporary.path()).is_err());
    assert_eq!(std::fs::read(&foreign).unwrap(), b"leave existing files untouched");
    std::fs::remove_file(foreign).unwrap();
    assert_eq!(prepare_cache(temporary.path()).unwrap(), temporary.path());
    assert_eq!(prepare_cache(temporary.path()).unwrap(), temporary.path());
    std::fs::write(temporary.path().join(".native-ocr-cache-v1"), "another owner").unwrap();
    assert!(prepare_cache(temporary.path()).is_err());
}

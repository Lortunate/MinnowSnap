use minnow_settings::{
    AppSettings, DEFAULT_CAPTURE_SHORTCUT, DEFAULT_QUICK_CAPTURE_SHORTCUT, OcrModelType, PersistenceStage, PersistenceStatus, SaveDirectory,
    SaveDirectoryError, SettingsAction, SettingsError, SettingsLoadStatus, SettingsStore, THEME_DARK, THEME_SYSTEM,
};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier, mpsc};
use std::thread;
use std::time::{Duration, Instant};

fn read_settings(path: &Path) -> AppSettings {
    toml::from_str(&fs::read_to_string(path).expect("read settings")).expect("parse complete settings")
}

fn write_settings(path: &Path, settings: &AppSettings) {
    fs::write(path, toml::to_string_pretty(settings).expect("serialize settings")).expect("write settings");
}

#[test]
fn load_origin_distinguishes_unknown_saved_intent_from_missing_and_loaded_settings() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("config.toml");
    let missing = SettingsStore::open(&path).expect("open missing settings");
    assert_eq!(missing.load_status(), SettingsLoadStatus::Missing);
    assert!(!path.exists());
    drop(missing);

    let loaded = SettingsStore::open(&path).expect("open persisted defaults");
    assert_eq!(loaded.load_status(), SettingsLoadStatus::Loaded);
    drop(loaded);

    fs::write(&path, "[broken").expect("write corrupt preferences");
    let error = SettingsStore::open(&path).err().expect("corruption must be returned");
    let degraded = SettingsStore::degraded(AppSettings::default(), error.clone());
    degraded.apply(SettingsAction::AutoStart(true)).expect("accept temporary intent");
    assert_eq!(degraded.load_status(), SettingsLoadStatus::Degraded { error });
    assert!(matches!(degraded.flush(), Ok(PersistenceStatus::InMemory { .. })));
    assert_eq!(fs::read_to_string(path).expect("corrupt file preserved"), "[broken");
}

#[test]
fn existing_partial_toml_retains_section_defaults_and_is_not_rewritten_by_flush() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("config.toml");
    let original = "# existing preferences\n[general]\ntheme = \"Dark\"\n";
    fs::write(&path, original).expect("write partial preferences");

    let store = SettingsStore::open(&path).expect("open partial preferences");
    let settings = store.snapshot();
    assert_eq!(settings.general.theme, THEME_DARK);
    assert_eq!(settings.general.language, "System");
    assert_eq!(settings.shortcuts.capture, DEFAULT_CAPTURE_SHORTCUT);
    assert_eq!(settings.shortcuts.quick_capture, DEFAULT_QUICK_CAPTURE_SHORTCUT);
    assert!(settings.output.oxipng_enabled);
    assert!(settings.notification.enabled);
    assert!(!settings.ocr.enabled);
    assert_eq!(settings.ocr.model_type, OcrModelType::Mobile);
    assert!(matches!(store.flush(), Ok(PersistenceStatus::Persisted { .. })));
    drop(store);
    assert_eq!(fs::read_to_string(path).expect("read original preferences"), original);
}

#[test]
fn existing_full_toml_keeps_all_current_fields() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("config.toml");
    let original = r#"
[general]
theme = "Dark"
language = "zh_CN"
font_family = "Example Font"
auto_start = true
[shortcuts]
capture = "Ctrl+Shift+1"
quick_capture = "Ctrl+Shift+2"
[output]
save_path = "/captures"
oxipng_enabled = false
[ocr]
enabled = true
model_type = "Server"
[notification]
enabled = false
save_notification = false
copy_notification = false
qr_code_notification = false
shutter_sound = false
"#;
    fs::write(&path, original).expect("write current schema");
    let expected: AppSettings = toml::from_str(original).expect("parse expected preferences");
    let store = SettingsStore::open(&path).expect("open current schema");
    assert_eq!(store.snapshot(), expected);

    store.apply(SettingsAction::Theme("Light".to_string())).expect("accept one change");
    store.flush().expect("persist one change");
    let mut persisted = read_settings(&path);
    persisted.general.theme = "Dark".to_string();
    assert_eq!(persisted, expected);
}

#[test]
fn ocr_model_names_remain_compatible_and_unknown_values_preserve_the_file() {
    for (name, expected) in [
        ("Mobile", OcrModelType::Mobile),
        ("mobile", OcrModelType::Mobile),
        (" MOBILE ", OcrModelType::Mobile),
        ("Server", OcrModelType::Server),
        ("server", OcrModelType::Server),
        ("sErVeR", OcrModelType::Server),
    ] {
        let settings: AppSettings = toml::from_str(&format!("[ocr]\nmodel_type = {name:?}")).expect("supported model name");
        assert_eq!(settings.ocr.model_type, expected);
        assert!(
            toml::to_string(&settings)
                .expect("serialize model")
                .contains(&format!("model_type = \"{expected}\""))
        );
    }

    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("config.toml");
    let original = "[ocr]\nmodel_type = \"Experimental\"";
    fs::write(&path, original).expect("write unsupported model");
    let error = SettingsStore::open(&path).err().expect("unknown model must not silently select Mobile");
    assert!(matches!(error, SettingsError::Parse { ref message, .. } if message.contains("expected Mobile or Server")));
    assert_eq!(fs::read_to_string(path).expect("read preserved settings"), original);
}

#[test]
fn corrupt_file_is_returned_as_an_error_and_preserved() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("config.toml");
    let original = "general = [ definitely not valid TOML";
    fs::write(&path, original).expect("write damaged preferences");

    assert!(matches!(SettingsStore::open(&path), Err(SettingsError::Parse { .. })));
    assert_eq!(fs::read_to_string(&path).expect("read damaged preferences"), original);
    assert_eq!(fs::read_dir(directory.path()).expect("read directory").count(), 1);
}

#[test]
fn unreadable_target_does_not_become_a_default_store() {
    let directory = tempfile::tempdir().expect("temporary directory");
    assert!(matches!(SettingsStore::open(directory.path()), Err(SettingsError::Read { .. })));
    assert!(directory.path().is_dir());
}

#[test]
fn missing_file_uses_defaults_until_a_write_is_requested() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("nested").join("config.toml");
    let store = SettingsStore::open(&path).expect("open defaults");
    assert_eq!(store.snapshot(), AppSettings::default());
    assert_eq!(store.path(), Some(path.as_path()));
    assert!(matches!(store.persistence_status(), PersistenceStatus::Unwritten { .. }));
    assert!(!path.exists());

    store.apply(SettingsAction::Language("en_US".to_string())).expect("accept language");
    let revision = store.revision();
    assert_eq!(store.flush().expect("flush defaults"), PersistenceStatus::Persisted { revision });
    assert_eq!(read_settings(&path).general.language, "en_US");
}

#[test]
fn legacy_directory_loading_and_actions_share_default_and_exact_custom_path_semantics() {
    let directory = tempfile::tempdir().expect("temporary directory");
    for (index, path) in ["", "   ", "\t\r\n", "/captures", "  /captures  ", "截图/with space"]
        .into_iter()
        .enumerate()
    {
        let file = directory.path().join(format!("{index}.toml"));
        let original = format!("[output]\nsave_path = {path:?}\n");
        fs::write(&file, &original).expect("write legacy directory");
        let store = SettingsStore::open(&file).expect("load legacy directory");
        let desired = SaveDirectory::new(path).expect("representable directory");
        assert_eq!(store.snapshot().output.save_path, desired);
        assert_eq!(desired.as_custom_str(), (!path.trim().is_empty()).then_some(path));
        assert_eq!(desired.as_custom_path(), (!path.trim().is_empty()).then(|| Path::new(path)));
        store.flush().expect("confirm untouched file");
        assert_eq!(fs::read_to_string(&file).expect("read untouched legacy settings"), original);

        store
            .apply(SettingsAction::SavePath(desired.clone()))
            .expect("accept validated directory");
        store.flush().expect("persist validated directory");
        assert_eq!(read_settings(&file).output.save_path, desired);
        let written = fs::read_to_string(file).expect("read canonical settings");
        assert_eq!(
            written.contains("save_path"),
            !desired.is_default(),
            "default directory is represented by an omitted field"
        );
    }
}

#[test]
fn invalid_native_directories_are_rejected_before_desired_revision_changes() {
    let store = SettingsStore::in_memory(AppSettings::default());
    let before = store.snapshot();
    let revision = store.revision();
    let result = SaveDirectory::new("bad\0directory").map(|directory| store.apply(SettingsAction::SavePath(directory)));
    assert_eq!(result, Err(SaveDirectoryError::ContainsNul));

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let native = std::path::PathBuf::from(std::ffi::OsString::from_vec(b"/captures/\xff".to_vec()));
        let result = SaveDirectory::new(native).map(|directory| store.apply(SettingsAction::SavePath(directory)));
        assert_eq!(result, Err(SaveDirectoryError::NonUtf8));
    }
    assert_eq!(store.snapshot(), before);
    assert_eq!(store.revision(), revision);
    assert_eq!(store.persistence_status(), PersistenceStatus::InMemory { revision });
}

#[test]
fn desired_settings_mutations_keep_the_existing_empty_value_contract() {
    let store = SettingsStore::in_memory(AppSettings::default());
    for action in [
        SettingsAction::Theme("Dark".to_string()),
        SettingsAction::Language("en_US".to_string()),
        SettingsAction::FontFamily("Example Font".to_string()),
        SettingsAction::SavePath(SaveDirectory::new("/captures").expect("valid directory")),
        SettingsAction::OxipngEnabled(false),
        SettingsAction::AutoStart(true),
        SettingsAction::OcrEnabled(true),
        SettingsAction::NotificationEnabled(false),
        SettingsAction::SaveNotification(false),
        SettingsAction::CopyNotification(false),
        SettingsAction::QrCodeNotification(false),
        SettingsAction::ShutterSound(false),
        SettingsAction::Shortcuts {
            capture: "Ctrl+1".to_string(),
            quick_capture: "Ctrl+2".to_string(),
        },
    ] {
        store.apply(action).expect("accept desired preference");
    }
    let settings = store.snapshot();
    assert_eq!(settings.general.theme, "Dark");
    assert_eq!(settings.general.language, "en_US");
    assert_eq!(settings.general.font_family.as_deref(), Some("Example Font"));
    assert!(settings.general.auto_start);
    assert_eq!(settings.output.save_path.as_custom_str(), Some("/captures"));
    assert!(!settings.output.oxipng_enabled);
    assert!(settings.ocr.enabled);
    assert!(!settings.notification.enabled);
    assert!(!settings.notification.save_notification);
    assert!(!settings.notification.copy_notification);
    assert!(!settings.notification.qr_code_notification);
    assert!(!settings.notification.shutter_sound);
    assert_eq!(settings.shortcuts.capture, "Ctrl+1");
    assert_eq!(settings.shortcuts.quick_capture, "Ctrl+2");

    store.apply(SettingsAction::FontFamily(String::new())).expect("clear font");
    store.apply(SettingsAction::SavePath(SaveDirectory::default())).expect("clear directory");
    assert_eq!(store.snapshot().general.font_family, None);
    assert!(store.snapshot().output.save_path.is_default());
}

#[test]
fn in_memory_mode_never_claims_filesystem_persistence() {
    let store = SettingsStore::in_memory(AppSettings::default());
    let revision = store.apply(SettingsAction::Theme(THEME_DARK.to_string())).expect("accept theme");
    assert_eq!(store.path(), None);
    assert_eq!(store.persistence_status(), PersistenceStatus::InMemory { revision });
    assert_eq!(store.flush().expect("flush memory"), PersistenceStatus::InMemory { revision });
    assert_eq!(store.snapshot().general.theme, THEME_DARK);
}

#[test]
fn write_failure_is_visible_and_keeps_the_last_good_file() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("config.toml");
    write_settings(&path, &AppSettings::default());
    let original = fs::read(&path).expect("read last good file");
    let original_permissions = fs::metadata(&path).expect("read permissions").permissions();
    let store = SettingsStore::open(&path).expect("open last good file");
    let mut read_only = original_permissions.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&path, read_only).expect("protect last good file");

    let revision = store
        .apply(SettingsAction::Theme(THEME_DARK.to_string()))
        .expect("accept theme in memory");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !matches!(store.persistence_status(), PersistenceStatus::Failed { .. }) {
        assert!(Instant::now() < deadline, "asynchronous failure must become observable without flush");
        thread::sleep(Duration::from_millis(1));
    }
    let error = store.flush().expect_err("read-only target must not be overwritten");
    assert!(matches!(
        error,
        SettingsError::Persist {
            stage: PersistenceStage::InspectTarget,
            ..
        }
    ));
    assert!(matches!(
        store.persistence_status(),
        PersistenceStatus::Failed {
            revision: failed,
            persisted_revision: Some(_),
            ..
        } if failed == revision
    ));
    assert_eq!(store.snapshot().general.theme, THEME_DARK);
    assert_eq!(fs::read(&path).expect("read protected file"), original);
    assert_eq!(fs::read_dir(directory.path()).expect("read directory").count(), 1);
    drop(store);
    assert_eq!(fs::read(&path).expect("read file after failed shutdown flush"), original);
    fs::set_permissions(path, original_permissions).expect("restore permissions");
}

#[test]
fn a_failed_revision_can_be_retried_without_losing_its_desired_value() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("config.toml");
    write_settings(&path, &AppSettings::default());
    let original_permissions = fs::metadata(&path).expect("read permissions").permissions();
    let store = SettingsStore::open(&path).expect("open preferences");
    let mut read_only = original_permissions.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&path, read_only).expect("protect file");

    let revision = store
        .apply(SettingsAction::Language("zh_CN".to_string()))
        .expect("accept desired language");
    store.flush().expect_err("write fails while protected");
    fs::set_permissions(&path, original_permissions).expect("restore file");
    assert_eq!(store.flush().expect("retry latest"), PersistenceStatus::Persisted { revision });
    assert_eq!(read_settings(&path).general.language, "zh_CN");
}

#[test]
fn cloned_handles_accept_revisions_in_the_same_order_the_writer_commits_them() {
    const WRITERS: usize = 8;
    const CHANGES_PER_WRITER: usize = 12;
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("config.toml");
    let store = SettingsStore::open(&path).expect("open preferences");
    let barrier = Arc::new(Barrier::new(WRITERS));
    let (sender, receiver) = mpsc::channel();
    thread::scope(|scope| {
        for writer in 0..WRITERS {
            let handle = store.clone();
            let barrier = barrier.clone();
            let sender = sender.clone();
            scope.spawn(move || {
                barrier.wait();
                for change in 0..CHANGES_PER_WRITER {
                    let value = format!("{writer}:{change}");
                    let revision = handle.apply(SettingsAction::Language(value.clone())).expect("accept revision");
                    sender.send((revision, value)).expect("record accepted revision");
                }
            });
        }
    });
    drop(sender);
    let accepted = receiver.into_iter().collect::<Vec<_>>();
    assert_eq!(accepted.len(), WRITERS * CHANGES_PER_WRITER);
    let (revision, expected) = accepted.into_iter().max_by_key(|(revision, _)| *revision).expect("accepted changes");
    assert_eq!(
        store.flush().expect("flush concurrent changes"),
        PersistenceStatus::Persisted { revision }
    );
    assert_eq!(store.snapshot().general.language, expected);
    assert_eq!(read_settings(&path).general.language, expected);
}

#[test]
fn flush_barriers_confirm_prior_changes_while_other_handles_keep_mutating() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("config.toml");
    let store = SettingsStore::open(&path).expect("open preferences");
    let barrier = Arc::new(Barrier::new(4));
    thread::scope(|scope| {
        for writer in 0..3 {
            let handle = store.clone();
            let barrier = barrier.clone();
            scope.spawn(move || {
                barrier.wait();
                for change in 0..12 {
                    handle
                        .apply(SettingsAction::Language(format!("{writer}:{change}")))
                        .expect("accept concurrent revision");
                }
            });
        }
        let handle = store.clone();
        let barrier = barrier.clone();
        scope.spawn(move || {
            barrier.wait();
            for _ in 0..8 {
                let before = handle.revision();
                let status = handle.flush().expect("flush concurrent barrier");
                let confirmed = match status {
                    PersistenceStatus::Persisted { revision } => revision,
                    PersistenceStatus::Pending {
                        persisted_revision: Some(revision),
                        ..
                    } => revision,
                    unexpected => panic!("barrier must confirm prior revisions: {unexpected:?}"),
                };
                assert!(confirmed >= before);
            }
        });
    });
    store.flush().expect("flush final revision");
    assert_eq!(read_settings(&path), store.snapshot());
}

#[test]
fn readers_observe_complete_files_while_revisions_are_replaced() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("config.toml");
    write_settings(&path, &AppSettings::default());
    let store = SettingsStore::open(&path).expect("open preferences");
    let done = Arc::new(AtomicBool::new(false));
    let reader_path = path.clone();
    let reader_done = done.clone();
    let (started, first_read) = mpsc::channel();
    let reader = thread::spawn(move || {
        read_settings(&reader_path);
        started.send(()).expect("reader ready");
        while !reader_done.load(Ordering::Acquire) {
            read_settings(&reader_path);
        }
    });
    first_read.recv().expect("first complete read");
    for index in 0..40 {
        store
            .apply(SettingsAction::Language(format!("{index}:{}", "x".repeat(4_096))))
            .expect("accept large preference");
    }
    let result = store.flush();
    done.store(true, Ordering::Release);
    reader.join().expect("all reads must be complete TOML");
    result.expect("flush replacements");
}

#[test]
fn explicitly_opened_paths_remain_independent() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let left_path = directory.path().join("left.toml");
    let right_path = directory.path().join("right.toml");
    let left = SettingsStore::open(&left_path).expect("open left");
    let right = SettingsStore::open(&right_path).expect("open right");
    left.apply(SettingsAction::Theme(THEME_DARK.to_string())).expect("change left");
    right.apply(SettingsAction::Language("zh_CN".to_string())).expect("change right");
    left.flush().expect("flush left");
    right.flush().expect("flush right");
    assert_eq!(read_settings(&left_path).general.theme, THEME_DARK);
    assert_eq!(read_settings(&left_path).general.language, "System");
    assert_eq!(read_settings(&right_path).general.theme, THEME_SYSTEM);
    assert_eq!(read_settings(&right_path).general.language, "zh_CN");
}

#[test]
fn dropping_one_clone_keeps_the_writer_available_and_final_drop_flushes() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("config.toml");
    let first = SettingsStore::open(&path).expect("open preferences");
    let last = first.clone();
    first.apply(SettingsAction::Theme(THEME_DARK.to_string())).expect("accept first change");
    drop(first);
    last.apply(SettingsAction::Language("en_US".to_string()))
        .expect("writer survives first handle");
    drop(last);
    let persisted = read_settings(&path);
    assert_eq!(persisted.general.theme, THEME_DARK);
    assert_eq!(persisted.general.language, "en_US");
}

#[test]
fn explicit_shutdown_persists_the_latest_revision_and_closes_every_clone() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("config.toml");
    let store = SettingsStore::open(&path).expect("open preferences");
    let peer = store.clone();
    peer.apply(SettingsAction::Theme(THEME_DARK.to_string())).expect("accept theme");
    let revision = store
        .apply(SettingsAction::Language("zh_CN".to_string()))
        .expect("accept latest language");
    assert_eq!(store.shutdown(Duration::from_secs(2)), Ok(PersistenceStatus::Persisted { revision }));
    assert_eq!(peer.apply(SettingsAction::Language("en_US".to_string())), Err(SettingsError::Closed));
    assert_eq!(read_settings(&path), store.snapshot());
    assert_eq!(peer.shutdown(Duration::ZERO), Ok(PersistenceStatus::Persisted { revision }));
}

#[test]
fn explicit_shutdown_reports_write_failure_and_closes_in_memory_stores_honestly() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("config.toml");
    write_settings(&path, &AppSettings::default());
    let original = fs::read(&path).expect("read last good settings");
    let permissions = fs::metadata(&path).expect("file metadata").permissions();
    let mut read_only = permissions.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&path, read_only).expect("protect file");
    let store = SettingsStore::open(&path).expect("open preferences");
    store.apply(SettingsAction::Theme(THEME_DARK.to_string())).expect("accept desired theme");
    let error = store.shutdown(Duration::from_secs(2)).expect_err("read-only write fails");
    assert!(matches!(error, SettingsError::Persist { .. }));
    assert_eq!(store.shutdown(Duration::ZERO), Err(error.clone()));
    assert_eq!(store.flush(), Err(error));
    drop(store);
    assert_eq!(fs::read(&path).expect("read preserved file"), original);
    fs::set_permissions(path, permissions).expect("restore permissions");

    let memory = SettingsStore::in_memory(AppSettings::default());
    let revision = memory
        .apply(SettingsAction::Language("zh_CN".to_string()))
        .expect("accept desired language");
    assert_eq!(memory.shutdown(Duration::ZERO), Ok(PersistenceStatus::InMemory { revision }));
    assert_eq!(memory.apply(SettingsAction::Theme(THEME_DARK.to_string())), Err(SettingsError::Closed));
    assert_eq!(memory.flush(), Ok(PersistenceStatus::InMemory { revision }));
}

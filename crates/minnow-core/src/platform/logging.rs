use crate::paths::ensure_dir;
use std::env;
use std::io::LineWriter;
use std::panic;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, SystemTime};
use tracing::error;
use tracing_appender::non_blocking::{NonBlocking, NonBlockingBuilder, WorkerGuard};
use tracing_appender::rolling::{Builder as RollingFileAppenderBuilder, Rotation};
use tracing_subscriber::{EnvFilter, Layer, filter::filter_fn, layer::SubscriberExt, util::SubscriberInitExt};

const LOG_FILE_PREFIX: &str = "minnowsnap";
const LOG_FILE_SUFFIX: &str = "log";
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_RETENTION_DAYS: u64 = 7;
const LOG_QUEUE_LINES_LIMIT: usize = 1024;
const LOG_QUEUE_LINES_ENV: &str = "MINNOW_LOG_QUEUE_LINES";
const LOG_WRITER_THREAD_NAME: &str = "minnowsnap-log-writer";

static PANIC_HOOK_ONCE: Once = Once::new();

fn should_suppress_gpui_window_not_found(metadata: &tracing::Metadata<'_>) -> bool {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = metadata;
        false
    }

    #[cfg(target_os = "windows")]
    {
        if *metadata.level() != tracing::Level::ERROR || !metadata.target().is_empty() {
            return false;
        }

        let Some(file) = metadata.file() else {
            return false;
        };

        let normalized_file = file.replace('\\', "/");
        if !normalized_file.contains("/gpui-") || !normalized_file.ends_with("/src/window.rs") {
            return false;
        }

        matches!(metadata.line(), Some(line) if (1108..=1112).contains(&line) || (1162..=1166).contains(&line))
    }
}

pub fn init_logger() -> Option<WorkerGuard> {
    let log_dir = prepare_log_dir()?;
    let (non_blocking, guard) = build_file_writer(&log_dir)?;
    let env_filter = || EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_LEVEL));

    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_filter(env_filter())
        .with_filter(filter_fn(|metadata| !should_suppress_gpui_window_not_found(metadata)));
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking)
        .with_filter(env_filter())
        .with_filter(filter_fn(|metadata| !should_suppress_gpui_window_not_found(metadata)));

    if let Err(e) = tracing_subscriber::registry().with(console_layer).with(file_layer).try_init() {
        eprintln!("Failed to initialize tracing subscriber: {}", e);
        return None;
    }

    let _ = tracing_log::LogTracer::init();
    install_panic_hook();
    log_portable_paths();
    Some(guard)
}

pub fn log_dir() -> PathBuf {
    minnow_paths::app_paths().logs_dir().to_path_buf()
}

fn prepare_log_dir() -> Option<PathBuf> {
    let log_dir = log_dir();
    if let Err(e) = ensure_dir(&log_dir) {
        eprintln!("Failed to create log directory {}: {}", log_dir.display(), e);
        return None;
    }

    cleanup_expired_logs(&log_dir, Duration::from_secs(DEFAULT_RETENTION_DAYS * 24 * 3600));
    Some(log_dir)
}

fn build_file_writer(log_dir: &Path) -> Option<(NonBlocking, WorkerGuard)> {
    let file_appender = RollingFileAppenderBuilder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX)
        .filename_suffix(LOG_FILE_SUFFIX)
        .build(log_dir)
        .map_err(|e| {
            eprintln!("Failed to initialize rolling log file appender in {}: {}", log_dir.display(), e);
        })
        .ok()?;

    let line_writer = LineWriter::new(file_appender);
    Some(
        NonBlockingBuilder::default()
            .buffered_lines_limit(resolve_log_queue_lines_limit())
            .lossy(false)
            .thread_name(LOG_WRITER_THREAD_NAME)
            .finish(line_writer),
    )
}

fn resolve_log_queue_lines_limit() -> usize {
    env::var(LOG_QUEUE_LINES_ENV)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(LOG_QUEUE_LINES_LIMIT)
}

fn cleanup_expired_logs(log_dir: &Path, retention: Duration) {
    let Ok(entries) = std::fs::read_dir(log_dir) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        let Ok(elapsed) = now.duration_since(modified) else {
            continue;
        };
        if elapsed > retention {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn install_panic_hook() {
    PANIC_HOOK_ONCE.call_once(|| {
        let next = panic::take_hook();
        panic::set_hook(Box::new(move |panic_info| {
            let payload = panic_info.payload();
            let msg = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
                .unwrap_or("Box<Any>");

            let location = panic_info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_else(|| "unknown".to_string());

            error!(target: "panic", "Panic at {}: {}", location, msg);
            next(panic_info);
        }));
    });
}

#[cfg(feature = "portable")]
fn log_portable_paths() {
    let paths = minnow_paths::app_paths();
    tracing::info!(
        data_dir = %paths.data_dir().display(),
        config_file = %paths.config_file().display(),
        logs_dir = %paths.logs_dir().display(),
        temp_dir = %paths.temp_dir().display(),
        ocr_models_dir = %paths.ocr_models_dir().display(),
        "Portable storage enabled"
    );
}

#[cfg(not(feature = "portable"))]
fn log_portable_paths() {}

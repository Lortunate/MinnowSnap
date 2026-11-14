use std::env;
use std::io::LineWriter;
use std::panic;
use std::path::Path;
use std::sync::Once;
use std::time::{Duration, SystemTime};
use tracing::error;
use tracing_appender::non_blocking::{NonBlocking, NonBlockingBuilder, WorkerGuard};
use tracing_appender::rolling::{Builder as RollingFileAppenderBuilder, Rotation};
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

const LOG_FILE_PREFIX: &str = "minnowsnap";
const LOG_FILE_SUFFIX: &str = "log";
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_RETENTION_DAYS: u64 = 7;
const LOG_QUEUE_LINES_LIMIT: usize = 1024;
const LOG_QUEUE_LINES_ENV: &str = "MINNOW_LOG_QUEUE_LINES";
const LOG_WRITER_THREAD_NAME: &str = "minnowsnap-log-writer";

static PANIC_HOOK_ONCE: Once = Once::new();

pub(crate) fn init_logger(log_dir: &Path) -> Option<WorkerGuard> {
    if let Err(error) = std::fs::create_dir_all(log_dir) {
        eprintln!("Failed to create log directory {}: {error}", log_dir.display());
        return None;
    }
    cleanup_expired_logs(log_dir, Duration::from_secs(DEFAULT_RETENTION_DAYS * 24 * 3600));
    let (non_blocking, guard) = build_file_writer(log_dir)?;
    let env_filter = || EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_LEVEL));

    let console_layer = tracing_subscriber::fmt::layer().with_writer(std::io::stdout).with_filter(env_filter());
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking)
        .with_filter(env_filter());

    if let Err(e) = tracing_subscriber::registry().with(console_layer).with(file_layer).try_init() {
        eprintln!("Failed to initialize tracing subscriber: {}", e);
        return None;
    }

    install_panic_hook();
    Some(guard)
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
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("minnowsnap.") || !name.ends_with(".log") {
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

use directories::ProjectDirs;
use std::env;
use std::panic;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, SystemTime};
use tracing::error;
use tracing_appender::non_blocking::{NonBlockingBuilder, WorkerGuard};
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

const LOG_DIR_NAME: &str = "logs";
const LOG_FILE_PREFIX: &str = "minnowsnap.log";
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_RETENTION_DAYS: u64 = 7;
const LOG_BUFFERED_LINES_LIMIT: usize = 8_192;
const LOG_BUFFERED_LINES_ENV: &str = "MINNOW_LOG_BUFFERED_LINES";

static PANIC_HOOK_ONCE: Once = Once::new();

pub fn init_logger(app_name: &str) -> Option<WorkerGuard> {
    let log_dir = resolve_log_dir(app_name);
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Failed to create log directory {}: {}", log_dir.display(), e);
        return None;
    }

    let retention = Duration::from_secs(DEFAULT_RETENTION_DAYS * 24 * 3600);
    cleanup_expired_logs(&log_dir, retention);

    let file_appender = tracing_appender::rolling::daily(&log_dir, LOG_FILE_PREFIX);
    let (non_blocking, guard) = NonBlockingBuilder::default()
        .buffered_lines_limit(resolve_log_buffered_lines_limit())
        .finish(file_appender);
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

    let _ = tracing_log::LogTracer::init();
    install_panic_hook();
    Some(guard)
}

fn resolve_log_buffered_lines_limit() -> usize {
    env::var(LOG_BUFFERED_LINES_ENV)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(LOG_BUFFERED_LINES_LIMIT)
}

fn resolve_log_dir(app_name: &str) -> PathBuf {
    ProjectDirs::from("com", "lortunate", app_name)
        .map(|d| d.data_local_dir().join(LOG_DIR_NAME))
        .unwrap_or_else(|| env::current_dir().unwrap_or_default().join(LOG_DIR_NAME))
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

use directories::ProjectDirs;
use std::env;
use std::io::LineWriter;
use std::panic;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, SystemTime};
use tracing::{debug, error, info, trace, warn};
use tracing_appender::non_blocking::{NonBlocking, NonBlockingBuilder, WorkerGuard};
use tracing_appender::rolling::{Builder as RollingFileAppenderBuilder, Rotation};
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

const LOG_DIR_NAME: &str = "logs";
const LOG_FILE_PREFIX: &str = "minnowsnap";
const LOG_FILE_SUFFIX: &str = "log";
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_RETENTION_DAYS: u64 = 7;
const LOG_QUEUE_LINES_LIMIT: usize = 1024;
const LOG_QUEUE_LINES_ENV: &str = "MINNOW_LOG_QUEUE_LINES";
const LOG_WRITER_THREAD_NAME: &str = "minnowsnap-log-writer";
const QT_TARGET: &str = "qt";

static PANIC_HOOK_ONCE: Once = Once::new();

struct QtLogMessage<'a> {
    level: i32,
    category: &'a str,
    message: &'a str,
    file: Option<&'a str>,
    line: Option<i32>,
}

impl<'a> QtLogMessage<'a> {
    fn new(level: i32, category: &'a str, message: &'a str, file: &'a str, line: i32) -> Self {
        Self {
            level,
            category: if category.is_empty() { QT_TARGET } else { category },
            message,
            file: (!file.is_empty()).then_some(file),
            line: (line > 0).then_some(line),
        }
    }

    fn log(self) {
        match (self.file, self.line) {
            (Some(file), Some(line)) => match self.level {
                0 => debug!(target: QT_TARGET, qt_category = self.category, qt_file = file, qt_line = line, "{}", self.message),
                1 => warn!(target: QT_TARGET, qt_category = self.category, qt_file = file, qt_line = line, "{}", self.message),
                2 => error!(target: QT_TARGET, qt_category = self.category, qt_file = file, qt_line = line, "{}", self.message),
                3 => info!(target: QT_TARGET, qt_category = self.category, qt_file = file, qt_line = line, "{}", self.message),
                _ => trace!(target: QT_TARGET, qt_category = self.category, qt_file = file, qt_line = line, "{}", self.message),
            },
            (Some(file), None) => match self.level {
                0 => debug!(target: QT_TARGET, qt_category = self.category, qt_file = file, "{}", self.message),
                1 => warn!(target: QT_TARGET, qt_category = self.category, qt_file = file, "{}", self.message),
                2 => error!(target: QT_TARGET, qt_category = self.category, qt_file = file, "{}", self.message),
                3 => info!(target: QT_TARGET, qt_category = self.category, qt_file = file, "{}", self.message),
                _ => trace!(target: QT_TARGET, qt_category = self.category, qt_file = file, "{}", self.message),
            },
            (None, _) => match self.level {
                0 => debug!(target: QT_TARGET, qt_category = self.category, "{}", self.message),
                1 => warn!(target: QT_TARGET, qt_category = self.category, "{}", self.message),
                2 => error!(target: QT_TARGET, qt_category = self.category, "{}", self.message),
                3 => info!(target: QT_TARGET, qt_category = self.category, "{}", self.message),
                _ => trace!(target: QT_TARGET, qt_category = self.category, "{}", self.message),
            },
        }
    }
}

pub fn init_logger(app_name: &str) -> Option<WorkerGuard> {
    let Some(log_dir) = prepare_log_dir(app_name) else {
        return None;
    };
    let Some((non_blocking, guard)) = build_file_writer(&log_dir) else {
        return None;
    };
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

pub fn log_dir(app_name: &str) -> PathBuf {
    resolve_log_dir(app_name)
}

pub fn log_qt_message(level: i32, category: &str, message: &str, file: &str, line: i32) {
    QtLogMessage::new(level, category, message, file, line).log();
}

fn resolve_log_dir(app_name: &str) -> PathBuf {
    ProjectDirs::from("com", "lortunate", app_name)
        .map(|d| d.data_local_dir().join(LOG_DIR_NAME))
        .unwrap_or_else(|| env::current_dir().unwrap_or_default().join(LOG_DIR_NAME))
}

fn prepare_log_dir(app_name: &str) -> Option<PathBuf> {
    let log_dir = resolve_log_dir(app_name);
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
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

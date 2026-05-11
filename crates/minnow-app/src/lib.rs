use std::sync::LazyLock;
use tokio::runtime::{Builder, Runtime};

const MIN_WORKER_THREADS: usize = 2;
const MAX_WORKER_THREADS: usize = 8;
const MAX_BLOCKING_THREADS: usize = 32;

fn build_runtime() -> Runtime {
    let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let worker_threads = cpus.clamp(MIN_WORKER_THREADS, MAX_WORKER_THREADS);

    Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .max_blocking_threads(MAX_BLOCKING_THREADS)
        .enable_all()
        .thread_name("minnow-app-rt")
        .build()
        .expect("Failed to create Tokio runtime")
}

pub static RUNTIME: LazyLock<Runtime> = LazyLock::new(build_runtime);

rust_i18n::i18n!("../minnow-assets/locales", fallback = "en");

pub mod app;
pub mod platform;
pub mod services;
pub mod ui;

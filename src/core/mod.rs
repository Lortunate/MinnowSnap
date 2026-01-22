use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

pub static RUNTIME: Lazy<Runtime> = Lazy::new(|| Runtime::new().expect("Failed to create Tokio runtime"));

pub mod app;
pub mod capture;
pub mod geometry;
pub mod hotkey;
pub mod io;
pub mod settings;
pub mod window;

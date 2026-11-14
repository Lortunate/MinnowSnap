//! Native desktop adapters. GPUI, application settings and executor ownership
//! stay with the host; every side effect here has an explicit input and result.

pub mod autostart;
pub mod capture;
pub mod clipboard;
pub mod export;
pub mod fonts;
pub mod hotkeys;
pub mod notifications;
pub mod paths;
pub mod sound;
pub mod window;

pub use capture::NativeCapture;
pub use export::DesktopOutput;
pub use notifications::DesktopNotifications;
pub use paths::{AppPaths, DataLayout};

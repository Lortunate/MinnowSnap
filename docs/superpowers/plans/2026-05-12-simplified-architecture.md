# Simplified Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collapse MinnowSnap's active runtime into a single primary crate with clear internal module boundaries for app bootstrap, platform integration, services, and UI.

**Architecture:** First introduce the target namespaces inside `crates/minnow-app` and keep the app compiling. Then merge support crates, OCR, core services, and UI in stages while preserving behavior. Only after the new paths are stable should the obsolete workspace crates be removed.

**Tech Stack:** Rust 2024, Cargo workspace, GPUI, Tokio, tracing, image, ort, config, rust-i18n

---

### Task 1: Establish the New Root Module Layout

**Files:**
- Create: `crates/minnow-app/src/lib.rs`
- Create: `crates/minnow-app/src/app/mod.rs`
- Create: `crates/minnow-app/src/app/commands.rs`
- Create: `crates/minnow-app/src/app/bootstrap.rs`
- Create: `crates/minnow-app/src/app/composition.rs`
- Create: `crates/minnow-app/src/app/runtime.rs`
- Create: `crates/minnow-app/src/platform/mod.rs`
- Create: `crates/minnow-app/src/services/mod.rs`
- Create: `crates/minnow-app/src/ui/mod.rs`
- Create: `crates/minnow-app/tests/module_layout_smoke.rs`
- Modify: `crates/minnow-app/src/main.rs`

- [ ] **Step 1: Write the failing smoke test**

```rust
// crates/minnow-app/tests/module_layout_smoke.rs
use minnow_app::{app, platform, services, ui};

#[test]
fn top_level_modules_are_exported() {
    let _ = std::any::type_name::<fn()>();
    let _ = std::mem::size_of_val(&app::parse_command_from);
    let _ = std::any::type_name_of_val(&platform::module_tag);
    let _ = std::any::type_name_of_val(&services::module_tag);
    let _ = std::any::type_name_of_val(&ui::module_tag);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p minnow-app --test module_layout_smoke`
Expected: FAIL because `minnow_app` library target and exported modules do not exist yet.

- [ ] **Step 3: Add a library root and module stubs**

```rust
// crates/minnow-app/src/lib.rs
pub mod app;
pub mod platform;
pub mod services;
pub mod ui;
```

```rust
// crates/minnow-app/src/app/mod.rs
pub mod bootstrap;
pub mod composition;
mod commands;
pub mod runtime;

pub use commands::{Command, parse_command, parse_command_from, run_command};
```

```rust
// crates/minnow-app/src/app/commands.rs
use std::process::ExitCode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Run,
    Shutdown,
}

pub fn parse_command_from(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    match args.next().as_deref() {
        None | Some("run") => Ok(Command::Run),
        Some("shutdown") => Ok(Command::Shutdown),
        Some(other) => Err(format!("Unknown command '{other}'. Supported commands: run, shutdown")),
    }
}

pub fn parse_command() -> Result<Command, String> {
    parse_command_from(std::env::args().skip(1))
}

pub fn run_command(command: Command) -> ExitCode {
    match command {
        Command::Run => {
            crate::app::runtime::run();
            ExitCode::SUCCESS
        }
        Command::Shutdown => ExitCode::from(crate::app::runtime::shutdown_running_instance()),
    }
}
```

```rust
// crates/minnow-app/src/app/runtime.rs
#[cfg(target_os = "macos")]
use crate::app::bootstrap::hide_dock_icon;
use crate::app::bootstrap::{ensure_single_instance, get_instance_id, init_logger, set_auto_start};
use crate::app::composition::run_application;
use minnow_core::shutdown;
use tracing::info;

pub fn run() {
    let _guard = init_logger();
    info!("Starting MinnowSnap...");

    if !ensure_single_instance(&get_instance_id()) {
        info!("Another instance is running, exiting.");
        return;
    }

    shutdown::init_control_plane();
    #[cfg(target_os = "windows")]
    {
        shutdown::install_ctrl_c_handler();
        shutdown::start_control_pipe_server();
    }

    #[cfg(target_os = "macos")]
    run_application(set_auto_start, hide_dock_icon);
    #[cfg(not(target_os = "macos"))]
    run_application(set_auto_start, noop_hide_dock_icon);

    shutdown::clear_control_plane();
}

pub fn shutdown_running_instance() -> u8 {
    #[cfg(target_os = "windows")]
    {
        match shutdown::shutdown_running_instance() {
            Ok(()) => 0,
            Err(shutdown::ShutdownClientError::NotRunning) => 2,
            Err(err) => {
                eprintln!("Failed to request graceful shutdown: {err}");
                3
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("Shutdown command is only supported on Windows.");
        3
    }
}

#[cfg(not(target_os = "macos"))]
fn noop_hide_dock_icon() {}
```

```rust
// crates/minnow-app/src/app/bootstrap.rs
// Move the current `crates/minnow-app/src/bootstrap.rs` contents here with import paths unchanged.
```

```rust
// crates/minnow-app/src/app/composition.rs
// Move the current `crates/minnow-app/src/composition.rs` contents here with import paths unchanged.
```

```rust
// crates/minnow-app/src/platform/mod.rs
pub const module_tag: &str = "platform";
```

```rust
// crates/minnow-app/src/services/mod.rs
pub const module_tag: &str = "services";
```

```rust
// crates/minnow-app/src/ui/mod.rs
pub const module_tag: &str = "ui";
```

- [ ] **Step 4: Move CLI parsing support out of `main.rs`**

```rust
// crates/minnow-app/src/main.rs
use minnow_app::app::{parse_command, run_command};
use std::process::ExitCode;

fn main() -> ExitCode {
    let command = match parse_command() {
        Ok(command) => command,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };

    run_command(command)
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p minnow-app --test module_layout_smoke`
Expected: PASS

- [ ] **Step 6: Run a crate-wide compile check**

Run: `cargo check -p minnow-app`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/minnow-app/src/lib.rs crates/minnow-app/src/app crates/minnow-app/src/platform crates/minnow-app/src/services crates/minnow-app/src/ui crates/minnow-app/src/main.rs crates/minnow-app/tests/module_layout_smoke.rs
git commit -m "refactor: add minnow-app module layout"
```

### Task 2: Merge `minnow-assets` and `minnow-paths` into `services`

**Files:**
- Create: `crates/minnow-app/src/services/assets/mod.rs`
- Create: `crates/minnow-app/src/services/assets/asset_bytes.rs`
- Create: `crates/minnow-app/src/services/assets/asset_paths.rs`
- Create: `crates/minnow-app/src/services/paths.rs`
- Create: `crates/minnow-app/resources/*`
- Create: `crates/minnow-app/assets_icons/*`
- Modify: `crates/minnow-app/build.rs`
- Modify: `crates/minnow-app/src/app/composition.rs`
- Modify: `crates/minnow-app/Cargo.toml`
- Test: `crates/minnow-app/tests/module_layout_smoke.rs`

- [ ] **Step 1: Write a failing assets/paths smoke test**

```rust
// append to crates/minnow-app/tests/module_layout_smoke.rs
use minnow_app::services::{assets::AppAssets, paths::app_paths};

#[test]
fn services_expose_assets_and_paths() {
    let _assets = AppAssets;
    let _paths = app_paths();
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p minnow-app --test module_layout_smoke`
Expected: FAIL because `services::assets` and `services::paths` do not exist yet.

- [ ] **Step 3: Copy support crate code into `services`**

```rust
// crates/minnow-app/src/services/mod.rs
pub mod assets;
pub mod paths;
```

```rust
// crates/minnow-app/src/services/assets/mod.rs
use std::{borrow::Cow, collections::HashSet};

use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

pub mod asset_bytes;
pub mod asset_paths;

#[derive(RustEmbed)]
#[folder = "."]
#[include = "resources/icons/**/*.svg"]
#[include = "resources/logo.png"]
struct ProjectAssets;

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        if let Some(file) = ProjectAssets::get(path) {
            return Ok(Some(file.data));
        }

        gpui_component_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut assets = gpui_component_assets::Assets.list(path)?;
        assets.extend(ProjectAssets::iter().filter(|asset| asset.starts_with(path)).map(SharedString::from));

        let mut seen = HashSet::new();
        assets.retain(|asset| seen.insert(asset.clone()));
        Ok(assets)
    }
}
```

```rust
// crates/minnow-app/src/services/paths.rs
#[cfg(not(feature = "portable"))]
use directories::ProjectDirs;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(not(feature = "portable"))]
const APP_QUALIFIER: &str = "com";
#[cfg(not(feature = "portable"))]
const APP_ORGANIZATION: &str = "lortunate";
#[cfg(not(feature = "portable"))]
const APP_NAME: &str = "MinnowSnap";
const DATA_DIR_NAME: &str = "data";
const CONFIG_FILE_NAME: &str = "config.toml";
const LOGS_DIR_NAME: &str = "logs";
#[cfg(feature = "portable")]
const TEMP_DIR_NAME: &str = "temp";
const OCR_MODELS_DIR_NAME: &str = "ocr_models";

pub struct AppPaths {
    data_dir: PathBuf,
    config_file: PathBuf,
    logs_dir: PathBuf,
    temp_dir: PathBuf,
    ocr_models_dir: PathBuf,
}

static APP_PATHS: OnceLock<AppPaths> = OnceLock::new();

#[cfg(feature = "portable")]
fn executable_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| env::current_dir().unwrap_or_default())
}

pub fn app_paths() -> &'static AppPaths {
    APP_PATHS.get_or_init(resolve_app_paths)
}

fn resolve_app_paths() -> AppPaths {
    #[cfg(feature = "portable")]
    {
        let data_dir = executable_dir().join(DATA_DIR_NAME);
        AppPaths {
            config_file: data_dir.join(CONFIG_FILE_NAME),
            logs_dir: data_dir.join(LOGS_DIR_NAME),
            temp_dir: data_dir.join(TEMP_DIR_NAME),
            ocr_models_dir: data_dir.join(OCR_MODELS_DIR_NAME),
            data_dir,
        }
    }

    #[cfg(not(feature = "portable"))]
    {
        let current_dir = env::current_dir().unwrap_or_default();
        let project_dirs = ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_NAME);
        let config_file = project_dirs
            .as_ref()
            .map(|dirs| dirs.config_dir().join(CONFIG_FILE_NAME))
            .unwrap_or_else(|| current_dir.join(CONFIG_FILE_NAME));
        let data_dir = project_dirs
            .as_ref()
            .map(|dirs| dirs.data_local_dir().to_path_buf())
            .unwrap_or_else(|| current_dir.join(DATA_DIR_NAME));

        AppPaths {
            logs_dir: data_dir.join(LOGS_DIR_NAME),
            ocr_models_dir: data_dir.join(OCR_MODELS_DIR_NAME),
            data_dir,
            config_file,
            temp_dir: env::temp_dir(),
        }
    }
}

impl AppPaths {
    pub fn data_dir(&self) -> &Path { &self.data_dir }
    pub fn config_file(&self) -> &Path { &self.config_file }
    pub fn logs_dir(&self) -> &Path { &self.logs_dir }
    pub fn temp_dir(&self) -> &Path { &self.temp_dir }
    pub fn temp_file(&self, file_name: &str) -> PathBuf { self.temp_dir.join(file_name) }
    pub fn ocr_models_dir(&self) -> &Path { &self.ocr_models_dir }
}
```

- [ ] **Step 4: Repoint build and runtime imports**

```rust
// crates/minnow-app/src/app/composition.rs
use crate::services::assets::AppAssets;
```

```rust
// crates/minnow-app/build.rs
println!("cargo:rerun-if-changed=resources/logo.png");
println!("cargo:rerun-if-changed=resources");
```

- [ ] **Step 5: Remove obsolete direct dependencies from `minnow-app`**

```toml
# crates/minnow-app/Cargo.toml
[dependencies]
minnow-core = { path = "../minnow-core" }
minnow-ui = { path = "../minnow-ui" }
# remove `minnow-assets`
# remove `minnow-paths`

[build-dependencies]
# keep existing third-party build dependencies unchanged
```

- [ ] **Step 6: Run tests and compile checks**

Run: `cargo test -p minnow-app --test module_layout_smoke`
Expected: PASS

Run: `cargo check -p minnow-app`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/minnow-app/src/services/assets crates/minnow-app/src/services/paths.rs crates/minnow-app/src/services/mod.rs crates/minnow-app/src/app/composition.rs crates/minnow-app/build.rs crates/minnow-app/Cargo.toml crates/minnow-app/tests/module_layout_smoke.rs
git commit -m "refactor: merge support crates into services"
```

### Task 3: Move OCR and Shared Domain Code into `services`

**Files:**
- Create: `crates/minnow-app/src/services/app_meta.rs`
- Create: `crates/minnow-app/src/services/ocr/mod.rs`
- Create: `crates/minnow-app/src/services/capture/mod.rs`
- Modify: `crates/minnow-app/src/services/mod.rs`
- Modify: files copied from `crates/minnow-ocr/src/*`
- Modify: files copied from `crates/minnow-core/src/{geometry.rs,i18n.rs,settings.rs,capture/*,ocr/service.rs}`
- Modify: `crates/minnow-app/Cargo.toml`
- Test: `crates/minnow-app/tests/module_layout_smoke.rs`

- [ ] **Step 1: Write a failing service access test**

```rust
// append to crates/minnow-app/tests/module_layout_smoke.rs
use minnow_app::services::{capture, i18n, settings};

#[test]
fn services_expose_capture_settings_and_i18n() {
    let _ = capture::active_monitor_scale as fn() -> f32;
    let _ = std::any::type_name_of_val(&i18n::SYSTEM_LOCALE);
    let _ = std::any::type_name::<settings::AppSettings>();
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p minnow-app --test module_layout_smoke`
Expected: FAIL because the migrated modules are not exported from `services` yet.

- [ ] **Step 3: Copy OCR and domain modules into `services`**

```rust
// crates/minnow-app/src/services/mod.rs
pub mod app_meta;
pub mod capture;
pub mod geometry;
pub mod i18n;
pub mod ocr;
pub mod settings;
```

```rust
// crates/minnow-app/src/services/ocr/mod.rs
pub mod config;
pub mod detector;
pub mod engine;
pub mod model_manager;
pub mod preprocess;
pub mod recognizer;
pub mod service;
pub mod visualization;
```

```rust
// crates/minnow-app/src/services/capture/mod.rs
pub mod action;
pub mod long_capture;
pub mod service;
pub mod source;
pub mod stitcher;
```

- [ ] **Step 4: Update settings and OCR imports to local modules**

```rust
// example import rewrites
use crate::services::paths;
use crate::services::ocr;
use crate::services::geometry::Rect;
```

- [ ] **Step 5: Update `Cargo.toml` to depend on direct third-party crates instead of `minnow-core` and `minnow-ocr`**

```toml
# crates/minnow-app/Cargo.toml
[dependencies]
config = { workspace = true }
image = { workspace = true, features = ["rayon", "png", "jpeg"] }
ort = { workspace = true }
rust-i18n = { workspace = true }
anyhow = { workspace = true }
ab_glyph = { workspace = true }
arboard = { workspace = true }
ctrlc = { workspace = true }
directories = { workspace = true }
dirs = { workspace = true }
font-kit = { workspace = true }
futures-util = { workspace = true }
global-hotkey = { workspace = true }
imageproc = { workspace = true }
ndarray = { workspace = true }
notify-rust = { workspace = true }
num-traits = { workspace = true }
once_cell = { workspace = true }
oxipng = { workspace = true }
raw-window-handle = { workspace = true }
rayon = { workspace = true }
reqwest = { workspace = true }
rodio = { workspace = true }
rqrr = { workspace = true }
rust-embed = { workspace = true }
serde = { workspace = true }
sys-locale = { workspace = true }
tauri-winrt-notification = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
tokio-util = { workspace = true }
tracing = { workspace = true }
tracing-appender = { workspace = true }
tracing-log = { workspace = true }
tracing-subscriber = { workspace = true }
tray-icon = { workspace = true }
windows = { workspace = true }
winreg = { workspace = true }
xcap = { workspace = true }
# remove `minnow-core` and `minnow-ocr` once compilation passes
```

- [ ] **Step 6: Run tests and a focused crate suite**

Run: `cargo test -p minnow-app --test module_layout_smoke`
Expected: PASS

Run: `cargo test -p minnow-app`
Expected: PASS, including existing command parsing tests and any migrated service unit tests.

- [ ] **Step 7: Commit**

```bash
git add crates/minnow-app/src/services crates/minnow-app/Cargo.toml crates/minnow-app/tests/module_layout_smoke.rs
git commit -m "refactor: move ocr and domain services into minnow-app"
```

### Task 4: Move Platform and UI Code into `minnow-app`

**Files:**
- Create: `crates/minnow-app/src/platform/*`
- Create: `crates/minnow-app/src/ui/*`
- Modify: files copied from `crates/minnow-ui/src/{features/*,support/*,key_unicode.rs,shell/*}`
- Modify: `crates/minnow-app/src/app/composition.rs`
- Modify: `crates/minnow-app/Cargo.toml`
- Test: `crates/minnow-app/tests/module_layout_smoke.rs`

- [ ] **Step 1: Write a failing UI/platform smoke test**

```rust
// append to crates/minnow-app/tests/module_layout_smoke.rs
use minnow_app::{platform, ui};

#[test]
fn ui_and_platform_modules_expose_runtime_entry_points() {
    let _ = platform::hotkey::install_hotkey_service;
    let _ = ui::features::preferences::open_window;
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p minnow-app --test module_layout_smoke`
Expected: FAIL because the UI and platform modules are not wired into `minnow-app` yet.

- [ ] **Step 3: Copy UI features and support modules into `ui`**

```rust
// crates/minnow-app/src/ui/mod.rs
pub mod features;
pub mod key_unicode;
pub mod support;
```

```rust
// crates/minnow-app/src/ui/features/mod.rs
pub mod long_capture;
pub mod overlay;
pub mod pin;
pub mod preferences;
```

- [ ] **Step 4: Move shell-only helpers into `platform`**

```rust
// crates/minnow-app/src/platform/mod.rs
pub mod async_ui;
pub mod background_host;
pub mod hotkey;
pub mod native_window;
pub mod tray;
pub mod window_drag;
pub mod windowing;
```

```rust
// import rewrite examples
use crate::platform::windowing::{PopupWindowSpec, configure_window, popup_window_options};
use crate::services::app_meta::APP_ID;
use crate::ui::features::{overlay, pin, preferences};
```

- [ ] **Step 5: Repoint the composition root**

```rust
// crates/minnow-app/src/app/composition.rs
use crate::platform::hotkey::HotkeyActionSink;
use crate::platform::tray::TrayActions;
use crate::ui::features::{overlay, pin, preferences};
use crate::ui::support::{appearance, locale};
```

- [ ] **Step 6: Run tests and compile checks**

Run: `cargo test -p minnow-app --test module_layout_smoke`
Expected: PASS

Run: `cargo check -p minnow-app`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/minnow-app/src/platform crates/minnow-app/src/ui crates/minnow-app/src/app/composition.rs crates/minnow-app/Cargo.toml crates/minnow-app/tests/module_layout_smoke.rs
git commit -m "refactor: move platform and ui into minnow-app"
```

### Task 5: Remove Obsolete Workspace Crates and Finalize Verification

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/minnow-app/Cargo.toml`
- Delete: `crates/minnow-assets`
- Delete: `crates/minnow-paths`
- Delete: `crates/minnow-core`
- Delete: `crates/minnow-ui`
- Delete: `crates/minnow-ocr`

- [ ] **Step 1: Remove obsolete workspace members**

```toml
# Cargo.toml
[workspace]
members = [
    "crates/minnow-app",
]
default-members = ["crates/minnow-app"]
```

- [ ] **Step 2: Remove obsolete path dependencies**

```toml
# crates/minnow-app/Cargo.toml
[dependencies]
# keep only third-party dependencies and local features
# ensure these path dependencies are gone:
# minnow-assets
# minnow-core
# minnow-ocr
# minnow-paths
# minnow-ui
```

- [ ] **Step 3: Run verification gates**

Run: `cargo test -p minnow-app`
Expected: PASS

Run: `cargo check --workspace`
Expected: PASS

Run: `cargo build -p minnow-app --release`
Expected: PASS

- [ ] **Step 4: Run an app startup smoke check**

Run: `cargo run -p minnow-app -- run`
Expected: App starts without unresolved import, asset, or settings-path errors.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/minnow-app/Cargo.toml crates/minnow-app/src crates/minnow-app/tests
git rm -r crates/minnow-assets crates/minnow-paths crates/minnow-core crates/minnow-ui crates/minnow-ocr
git commit -m "refactor: collapse runtime crates into minnow-app"
```

### Self-Review Checklist

- [ ] Every spec section maps to at least one task above.
- [ ] No task depends on `legacy/qt`.
- [ ] `main.rs` stays thin and `lib.rs` becomes the module root.
- [ ] The plan removes `core` as a catch-all boundary rather than renaming it.
- [ ] Verification includes compile, test, release build, and startup smoke checks.

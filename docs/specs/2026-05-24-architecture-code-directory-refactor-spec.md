# Architecture And UI Consolidation Specification

Date: 2026-08-09
Status: Active

## Goal

Keep MinnowSnap as one Rust 2024 workspace crate with GPUI as its only desktop
UI toolkit. The refactor removes retired UI sources and tightens the existing
module seams without changing capture, annotation, OCR, pin, preferences, tray,
or shortcut behaviour.

## Current architecture

- `src/main.rs` is a thin process entry point.
- `app` owns command parsing, startup order, lifecycle, and top-level wiring.
- `platform` owns operating-system and native-window adapters.
- `services` owns capture/image processing, OCR, settings, assets, paths, and
  other domain data transformations.
- `ui/features` owns GPUI windows, feature state, input, and rendering.
- `ui/support` owns reusable UI-only layout and rendering helpers.

There is one workspace member (`crates/minnow-app`) and one public crate facade
(`minnow_app::app`).

## Target seams and ownership

| Module | Owns | Must not own |
| --- | --- | --- |
| `app` | Process entry, startup, dependency registration, workflow wiring | Feature internals or image algorithms |
| `platform` | Native windows, clipboard, notifications, tray, hotkeys, storage, shutdown | Product policy and feature state |
| `services` | Domain decisions, image/OCR processing, settings persistence | GPUI or platform side effects |
| `ui::features` | GPUI views, local state, input and feature-facing requests | Direct private platform access or domain I/O |
| `ui::support` | Shared UI layout/rendering seams | OS calls, persistence, or duplicate state |

The capture action seam is deliberately deep: a small domain planner resolves
an image or text outcome, while one application workflow executes platform side
effects and one UI support module interprets the result for feature hosts.

## Single sources of truth

- App identity and lock policy: `services::app_meta`.
- Capture image resolution and monitor selection: `services::capture`.
- Capture action side effects: `app::workflows::execute_capture_action` through
  `platform::shell`.
- Settings persistence: `services::settings`.
- OCR model and recognition pipeline: `services::ocr`.
- Shared toolbar/panel placement: `ui::support::panel_layout`.
- Native handle extraction: `platform::native_window`.
- Packaging icons: `resources/logo.png`; generated files are build outputs.
- Toolchain: `rust-toolchain.toml` and the package `rust-version` field.

## Migration and cleanup rules

1. Remove every retired UI source, bridge, resource manifest, and build/editor
   configuration from the repository.
2. Keep all user-visible feature entry points on GPUI; deletion is allowed only
   after the corresponding active feature and call chain are verified.
3. Do not retain compatibility paths, duplicate state, or a second executor.
4. Keep interfaces small and crate-private unless an actual external caller
   needs the symbol. Introduce a trait only where at least two real adapters
   exist.
5. Propagate recoverable errors to the owning workflow and use structured
   tracing for runtime diagnostics.
6. Avoid unrelated renames, broad formatting churn, and dependency upgrades
   that do not reduce complexity.

## Verification

Required source and boundary checks:

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The architecture smoke test also verifies that no retired UI assets or runtime
references remain, that `minnow_app::app` is the only public facade, and that UI
features use the platform shell seam.

If a host platform lacks its graphics SDK, run the same checks for an installed
cross target and record the host-only gate separately; do not claim the blocked
gate passed.

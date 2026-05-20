# Architecture Cleanup Plan

Date: 2026-05-18
Status: Active
Tracking: `bd` issue `minnowsnap-0l0`

## Phase 1: Public API Boundary

Scope:

- Keep `minnow_app::app` as the only public crate-root module.
- Make `platform`, `services`, `ui`, and the shared Tokio runtime crate-private.
- Make `app::bootstrap`, `app::composition`, and `app::runtime` private.
- Update the module layout smoke test so the intended boundary is enforced.

Verification:

- `cargo test -p minnow-app --test module_layout_smoke -- --nocapture`
- `cargo check -p minnow-app`

## Phase 2: UI Feature Decomposition

Scope:

- Split oversized overlay render/state files by stable responsibility, not by temporary fragments.
- Keep cross-feature access through public feature APIs only.
- Remove render/state helper wrappers when a helper has one caller and no domain name.

Primary targets:

- `crates/minnow-app/src/ui/features/overlay/render/toolbar.rs`
- `crates/minnow-app/src/ui/features/overlay/render/layout.rs`
- `crates/minnow-app/src/ui/features/overlay/state/session.rs`
- `crates/minnow-app/src/ui/features/pin/state.rs`
- `crates/minnow-app/src/ui/features/preferences/state/frame.rs`

## Phase 3: Capture And Image Pipeline

Scope:

- Reduce buffer cloning in capture, pin, annotation, and long-capture paths.
- Keep stitching and OCR preprocessing out of UI modules.
- Introduce small data structs only when they replace repeated parameter groups or clarify ownership.

Primary targets:

- `crates/minnow-app/src/services/capture/stitcher.rs`
- `crates/minnow-app/src/services/capture/mod.rs`
- `crates/minnow-app/src/services/capture/long_capture.rs`
- `crates/minnow-app/src/ui/support/render_image.rs`

## Phase 4: Dead Code And Dependency Hygiene

Scope:

- Delete stale modules after call sites move.
- Keep `cargo machete` clean.
- Keep archived legacy Qt code out of active build and tests.

Verification:

- `cargo fmt --check`
- `cargo check -p minnow-app`
- `cargo test -p minnow-app`
- `cargo clippy -p minnow-app -- -W clippy::all`
- `cargo machete`

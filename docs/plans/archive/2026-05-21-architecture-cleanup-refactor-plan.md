# Architecture Cleanup Refactor Plan

Status: Archived
Spec: `docs/specs/archive/2026-05-21-architecture-cleanup-refactor-spec.md`
Tracking: `minnowsnap-0l0`
Archived-By: `docs/plans/2026-05-22-architecture-cleanup-refactor-plan.md`
Supersedes: `docs/plans/archive/2026-05-18-architecture-cleanup-plan.md`

## Phase 1: Public API Boundary

Status: Complete
Related issue: `minnowsnap-9cb`

- Keep `minnow_app::app` as the only public crate-root facade.
- Keep `platform`, `services`, `ui`, and runtime internals crate-private.
- Keep `main.rs` thin: parse command, run command.
- Guard with `crates/minnow-app/tests/module_layout_smoke.rs`.

Verification:

```bash
cargo test -p minnow-app --test module_layout_smoke -- --nocapture
cargo check -p minnow-app
```

## Phase 2: UI Feature Decomposition

Status: Open
Issue: `minnowsnap-pzo`

Target modules:

- `crates/minnow-app/src/ui/features/overlay/render/*`
- `crates/minnow-app/src/ui/features/overlay/state/*`
- `crates/minnow-app/src/ui/features/pin/state.rs`
- `crates/minnow-app/src/ui/features/preferences/state/*`
- `crates/minnow-app/src/ui/features/long_capture/*`

Work:

- Split oversized files only by stable responsibilities: view rendering, input handling, state transitions, diagnostics, and feature effects.
- Remove helper modules or functions that are one-caller pass-throughs without domain meaning.
- Keep cross-feature access through public feature APIs only.
- Update `module_layout_smoke` if a new boundary needs a guard.

Risks:

- Overlay and pin state are interaction-heavy; avoid moving behavior without focused tests.
- Cross-feature import restrictions can break if private render/state modules leak.

Verification:

```bash
cargo fmt --check
cargo check -p minnow-app
cargo test -p minnow-app --test module_layout_smoke -- --nocapture
```

## Phase 3: Capture And Image Pipeline

Status: Open
Issue: `minnowsnap-zwr`

Target modules:

- `crates/minnow-app/src/services/capture/mod.rs`
- `crates/minnow-app/src/services/capture/service.rs`
- `crates/minnow-app/src/services/capture/long_capture.rs`
- `crates/minnow-app/src/services/capture/stitcher.rs`
- `crates/minnow-app/src/services/ocr/*`
- `crates/minnow-app/src/ui/support/render_image.rs`
- Pin and overlay call sites that own image handles.

Work:

- Reduce image buffer clones in capture, annotation, pin, OCR, and long-capture flows.
- Keep stitching and OCR preprocessing in services.
- Replace repeated parameter groups with small data structs only when they clarify ownership.
- Keep cached preview/scroll image ownership in `services::capture`.

Risks:

- Image ownership changes can regress clipboard, save, OCR, pin, and long-capture workflows.
- OCR model download and recognition use async runtime bridging; keep UI-facing state simple.

Verification:

```bash
cargo fmt --check
cargo check -p minnow-app
cargo test -p minnow-app
```

## Phase 4: Dead Code And Dependency Hygiene

Status: In Progress
Issue: `minnowsnap-r15`

Target modules:

- Active warnings from `cargo check -p minnow-app`.
- Stale modules with no `rg` call sites.
- `Cargo.toml` workspace and crate dependencies.
- Active references to `legacy/qt`.

Work:

- Preserve already-valid cleanup of unused OCR visualization, path helpers, and i18n helpers.
- Delete dead capture monitor target selection code if it has no active caller.
- Delete dead notification/window variants and unused geometry helpers.
- Remove unused result fields where no caller reads the value.
- Keep `cargo machete` clean; if the tool is unavailable, record the blocker and use Cargo/`rg` evidence.

Deletion evidence format:

```text
symbol:
call-site check:
facade check:
verification:
decision:
```

Verification:

```bash
cargo fmt --check
cargo check -p minnow-app
cargo test -p minnow-app --test module_layout_smoke -- --nocapture
cargo test -p minnow-app
cargo clippy -p minnow-app -- -W clippy::all
cargo machete
rg -n -F -e "legacy/qt" -e "legacy::" -e "cxx_qt" -e "cxx-qt" -e "qt_" Cargo.toml crates/minnow-app/Cargo.toml crates/minnow-app/build.rs crates/minnow-app/src crates/minnow-app/tests --glob '!target/**'
```

## Session Close

- Update or close the claimed Beads issue with verification evidence.
- Create follow-up Beads issues for remaining architecture work instead of leaving markdown TODOs.
- Stage only files related to the claimed issue.
- Run:

```bash
git pull --rebase
bd dolt push
git push
git status
```

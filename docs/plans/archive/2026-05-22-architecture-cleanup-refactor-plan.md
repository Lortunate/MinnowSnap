# Architecture Cleanup Refactor Plan

Date: 2026-05-22
Issue: `minnowsnap-2vo`
Spec: `docs/specs/2026-05-22-architecture-cleanup-refactor-spec.md`
Status: Active

Supersedes:
- `docs/plans/archive/2026-05-21-architecture-cleanup-refactor-plan.md`
- `docs/plans/archive/2026-05-18-architecture-cleanup-plan.md`

## Baseline

The starting tree is a single workspace crate, `crates/minnow-app`, with active code split across `app`, `platform`, `services`, and `ui`.

Baseline gates before phase 1:

```bash
cargo fmt --check
cargo check -p minnow-app
cargo test -p minnow-app --test module_layout_smoke -- --nocapture
cargo machete
```

Observed baseline: formatting, check, module layout smoke tests, and dependency hygiene all passed.

## Phase 1: UI Platform Boundary

Goal: make `platform::shell` the single UI-facing source for platform side effects and shell window helpers.

Files:
- Create: `crates/minnow-app/src/platform/shell.rs`
- Modify: `crates/minnow-app/src/platform/mod.rs`
- Modify: `crates/minnow-app/src/ui/features/**`
- Modify: `crates/minnow-app/tests/module_layout_smoke.rs`

Implementation:

1. Add an architecture test named `ui_features_use_platform_shell_facade_only`.
2. Verify the test fails against direct `crate::platform::{notify, clipboard, windowing, native_window, window_drag, system, hotkey, storage}` imports.
3. Add `platform::shell` with UI-facing helpers:
   - popup window options and focus configuration
   - always-on-top and click-through helpers
   - notification and text clipboard helpers
   - default save path helper
   - drag, hotkey, and system action facade types used by UI
4. Replace direct platform imports in `ui/features` with `crate::platform::shell`.
5. Run:

```bash
cargo test -p minnow-app --test module_layout_smoke ui_features_use_platform_shell_facade_only -- --nocapture
cargo check -p minnow-app
```

Exit criteria:
- The new architecture test passes.
- `cargo check -p minnow-app` passes.
- No direct private platform references remain under `crates/minnow-app/src/ui/features`.

## Phase 2: Feature Boundary Tightening

Goal: keep cross-feature calls on public feature APIs and remove accidental coupling to private `state` and `render` modules.

Candidate targets:
- `ui/features/overlay`
- `ui/features/pin`
- `ui/features/long_capture`
- `ui/features/preferences`

Work sequence:

1. Use `module_layout_smoke` to identify any cross-feature private imports.
2. Replace private imports with explicit feature facade functions or request/result types.
3. Keep feature-local helpers private unless another feature has a documented public need.
4. Run:

```bash
cargo fmt --check
cargo check -p minnow-app
cargo test -p minnow-app --test module_layout_smoke -- --nocapture
```

## Phase 3: Capture And Image Pipeline Simplification

Goal: keep capture/image ownership in `services::capture` and avoid UI-side image IO duplication.

Candidate targets:
- `src/services/capture/service.rs`
- `src/services/capture/long_capture.rs`
- `src/services/capture/stitcher.rs`
- `src/ui/features/overlay/state/effects.rs`
- `src/ui/features/pin/view/mod.rs`

Work sequence:

1. Search for image saving, clipboard image copy, QR decode, and stitching call sites.
2. Delete stale wrappers after proving no call sites remain.
3. Prefer service functions that accept explicit request/context values over feature-specific hidden globals.
4. Keep long-capture stitching in services unless UI-only layout state is involved.
5. Run:

```bash
cargo fmt --check
cargo check -p minnow-app
cargo test -p minnow-app
```

## Phase 4: Dead Code And Dependency Hygiene

Goal: remove dead code, redundant modules, and dependency drift without speculative churn.

Evidence commands:

```bash
cargo check -p minnow-app
cargo clippy -p minnow-app -- -W clippy::all
cargo machete
cargo test -p minnow-app --test module_layout_smoke -- --nocapture
```

Work sequence:

1. Delete warning-backed unused functions, traits, enum variants, or modules.
2. Remove dependencies only when `cargo machete` or manual call-site evidence agrees.
3. Keep retired UI references out of active Cargo/build/source/test paths.
4. File beads follow-ups for risky deletions instead of leaving markdown TODOs.

## Session Close

Required final gates after code changes:

```bash
cargo fmt --check
cargo check -p minnow-app
cargo test -p minnow-app --test module_layout_smoke -- --nocapture
cargo test -p minnow-app
cargo machete
```

Project close protocol:

```bash
bd update minnowsnap-2vo --notes "<summary and verification>"
bd close minnowsnap-2vo
git pull --rebase
bd dolt push
git push
git status
```

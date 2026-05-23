# Architecture Code Directory Refactor Spec

Date: 2026-05-24
Issue: `minnowsnap-0l0`
Status: Active

Supersedes:
- `docs/specs/archive/2026-05-22-architecture-cleanup-refactor-spec.md`
- `docs/specs/archive/2026-05-21-architecture-cleanup-refactor-spec.md`
- `docs/specs/archive/2026-05-18-architecture-cleanup-spec.md`
- `docs/specs/archive/2026-05-17-conservative-merge-refactor-design.md`

## Goal

Make MinnowSnap's Rust 2024 GPUI application easier to read, change, and verify by tightening ownership boundaries inside the existing `minnow-app` crate, removing duplicate sources of truth, and deleting wrappers or dead code that no longer earn their indirection. Compatibility with old internal module paths is not required.

## Current Stack

- UI: GPUI and `gpui-component`.
- App shell: one workspace crate, `crates/minnow-app`, with binary `MinnowSnap`.
- Platform integration: tray, global hotkeys, native windows, notification, clipboard, storage, shell actions, and shutdown.
- Domain services: capture, long capture, OCR, geometry, settings, i18n, fonts, paths, and assets.
- Legacy Qt/CXX-Qt implementation: archived under `legacy/qt` only.

Context7 documentation for GPUI and `gpui-component` was checked before this spec. The current stack remains valid: initialize `gpui_component::init(cx)` during app startup before component use, and wrap window root views with `Root::new(view, window, cx)`.

## Scope

This refactor keeps the app on GPUI and keeps a single workspace crate unless the implementation plan later proves a crate split removes more complexity than it adds. The first target is to make the current logical boundaries real and testable.

In scope:
- Archive superseded architecture docs and keep one active spec/plan pair.
- Split application composition from user workflows where `app::composition` currently carries behavior.
- Separate capture decisions from platform/UI side effects.
- Consolidate source-of-truth drift in app identity, lockfile policy, generated assets, locale wrappers, layout algorithms, and native-window helpers.
- Remove zero-value wrappers and dead code when call-site evidence and tests support deletion.
- Strengthen architecture smoke tests for the boundaries this refactor changes.

Out of scope:
- Toolkit migration away from GPUI.
- Reintroducing Qt/CXX-Qt runtime paths.
- Keeping compatibility shims for old internal module paths.
- Large feature additions unrelated to readability, maintainability, or source-of-truth consolidation.

## Target Boundaries

| Area | Owns | Must Not Own |
| --- | --- | --- |
| `app` | Command entry, startup order, dependency registration, top-level workflow wiring. | Feature internals, capture algorithms, notification message policy, window-specific result interpretation. |
| `platform` | OS capabilities: native windows, drag, notification, clipboard, tray, hotkeys, storage, shutdown, shell launch. | Capture/OCR/settings business rules or feature state machines. |
| `platform::shell` | UI-facing platform API only when it provides a real architectural boundary. | Pure pass-through wrappers that hide no complexity and create a second API surface. |
| `services` | Domain logic, data transformations, image/OCR/capture processing, settings persistence. | GPUI windows, notifications, clipboard decisions, feature-specific UI flow. |
| `ui::features` | GPUI views, feature-local state, input handling, feature public requests/results. | Direct private platform imports, duplicated cross-feature result interpretation, domain image algorithms. |
| `ui::support` | Shared UI-only helpers such as appearance, locale, image rendering, and reusable panel/window layout math. | Platform shell/window side effects or domain service state. |
| `legacy/qt` | Read-only historical archive. | Active build input, tests, implementation references, or source-of-truth docs. |

## Source Of Truth Decisions

| Domain | Source Of Truth | Required Change |
| --- | --- | --- |
| App identity | `services::app_meta` constants consumed by packaging and runtime code. | Make `APP_ID`, lock id, bundle metadata, notification app id, and path identity agree. |
| Dependency lock | `Cargo.lock` in version control. | Stop ignoring `Cargo.lock` for this app workspace and track it for reproducible desktop builds. |
| Icons | `resources/logo.png` as the source asset; generated icon outputs as build artifacts unless a packaging tool requires checked-in outputs. | Remove or justify tracked generated icons; make `build.rs`, bundle metadata, and `.gitignore` agree. |
| i18n keys | Locale YAML keys plus generated/typed wrappers in `services::i18n`. | Add tests that detect stale wrappers and unwrapped live keys, then remove unused keys or expose wrappers used by active code. |
| Capture action flow | `services::capture` returns domain outcomes; a single UI/platform executor interprets outcomes. | Stop repeating `ActionResult` handling in overlay, pin, and long-capture toolbar paths. |
| Shared panel layout | `ui::support` layout helpers. | Move duplicated overlay/long-capture toolbar placement math to one module with focused tests. |
| Native window handle extraction | One platform helper. | Remove duplicate Win32 `HWND` extraction from `native_window` and `window_drag`. |
| Architecture docs | One active spec and one active implementation plan. | Keep superseded docs under matching `archive` directories. |

## Workflow Design

Startup remains:

1. `main.rs` parses command and calls `app::run_command`.
2. `app::runtime` handles logging, single-instance checks, shutdown initialization, and command routing.
3. `app::composition` initializes GPUI assets, locale, `gpui_component`, appearance, globals, hotkeys, tray, and background host.

The composition layer should register workflows but not hold workflow internals. Quick capture should move behind a named workflow module or coordinator that returns a clear result. The composition layer can bind that workflow to tray and hotkey callbacks.

Capture actions should become a two-step flow:

1. Domain step: resolve/crop/save temporary data/decode QR as needed and return a typed `CaptureActionOutcome`.
2. UI/platform step: one executor translates the outcome into notification, clipboard text, pin window, close/refresh, or error behavior.

This keeps capture processing testable without GPUI and makes UI features reuse one interpretation path.

## Cleanup Order

1. Replace active architecture docs and archive superseded docs.
2. Fix source-of-truth drift: app identity, `Cargo.lock`, icon generation policy, locale key coverage.
3. Split `app::composition` workflow behavior into named app workflow modules.
4. Consolidate capture action outcome interpretation.
5. Extract shared panel layout helpers used by overlay and long capture.
6. Consolidate native window handle helpers.
7. Delete wrappers, public exports, modules, and dependencies that have no call sites or no boundary value.

## Testing And Verification

Required gates for every implementation phase:

```bash
cargo fmt --check
cargo check -p minnow-app
cargo test -p minnow-app --test module_layout_smoke -- --nocapture
cargo machete
```

Required gates before closing the refactor:

```bash
python scripts/check_no_qt_runtime_deps.py
cargo fmt --check
cargo check -p minnow-app --all-targets
cargo test -p minnow-app --test module_layout_smoke -- --nocapture
cargo test -p minnow-app
CARGO_INCREMENTAL=0 cargo clippy -q -p minnow-app --all-targets -- -D warnings
cargo machete
rg -n -F -e "legacy/qt" -e "legacy::" -e "cxx_qt" -e "cxx-qt" -e "qt_" Cargo.toml crates/minnow-app/Cargo.toml crates/minnow-app/build.rs crates/minnow-app/src crates/minnow-app/tests --glob "!target/**"
```

If Windows resource pressure prevents broad `cargo test` or clippy from completing, record the exact error and rerun narrower gates that still prove the changed boundary.

## Acceptance Criteria

- Only one active architecture spec exists under `docs/specs`; superseded specs are in `docs/specs/archive`.
- Only one active architecture implementation plan exists under `docs/plans` after the plan is written; superseded plans are in `docs/plans/archive`.
- `docs/prompts/2026-05-21-architecture-refactor-prompt.md` no longer points readers at stale active docs.
- `module_layout_smoke` covers public crate API, app composition boundary, UI platform facade boundary, cross-feature private module boundaries, legacy Qt isolation, and any new source-of-truth invariants introduced by this refactor.
- App identity has one authoritative value used by runtime, packaging metadata, lock id, and notification setup.
- `Cargo.lock` policy matches desktop app reproducibility.
- Generated icon artifacts are not a second unchecked source of truth.
- Locale wrappers and locale YAML files have test coverage that detects stale or missing active keys.
- Capture action result interpretation is centralized.
- Overlay and long-capture toolbar placement share one layout implementation.
- Win32 native handle extraction exists in one helper.
- Active Rust code and tests do not reference `legacy/qt` except through the dedicated guard script or documentation.
- Deleted code has call-site evidence and passing verification.

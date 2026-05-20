# Architecture Cleanup Refactor Spec

Status: Active
Tracking: `minnowsnap-0l0`
Supersedes:
- `docs/specs/archive/2026-05-18-architecture-cleanup-spec.md`
- `docs/specs/archive/2026-05-17-conservative-merge-refactor-design.md`

## Goal

Make MinnowSnap's Rust 2024 GPUI app easier to read, test, and evolve by aligning directories with real ownership, deleting dead compatibility paths, and keeping one source of truth for each domain. This refactor does not need to preserve internal old paths, shims, or stale wrappers.

## Current Problems

- Public API boundaries are mostly established, but some internal modules still expose broad `pub` surfaces and compatibility-style re-exports.
- UI feature directories contain large files whose boundaries sometimes reflect historical growth instead of stable responsibilities.
- Capture, OCR, pin, and long-capture paths still contain unused helpers and duplicated ownership around image buffers, monitor state, and preprocessing.
- Settings, i18n, appearance, hotkeys, and OCR preferences have multiple nearby representations that need explicit ownership.
- Legacy Qt code is archival only and must remain outside active build, tests, and documentation flows.
- Dead code exists in active Rust modules after earlier call sites moved.

## Target Boundaries

| Area | Owns | Must Not Own |
| --- | --- | --- |
| `app` | Command facade, application bootstrap, composition wiring, runtime entrypoint coordination. | Platform primitives, UI state internals, durable service policy. |
| `platform` | OS edges: native windows, hotkeys, tray, notifications, clipboard, logging, shutdown, background host. | User settings schema, GPUI feature state, OCR/capture business logic. |
| `services` | Durable app logic: settings, capture, OCR, i18n, assets, paths, fonts, geometry. | GPUI rendering, window widget composition, compatibility facades. |
| `ui/features/<feature>` | Feature state, view composition, input handling, and feature-local render helpers. | Cross-feature private imports, storage policy, heavy image/OCR/stitching work. |
| `ui/support` | Small GPUI support adapters shared by multiple features. | Shell/window behavior, service orchestration, duplicate settings/i18n state. |
| `legacy/qt` | Historical archive only. | Active build input, tests, or new implementation references. |

## Public API

The crate-root public facade is `minnow_app::app`:

- `minnow_app::app::Command`
- `minnow_app::app::parse_command`
- `minnow_app::app::parse_command_from`
- `minnow_app::app::run_command`

Everything else should be `pub(crate)` or private unless the binary, tests, or a documented facade needs it.

## Single Sources Of Truth

| Domain | Current Duplicate Or Drift Risk | Target Source Of Truth | Temporary Adapter Rule |
| --- | --- | --- | --- |
| Settings persistence | `services/settings.rs` plus UI preference state mirrors. | `services::settings` owns persisted schema and defaults. | UI state may mirror form values only while editing. |
| User-facing text | Rust helper functions plus locale YAML. | `services::i18n` function keys backed by locale YAML. | Remove unused i18n helpers when no Rust caller exists. |
| Hotkeys | Settings bindings, platform registration, preferences shortcut state. | `services::hotkeys` and `platform::hotkey` split parsing/storage from OS registration. | Preferences can dispatch updates but cannot own registration policy. |
| Appearance, language, font | Settings, `ui/support`, preferences general state. | Settings own persisted choices; `ui/support` adapts them for rendering. | No extra wrapper if a direct settings call is clearer. |
| Capture source cache | Capture service and overlay state both touch preview/scroll images. | `services::capture` owns cached source images and crop behavior. | UI may hold view-local handles, not duplicate repositories. |
| Monitor selection | Old target helpers and window catalog scaling can drift. | Active capture monitor resolution lives in `services::capture`; window catalog uses current monitor data directly. | Do not keep target shims without a live caller. |
| OCR models | OCR config, model manager, preferences OCR state. | `services::ocr` owns model paths, download state, recognition pipeline. | Preferences only shows and updates user intent. |
| OCR/pin rendering | OCR service output and pin text/geometry view code. | OCR service owns recognition data; pin view owns display geometry. | No OCR preprocessing in UI view modules. |
| Overlay annotation | Annotation model, render state, raster cache. | Overlay annotation module owns annotation data and rasterization. | Cross-feature imports cannot reach private render/state modules. |
| Legacy Qt | Archived historical source. | `legacy/qt` archive only. | No adapter, dependency, or active test may point at it. |

## Cleanup Rules

- Delete internal compatibility paths instead of preserving shims for old module names.
- Delete re-exports, wrappers, and helper functions when evidence shows no active caller and they are not part of `minnow_app::app`.
- Merge one-line forwarding functions unless the name carries domain meaning used by multiple call sites.
- Split large files by responsibility only when the new boundary is stable and removes cognitive load.
- Tighten visibility during every move; prefer private or `pub(crate)` over broad `pub`.
- Do not introduce empty traits, manager/facade layers, or service shells just to make the architecture look layered.
- Prefer existing mainstream dependencies and modern Rust APIs. New or upgraded libraries require a documented complexity reduction and current docs lookup before adoption.

## Deletion Evidence

Before removing a wrapper, re-export, shim, or dead module, record:

- `rg` or compiler warning evidence showing active call sites.
- Whether the symbol belongs to the documented `minnow_app::app` facade.
- The compile or test command that proves removal remains valid.
- Any follow-up Beads issue if the deletion exposes a larger design problem.

## Non-Goals

- No migration away from GPUI / `gpui-component`.
- No framework, runtime, or language rewrite.
- No reactivation of Qt/CXX-Qt code.
- No compatibility layer for internal old paths unless an active external consumer is proven.
- No broad phase-2 or phase-3 restructuring while executing a narrower claimed issue.

## Acceptance Criteria

- Only one active architecture spec/plan pair exists under `docs/specs` and `docs/plans`.
- Archived specs/plans live under the corresponding `archive` directory.
- `minnow_app::app` remains the only public crate-root facade.
- Active Rust code contains no references to `legacy/qt`.
- Deleted wrappers and dead modules have call-site evidence and fresh verification.
- `cargo fmt --check`, `cargo check -p minnow-app`, `cargo test -p minnow-app --test module_layout_smoke`, and issue-specific tests pass before closing a phase issue.
- `cargo machete` is clean or any tool/install blocker is recorded on the Beads issue.

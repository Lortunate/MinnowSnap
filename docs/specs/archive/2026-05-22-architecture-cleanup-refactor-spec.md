# Architecture Cleanup Refactor Spec

Date: 2026-05-22
Issue: `minnowsnap-2vo`
Status: Active

Supersedes:
- `docs/specs/archive/2026-05-21-architecture-cleanup-refactor-spec.md`
- `docs/specs/archive/2026-05-18-architecture-cleanup-spec.md`
- `docs/specs/archive/2026-05-17-conservative-merge-refactor-design.md`

## Goal

Make MinnowSnap's Rust 2024 GPUI application easier to read, test, and evolve by keeping one owner for each architectural responsibility. Internal compatibility with old paths is not required; stale wrappers, duplicate definitions, and dead code should be removed when call-site and verification evidence prove they are unused.

## Current Stack

- UI: GPUI and `gpui-component`.
- App shell: one binary crate, `minnow-app`.
- Platform integration: tray, hotkeys, native windows, notifications, clipboard, storage, and shutdown.
- Domain services: capture, long capture, OCR, geometry, settings, i18n, fonts, paths, and assets.
- Retired UI code: removed from the working tree.

Context7 documentation for GPUI and `gpui-component` was checked before this spec refresh. The current GPUI direction remains valid: initialize `gpui_component::init(cx)` during app startup before component use, and open windows with `Root` as the first-level view.

## Target Boundaries

| Area | Owns | Must Not Own |
| --- | --- | --- |
| `app` | Process entry flow, command dispatch, application composition, and top-level feature wiring. | Feature internals or domain algorithms. |
| `platform` | OS and shell integration: native windows, notifications, clipboard, tray, hotkeys, storage, shutdown. | Capture/OCR/settings business rules. |
| `platform::shell` | The UI-facing facade for platform side effects and shell window helpers. | Feature state machines or view rendering. |
| `services` | Domain logic and data transformations that can be tested without GPUI windows where possible. | GPUI view code or native window setup. |
| `ui::features` | GPUI views, feature state, user interaction handling, and feature-level public APIs. | Direct imports from private platform modules. |
| `ui::support` | Shared UI-only support such as appearance, locale, and image rendering helpers. | Platform shell/window helpers. |
| Retired UI sources | Historical archive. | Active build input, tests, or new implementation references. |

## Public API

- `src/lib.rs` exposes only `pub mod app;`.
- `app` exposes the binary command facade: `parse_command`, `run_command`, and `Command`.
- All other modules stay `pub(crate)` or private unless an integration test, binary entry point, or documented feature facade needs access.
- UI features may expose a small public feature API, but cross-feature access to private `state` and `render` modules is forbidden.

## Single Sources Of Truth

| Domain | Source Of Truth | Notes |
| --- | --- | --- |
| Runtime | `src/lib.rs` `RUNTIME` | Internal app runtime only. |
| Startup/composition | `src/app` | `main.rs` remains a thin command runner. |
| Shell side effects used by UI | `src/platform/shell.rs` | UI feature code reaches platform side effects through this facade only. |
| Native window setup | `src/platform/windowing.rs` and `src/platform/native_window.rs` behind `platform::shell` | Do not duplicate popup/window option construction inside services or UI support. |
| Capture/image pipeline | `src/services/capture` | UI dispatches actions; capture owns image IO, clipboard image copy, saving, QR decode, and stitching. |
| OCR | `src/services/ocr` | UI requests OCR; OCR service owns model state, preprocessing, and inference. |
| Settings | `src/services/settings.rs` | Preferences UI mutates settings through settings actions and registered shell services. |
| Retired UI | Removed source tree | No adapters, tests, or build paths should point to it. |

## Cleanup Rules

- Prefer deleting dead internal compatibility shims over preserving old module paths.
- Prefer one cohesive module or facade over duplicated partial wrappers.
- Do not introduce a new library unless it removes real complexity; check current docs before adoption.
- Do not move platform shell/window helpers into `ui::support` or `services`.
- Keep tests focused on boundaries that have regressed or are easy to regress.
- Record removal evidence before deleting wrappers or modules: call-site search, reason for deletion, and verification command.

## Acceptance Criteria

- Active specs/plans live under `docs/specs` and `docs/plans`; superseded specs/plans live under matching `archive` directories.
- `module_layout_smoke` locks the public crate API, app composition boundary, feature-private module boundary, shell helper placement, and UI platform facade boundary.
- Active Rust code contains no references to retired UI sources.
- `ui/features` imports platform APIs only through `crate::platform::shell`.
- Deleted wrappers/dead modules have call-site evidence and fresh verification.
- Required gates pass before closing a phase: `cargo fmt --check`, `cargo check -p minnow-app`, `cargo test -p minnow-app --test module_layout_smoke -- --nocapture`, plus any phase-specific tests.

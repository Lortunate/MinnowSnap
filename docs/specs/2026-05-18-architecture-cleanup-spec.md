# Architecture Cleanup Spec

Date: 2026-05-18
Status: Active
Tracking: `bd` issue `minnowsnap-0l0`

## Goal

Make MinnowSnap easier to read, change, and profile by reducing public surface area, keeping one owner per app concern, and deleting stale architecture documents before deeper refactors.

Compatibility with older internal module paths is not required. User-visible behavior should remain stable unless a later spec explicitly changes it.

## Architecture Boundaries

The crate root exposes only the binary-facing app facade:

- `minnow_app::app::parse_command`
- `minnow_app::app::run_command`
- `minnow_app::app::Command`

Everything else is crate-private implementation:

- `platform`: OS, window, tray, notification, shutdown, storage, and hotkey edges.
- `services`: durable app logic, settings, capture, OCR, i18n, assets, geometry, and paths.
- `ui`: GPUI feature modules and shared rendering support.

The app module owns startup composition but does not expose its wiring internals. `app::composition`, `app::runtime`, and `app::bootstrap` stay private; callers enter through the command facade only.

## Single Sources Of Truth

- Settings persistence lives in `services::settings`.
- User-facing labels live in `services::i18n` and locale YAML files.
- Capture commands live in `services::capture::action::CaptureAction`.
- Window and shell behavior lives in `platform`, not `ui/support` or `services`.
- Feature-private render/state modules are not imported by other features.

## Code Health Rules

- Prefer deleting zero-value wrappers over preserving old file paths.
- Prefer crate-private modules unless the binary or tests need an explicit public facade.
- Keep heavy image, OCR, and stitching work in services; UI modules should orchestrate state and render.
- Avoid extra cloning of image buffers or app state in hot paths.
- Keep tests that lock architecture boundaries close to the rule they enforce.

## Archive Policy

Historical specs and plans move under `docs/specs/archive/` or `docs/plans/archive/` when they no longer describe the active architecture. Archived documents are reference material, not current design authority.

The previous conservative refactor spec is archived at:

- `docs/specs/archive/2026-05-17-conservative-merge-refactor-design.md`

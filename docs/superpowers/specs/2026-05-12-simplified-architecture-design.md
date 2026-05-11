# MinnowSnap Simplified Architecture Design

**Date:** 2026-05-12

## Goal

Reduce cognitive load by converging the active GPUI runtime into one primary crate, `minnow-app`, while keeping clean internal boundaries for application bootstrap, platform integration, domain services, and UI features.

## Scope

In scope:
- Simplify the active Rust workspace structure.
- Replace support crates used only by this app with internal modules.
- Replace the current `minnow-core` / `minnow-ui` split with directory boundaries inside `minnow-app`.
- Preserve existing user-facing behavior during the migration.

Out of scope:
- `legacy/qt`
- Feature redesign
- OCR algorithm changes
- New user-facing functionality

## Why Change

The current workspace is understandable, but it carries more indirection than this project needs:

- `minnow-app`, `minnow-core`, `minnow-ui`, `minnow-assets`, and `minnow-paths` are all active runtime crates for one desktop app.
- `minnow-core` is trending toward a broad shared bucket for unrelated concerns.
- Simple call paths cross crate boundaries even when there is no reuse requirement.
- Moving code between app, core, and UI requires extra manifest work and import churn without adding enough isolation benefit.

The project is not over-engineered everywhere, but it is more distributed than necessary for a single desktop application.

## Design Principles

1. Prefer one active runtime crate over several small internal crates.
2. Use directories and module boundaries before introducing crate boundaries.
3. Keep `main.rs` and startup glue thin.
4. Do not create a new "core" bucket with unclear ownership.
5. Separate platform concerns from product concerns.
6. Keep UI feature folders aligned with user workflows, not technical layers alone.
7. Only keep a separate crate when it provides a strong payoff in reuse or isolation.

## Target Workspace

The target workspace should treat `crates/minnow-app` as the only active runtime crate.

Recommended end state:

- Keep:
  - `crates/minnow-app`
- Remove as active runtime crates by merging into `minnow-app`:
  - `crates/minnow-assets`
  - `crates/minnow-paths`
  - `crates/minnow-core`
  - `crates/minnow-ui`
  - `crates/minnow-ocr`

If OCR compile or dependency isolation becomes a serious problem later, `services/ocr` can be extracted again. That is a future optimization, not the preferred starting point.

## Target Directory Structure

```text
crates/minnow-app/
  resources/
  assets_icons/
  build.rs
  Cargo.toml
  src/
    main.rs
    lib.rs

    app/
      mod.rs
      bootstrap.rs
      runtime.rs
      composition.rs
      commands.rs

    platform/
      mod.rs
      logging.rs
      notify.rs
      shutdown/
        mod.rs
        control_plane.rs
        windows.rs
      clipboard.rs
      storage.rs
      hotkey.rs
      tray.rs
      native_window.rs
      windowing.rs
      window_drag.rs
      background_host.rs
      async_ui.rs

    services/
      mod.rs
      app_meta.rs
      assets/
        mod.rs
        asset_bytes.rs
        asset_paths.rs
      paths.rs
      geometry.rs
      i18n.rs
      settings.rs
      capture/
        mod.rs
        action.rs
        long_capture.rs
        service.rs
        source.rs
        stitcher.rs
      ocr/
        mod.rs
        config.rs
        detector.rs
        engine.rs
        model_manager.rs
        preprocess.rs
        recognizer.rs
        service.rs
        visualization.rs

    ui/
      mod.rs
      key_unicode.rs
      support/
        mod.rs
        appearance.rs
        locale.rs
        render_image.rs
      features/
        mod.rs
        overlay/
        pin/
        preferences/
        long_capture/
```

## Boundary Rules

### `main.rs`

- Parses CLI arguments.
- Calls into `app`.
- Contains no business logic.

### `app/`

- Owns startup orchestration.
- Wires together platform, services, and UI.
- Contains the root runtime lifecycle and top-level commands.
- Must stay thin and procedural.

### `platform/`

- Owns OS integration and shell-level concerns.
- Contains logging, notifications, shutdown plumbing, clipboard, storage adapters, tray, hotkeys, and native window helpers.
- May depend on OS-specific crates and GPUI shell APIs.
- Must not contain product rules such as capture flow policy or OCR formatting.

### `services/`

- Owns app capabilities and data rules.
- Contains settings, paths, i18n, geometry, capture, OCR, and asset access.
- Should avoid OS-specific code except where a capability inherently requires a platform adapter.
- Should not depend on UI feature modules.

### `ui/`

- Owns user-facing views, interaction state, and workflow presentation.
- May depend on GPUI and on `services`.
- Must not depend directly on OS-specific crates such as `windows`, `winreg`, or platform-only notification APIs.

## Feature Structure Rules

Top-level UI features should stay aligned with actual workflows:

- `overlay`
- `pin`
- `preferences`
- `long_capture`

Within a feature:

- Split when the submodule has a distinct responsibility.
- Do not create thin wrappers just to make the tree look symmetrical.
- Keep rendering, state, and domain-specific submodules only when they are independently understandable.

Specific guidance:

- `overlay` can stay as the largest feature because the domain is genuinely large.
- `preferences` should stay grouped by settings domain, but page/state splits should be merged when they become boilerplate-only.
- `long_capture` is justified as its own feature because it coordinates multiple windows and asynchronous behavior.

## Dependency Rules

Allowed direction:

- `main` -> `app`
- `app` -> `platform`
- `app` -> `services`
- `app` -> `ui`
- `ui` -> `services`
- `platform` -> `services` only where shell integration needs shared types or settings

Disallowed direction:

- `services` -> `ui`
- `services` -> `app`
- `platform` -> `ui` feature internals
- Reintroducing any crate or module named `core` as a catch-all owner

## Migration Strategy

Use a staged refactor that preserves behavior:

1. Create the target module namespaces inside `minnow-app`.
2. Move support crates (`minnow-assets`, `minnow-paths`) into `services`.
3. Move OCR internals into `services/ocr`.
4. Split `minnow-core` responsibilities into `services` and `platform`.
5. Move `minnow-ui` into `ui`, moving shell-only helpers into `platform`.
6. Remove obsolete crate members and simplify manifests only after the code compiles from the new locations.

## Acceptance Criteria

- The workspace builds and runs with `minnow-app` as the only active runtime crate.
- Support functionality currently in `minnow-assets`, `minnow-paths`, `minnow-core`, `minnow-ui`, and `minnow-ocr` is reachable through `minnow-app` modules.
- `main.rs` remains thin.
- No new `core`-style catch-all module is introduced.
- Existing capture, pin, preferences, tray, OCR, and shutdown flows still work.
- The resulting tree is easier to navigate than the current crate split.

## Risks

- Large refactors can break imports and module visibility in many files at once.
- Folding OCR into the main crate increases local module count, even while reducing cross-crate complexity.
- Moving shell code out of UI may surface places where current boundaries are mixed.

## Mitigations

- Migrate in small, compiling phases.
- Add `lib.rs` early so module boundaries become testable.
- Keep behavior-preserving smoke tests around CLI parsing, settings loading, and core feature entry points.
- Remove old crates only after the new structure is fully wired and verified.

# Legacy Qt/CXX-Qt Archive

This directory contains the retired Qt/CXX-Qt implementation moved out of the active build.

Status
- Retired on 2026-04-07.
- Not compiled or packaged by the current GPUI pipeline.
- Kept only for historical reference and migration traceability.
- Treated as read-only archive content for normal feature work.

Archived Areas
- `src/bridge`: CXX-Qt bridge layer and Qt QObject/QML bindings.
- `src/interop`: Qt type adapters (`QRectF`, `QUrl`) used by the bridge layer.
- `qml`: Legacy QML views and components.
- `cpp`: Qt/C++ helper headers and Objective-C++ bridge snippets.
- `resources/i18n/zh_CN.ts`: Legacy Qt translation source.
- `rust/build-tools/src/{i18n,resources,utils}.rs`: Qt-oriented build helpers.
- `resources.qrc` and `.qmlls.ini`: legacy local Qt resource/editor config.

Legacy -> GPUI Mapping
- `bridge/screen_capture`, `bridge/capture_session`, `bridge/capture_compositor`, `bridge/overlay_controller`, `bridge/annotation`
  -> `crates/minnow-ui/src/features/overlay/*` + `crates/minnow-core/src/capture/*`
- `bridge/pin`, `bridge/ocr_overlay`
  -> `crates/minnow-ui/src/features/pin/*`
- `bridge/config`, `bridge/shortcut_helper`
  -> `crates/minnow-core/src/settings.rs` + `crates/minnow-ui/src/shell/hotkey.rs` + `crates/minnow-ui/src/features/preferences/*`
- `bridge/tray_menu`
  -> `crates/minnow-ui/src/shell/tray.rs`
- `bridge/window`
  -> `crates/minnow-ui/src/shell/windowing.rs` + `crates/minnow-ui/src/shell/native_window.rs`
- `qml/features/preferences/*`
  -> `crates/minnow-ui/src/features/preferences/*`

Do not add new runtime code under `legacy/qt`.
Do not add references from active runtime paths (`crates/`, `Cargo.toml`, build or CI scripts) back to `legacy/qt`.
CI guard: `scripts/check_no_qt_runtime_deps.py`.

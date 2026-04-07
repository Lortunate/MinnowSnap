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
  -> `src/ui/overlay/*` + `src/core/capture/*`
- `bridge/pin`, `bridge/ocr_overlay`
  -> `src/ui/pin/*`
- `bridge/config`, `bridge/shortcut_helper`
  -> `src/core/settings/*` + `src/core/hotkey.rs` + `src/ui/preferences/*`
- `bridge/tray_menu`
  -> `src/ui/tray_icon.rs` + `src/core/tray/*`
- `bridge/window`
  -> `src/ui/windowing.rs` + `rust/gpui-window-ext/*`
- `qml/features/preferences/*`
  -> `src/ui/preferences/*`

Do not add new runtime code under `legacy/qt`.
Do not add references from active runtime paths (`src/`, `Cargo.toml`, build or CI scripts) back to `legacy/qt`.
CI guard: `scripts/check_no_qt_runtime_deps.py`.

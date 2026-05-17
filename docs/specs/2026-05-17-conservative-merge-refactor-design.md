# Conservative Merge Refactor — Design Spec

Date: 2026-05-17
Status: Approved
Branch: refactor/simplified-architecture

## Goal

Simplify the MinnowSnap codebase by eliminating zero-value abstractions, consolidating fragment files, and replacing boilerplate with macros. No architectural layer changes. Public API unchanged.

Expected outcome: ~600 LOC removed, 6 files deleted, improved readability and maintainability.

## 1. i18n Macro Replacement

**Problem**: `services/i18n.rs` is 685 LOC of hand-written accessor functions. Each follows the identical pattern `pub fn name() -> String { t!("key").into_owned() }`.

**Solution**: Replace with a declarative macro `i18n_fns!` that generates the same public API from a compact key table.

Macro design:
```rust
macro_rules! i18n_fns {
    // Zero-argument variant
    ($($fn_name:ident => $key:literal),* $(,)?) => {
        $(pub fn $fn_name() -> String { t!($key).into_owned() })*
    };
}

macro_rules! i18n_fns_with_args {
    // Single-argument variant
    ($($fn_name:ident($($arg:ident),+) => $key:literal),* $(,)?) => {
        $(pub fn $fn_name($($arg: impl ToString),+) -> String {
            t!($key, $($arg = $arg.to_string()),+).into_owned()
        })*
    };
}
```

Each sub-module (app, common, overlay, notify, capture, preferences, pin, tray) becomes a compact table of `name => "key.path"` entries. The ~12 parameterized functions use the `_with_args` variant.

**Result**: 685 LOC → ~120 LOC. Same public API, zero caller changes.

## 2. Overlay Fragment Consolidation

### 2a. Merge state/frame.rs + state/surface.rs → state/session.rs

- `frame.rs` (28 LOC): single `frame()` method on `OverlaySession`
- `surface.rs` (35 LOC): `OverlaySurface` struct + constructor

Both are trivial extensions with no independent logic. Session.rs grows from 354 to ~417 LOC — acceptable.

### 2b. Merge interaction.rs → view/input.rs

- `interaction.rs` (113 LOC): `resolve_mouse_down_command()` + hit testing
- `view/input.rs` (75 LOC): mouse/key event handlers that call interaction

These are the same conceptual layer (raw events → commands). Combined file: ~188 LOC.

### 2c. Keep state/annotation.rs (159 LOC) as-is

Pure delegation but prevents session.rs from exceeding 500 LOC. Not worth the merge.

**Files deleted**: 3 (frame.rs, surface.rs, interaction.rs)

## 3. Settings Setter Inlining

**Problem**: 13 private setter methods in `settings.rs`, each a one-liner like:
```rust
fn set_save_path(&mut self, path: Option<String>) {
    self.update(|c| c.output.save_path = path);
}
```

**Solution**: Inline directly into the `apply()` match arms:
```rust
SettingsAction::SetSavePath(path) => self.update(|c| c.output.save_path = path),
```

**Result**: ~60 LOC removed. `apply()` becomes self-contained. The `SettingsAction` enum stays (it's a clean API boundary).

## 4. Capture Module Consolidation

### 4a. Merge repository.rs (53 LOC) → capture/mod.rs

`CaptureRepository` is `pub(super)` — only used by the parent module. No external consumers.

### 4b. Merge source.rs (39 LOC) → capture/mod.rs

`VirtualCaptureSource` enum + 4 parsing functions. Tightly coupled to the repository.

### Keep action.rs (150 LOC) and target.rs (55 LOC)

Both have independent logic and public types that justify separate files.

**Files deleted**: 2 (repository.rs, source.rs)

## 5. Platform async_ui Merge

**Problem**: `platform/async_ui.rs` is 10 LOC (two functions, zero types).

**Solution**: Move `app_ready()` and `update_app()` into `platform/mod.rs`.

**Files deleted**: 1

## What Stays Unchanged

- Global state pattern (`LazyLock<Mutex<T>>`) — appropriate for GUI apps
- `geometry.rs` location — pure math, no GPUI dependency, easy to test
- `windowing.rs` / `system.rs` — small but distinct responsibilities
- Overlay command/effects/picker/diagnostics/selection — each has substantial logic
- All public APIs and module paths (except deleted internal files)

## Execution Order

1. i18n macro replacement (biggest LOC win, isolated change)
2. Settings setter inlining (single file, no cross-module impact)
3. Capture module consolidation (two files merged into parent)
4. Platform async_ui merge (trivial)
5. Overlay fragment consolidation (most files touched, do last)
6. Final: cargo build + cargo test to verify

## Risk Assessment

- **Low risk**: All changes are internal reorganization. No public API changes.
- **No behavioral changes**: Pure structural refactoring.
- **Rollback**: Each step is independently committable and revertable.

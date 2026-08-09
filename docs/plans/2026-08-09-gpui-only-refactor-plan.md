# GPUI-Only Architecture Refactor Plan

Date: 2026-08-09
Status: Active
Spec: `docs/specs/2026-05-24-architecture-code-directory-refactor-spec.md`

## Phase 1: Remove retired UI sources (completed)

Delete the retired source tree, its dependency guard, stale ignore rules, and
CI steps. Track `Cargo.lock`, remove checked-in generated icons, and pin the
resolved GPUI release. Add a source scan to the architecture smoke test.

Verification: repository scan, `cargo metadata`, and the smoke test. The
retired tree, guard script, generated icon, and stale Qt/QML references are
gone; `Cargo.lock` is tracked and GPUI is pinned.

## Phase 2: Make capture workflows deep (completed)

Move quick-capture orchestration and action side effects out of the capture
domain service. The domain planner returns typed plans; one application workflow
executes clipboard, storage, notification, and temporary-file effects through
the platform shell seam. Update overlay, pin, and long-capture callers together.

Verification: capture unit tests, smoke test, and workspace check. Quick capture
and image-action effects now have one application workflow, with UI result
interpretation shared by all three GPUI capture hosts.

## Phase 3: Consolidate shared platform/layout seams (completed)

Reuse one native window-handle extractor and one toolbar/panel placement module.
Delete duplicate helpers after call-site searches prove they are private-only.

Verification: focused geometry tests, smoke test, format, check, and clippy.
Native handle extraction and toolbar placement each have one active owner.

## Phase 4: Toolchain and final gates (completed)

Pin the current stable Rust toolchain in `rust-toolchain.toml`, keep the package
MSRV aligned, and run every required workspace gate. Separate host graphics SDK
failures from source failures in the handoff.

Verification: Rust `1.97.1` is current and up to date at execution time;
`cargo fmt --check`, `cargo check --workspace --all-targets`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo test --workspace`, and `cargo build --workspace --release` all pass on
the host. The Windows GNU cross-check was attempted separately and is blocked
only by the unavailable `x86_64-w64-mingw32-gcc` toolchain.

## Phase 5: Deepen capture and settings state ownership (completed)

Separate strict scaled-region cropping from user-selection recovery, then route
shared and owned capture images through that single policy. Move long-capture
event transitions, revision updates, and window-handle cleanup into the
coordinator state owner; recover poisoned capture locks without discarding the
latest result.

Replace unordered per-update settings writes with one serial persistence
adapter. Keep the in-memory store authoritative, preserve the existing settings
API, and flush all queued snapshots after the GPUI application exits.

Verification: focused crop/state/persistence tests, full library tests,
architecture smoke tests, workspace check, and all-target strict clippy. Capture
selection math has one owner, long-capture transitions have one owner, and older
settings snapshots can no longer overtake newer snapshots.

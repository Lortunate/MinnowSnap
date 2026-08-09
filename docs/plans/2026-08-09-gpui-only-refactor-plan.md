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

## Phase 4: Toolchain and final gates

Pin the current stable Rust toolchain in `rust-toolchain.toml`, keep the package
MSRV aligned, and run every required workspace gate. Separate host graphics SDK
failures from source failures in the handoff.

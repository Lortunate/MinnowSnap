# minnow-capture

Platform-independent capture contracts, scroll alignment and bounded image assembly.
Native APIs belong to `minnow-platform`; job ownership belongs to
`minnow-application`; windows and presentation belong to `minnow-ui`.

## Responsibilities

```text
CaptureBackend -> CaptureSource (one per run, owned by the worker)
                        |
                        v
CaptureRun -> ScrollCapture -> matching -> ScrollStitcher
    |              |            decision       commit
    v              v
PreviewSlot    CaptureOutcome
```

- `source.rs`: acquisition session contract. The default adapter pins the first
  display and rejects display identity changes. Native adapters can own filters
  and other resources for the entire run without making them thread-safe.
- `run.rs`: cooperative control, acquisition cadence and latest-only preview
  publication. Opens one source and drops it before publishing a terminal result.
- `scroll.rs`: selection/geometry validation, resource admission and terminal
  outcome. A session cannot restart after completion.
- `matching.rs`: immutable accepted-frame analysis and overlap decisions.
  Row descriptors propose a displacement; full RGB verification checks the
  overlap and chooses a seam. Decisions expose evidence or a typed rejection.
- `stitcher.rs`: the only owner of the mutable output canvas. Commits a verified
  seam, grows storage within the working budget and maintains a bounded preview.

Alignment uses the last accepted frame. Rejected observations never replace this
anchor or mutate the committed image. Fixed top/bottom regions are excluded from
matching; committing a forward frame replaces the complete old tail from the
verified seam, keeping one current footer. Reverse motion, insufficient detail,
ambiguous periodic content and lost overlap are rejected conservatively.
Horizontal scrolling and arbitrary animated pages are not supported.

## Completion and recovery

`Ready` means that all currently retained content has a verified continuation.
Finishing with an unresolved rejection returns `Failed(Incomplete)` plus the
useful partial image. A later verified forward frame or stationary observation
can clear the rejection. Cancelled and failed results remain distinct from
successful completion, even when they contain an exportable partial image.

Cancellation is cooperative: an in-flight native call may finish before the
worker releases its resources. The application supervises the worker through
that release. The UI closes the selection overlay and prepares capture controls
before releasing the acquisition start gate.

## Resource policy

| Resource | Default limit |
| --- | --- |
| Native source logical pixels | 32 Mi pixels |
| Stitched output logical pixels | 64 Mi pixels |
| Capture working set | 512 MiB |
| Preview | 500 px target width; 4096 px edge; 1 Mi pixels |
| Preview publication interval | 33 ms |

Admission accounts for source storage, input/analysis buffers, previews and both
old/new canvases during growth. These are local allocation limits, not a process
RSS guarantee: native compositor storage, allocator overhead and image copies
retained by external consumers are outside this accounting. Allocation failures
are reported as capture errors.

## Validation

`cargo test -p minnow-capture` runs state/lifecycle/resource tests and deterministic
pixel replays. `tests/scroll_replay.rs` replays an article raster at six scroll
steps (including 1, 2 and 4 pixels), 273 periodic-sidebar/fixed-edge combinations,
and ambiguous-content recovery. Every committed pixel is checked against the
expected document. Fixture provenance is in `tests/fixtures/README.md`.

These tests establish algorithm behavior for their inputs. Native acquisition,
window exclusion, display changes and visual behavior still require an OS session
with screen-recording permission. macOS scrolling uses ScreenCaptureKit on macOS
14 or newer and excludes the application's windows through its session filter;
Windows uses capture-excluded control windows. Neither native exclusion contract
is established by pixel replay tests.

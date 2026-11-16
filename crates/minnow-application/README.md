# minnow-application

Application use cases and execution lifecycle.

- Coordinates capture, annotation export, recognition and output.
- Owns document sessions, background jobs, cancellation and shutdown.
- Defines the contracts implemented by native and recognition adapters.

Depends on core capabilities; independent of GPUI and concrete OS or OCR backends.

## Long capture

`LongCaptureHandle` owns control requests and the latest preview; the supervised
job owns the acquisition worker. Preparation returns a handle before acquisition
starts, so the UI can prepare its windows and close the selection overlay before
releasing the start gate. Abandoning preparation or cancelling before start
cancels the job without opening a capture source. Cancellation after start waits
for the worker to release the source.

Capture failures retain their typed `CaptureError` separately from job/task
failures. Partial images stay attached to failed/cancelled outcomes and require
an explicit user action to use them. Alignment and resource policy are documented
in [minnow-capture](../minnow-capture/README.md).

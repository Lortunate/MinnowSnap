# minnow-application

Application use cases and execution lifecycle.

- Coordinates capture, annotation export, recognition and output.
- Owns document sessions, background jobs, cancellation and shutdown.
- Defines the contracts implemented by native and recognition adapters.

Depends on core capabilities; independent of GPUI and concrete OS or OCR backends.

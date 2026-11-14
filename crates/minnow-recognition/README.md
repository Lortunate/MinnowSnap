# minnow-recognition

Local OCR, QR decoding and model lifecycle.

- `ocr/`: inference planning, bounded batches and engine integration.
- `models/`: verified candidates, atomic publication and generation retention.
- `service.rs` and `local.rs`: runtime capability, shared inference and task lifetime.

Implements application recognition contracts. The executable creates one shared service.

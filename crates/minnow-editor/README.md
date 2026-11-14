# minnow-editor

Annotation documents and rendering.

- Owns editing interactions, hit testing and undo/redo history.
- Produces immutable snapshots and CPU-rendered image layers.
- Receives image geometry and fonts from callers.

GPU textures, font discovery and file output belong to other packages.

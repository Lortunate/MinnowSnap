//! Editing documents, annotation interactions, history, and CPU rasterization.
//!
//! Coordinates use the logical canvas space supplied by the caller. Rasterization
//! converts that space using an explicit scale; no display or UI state is queried.
mod engine;
mod hit_test;
mod model;
mod ops;
mod raster;
mod raster_cache;
mod store;

pub use engine::{AnnotationEngine, AnnotationInputError, AnnotationOutputError, AnnotationOutputIdentity, AnnotationRevisions, AnnotationSnapshot};
pub use model::{
    AnnotationItem, AnnotationKind, AnnotationKindTag, AnnotationLayerState, AnnotationOutline, AnnotationSelectionInfo, AnnotationStyleState,
    AnnotationTool, AnnotationUiState, COLOR_PRESETS, MosaicMode,
};
pub use raster_cache::{AnnotationRasterDiagnostics, RasterBudget};
pub use store::{HistoryBudget, HistoryUsage};

use image::RgbaImage;
use std::sync::{Arc, Weak};

use minnow_imaging::geometry::RectF;

/// Maximum retained derived pixels. Caller-owned source pixels and returned
/// images are outside this cache budget. Oversized selections render directly;
/// the budget never removes annotations or prevents a complete export.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RasterBudget {
    pub max_cached_bytes: usize,
}

impl Default for RasterBudget {
    fn default() -> Self {
        Self {
            max_cached_bytes: 128 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CommittedLayerCache {
    pub selection: RectF,
    pub scale: f64,
    pub committed_revision: u64,
    pub source: Weak<RgbaImage>,
    pub image: Arc<RgbaImage>,
}

#[derive(Clone, Debug)]
pub(crate) struct ComposedLayerCache {
    pub source: Weak<RgbaImage>,
    pub selection: RectF,
    pub scale: f64,
    pub committed_revision: u64,
    pub transient_revision: u64,
    pub preview_translate: Option<(f64, f64)>,
    pub image: Arc<RgbaImage>,
}

#[derive(Clone, Debug)]
pub(crate) struct InteractionBaseCache {
    pub selection: RectF,
    pub scale: f64,
    pub committed_revision: u64,
    pub moving_id: u64,
    pub source: Weak<RgbaImage>,
    pub rgba: Arc<RgbaImage>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AnnotationRasterDiagnostics {
    pub cached_bytes: usize,
    pub budget_bytes: usize,
    pub uncached_renders: u64,
    pub committed_rebuilds: u64,
    pub composed_rebuilds: u64,
    pub interaction_base_rebuilds: u64,
    pub drawing_fast_path_hits: u64,
    pub moving_fast_path_hits: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AnnotationRasterCache {
    pub committed: Option<CommittedLayerCache>,
    pub composed: Option<ComposedLayerCache>,
    pub interaction_base: Option<InteractionBaseCache>,
    pub budget: RasterBudget,
    pub uncached_renders: u64,
    pub committed_rebuilds: u64,
    pub composed_rebuilds: u64,
    pub interaction_base_rebuilds: u64,
    pub drawing_fast_path_hits: u64,
    pub moving_fast_path_hits: u64,
}

impl AnnotationRasterCache {
    pub(crate) fn cached_bytes(&self) -> usize {
        self.committed
            .as_ref()
            .map_or(0, |cache| cache.image.as_raw().len())
            .saturating_add(self.composed.as_ref().map_or(0, |cache| cache.image.as_raw().len()))
            .saturating_add(self.interaction_base.as_ref().map_or(0, |cache| cache.rgba.as_raw().len()))
    }

    pub(crate) fn enforce_budget(&mut self) {
        if self.cached_bytes() > self.budget.max_cached_bytes {
            self.interaction_base = None;
        }
        if self.cached_bytes() > self.budget.max_cached_bytes {
            self.committed = None;
        }
        if self.cached_bytes() > self.budget.max_cached_bytes {
            self.composed = None;
        }
    }

    pub(crate) fn invalidate_layers(&mut self) {
        self.committed = None;
        self.composed = None;
        self.interaction_base = None;
    }

    pub(crate) fn clear(&mut self) {
        self.invalidate_layers();
        self.committed_rebuilds = 0;
        self.composed_rebuilds = 0;
        self.interaction_base_rebuilds = 0;
        self.drawing_fast_path_hits = 0;
        self.moving_fast_path_hits = 0;
        self.uncached_renders = 0;
    }

    pub(crate) fn invalidate_composed(&mut self) {
        self.composed = None;
    }

    pub(crate) fn invalidate_interaction_base(&mut self) {
        self.interaction_base = None;
    }

    pub(crate) fn diagnostics(&self) -> AnnotationRasterDiagnostics {
        AnnotationRasterDiagnostics {
            cached_bytes: self.cached_bytes(),
            budget_bytes: self.budget.max_cached_bytes,
            uncached_renders: self.uncached_renders,
            committed_rebuilds: self.committed_rebuilds,
            composed_rebuilds: self.composed_rebuilds,
            interaction_base_rebuilds: self.interaction_base_rebuilds,
            drawing_fast_path_hits: self.drawing_fast_path_hits,
            moving_fast_path_hits: self.moving_fast_path_hits,
        }
    }
}

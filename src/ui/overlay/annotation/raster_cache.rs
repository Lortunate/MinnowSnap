use gpui::RenderImage;
use image::RgbaImage;
use std::sync::Arc;

use crate::core::geometry::RectF;

#[derive(Clone, Debug)]
pub(crate) struct CommittedLayerCache {
    pub selection: RectF,
    pub scale: f64,
    pub revision: u64,
    pub rgba: Arc<RgbaImage>,
    pub image: Arc<RenderImage>,
}

#[derive(Clone, Debug)]
pub(crate) struct ComposedLayerCache {
    pub selection: RectF,
    pub scale: f64,
    pub committed_revision: u64,
    pub transient_revision: u64,
    pub image: Arc<RenderImage>,
}

#[derive(Clone, Debug)]
pub(crate) struct InteractionBaseCache {
    pub selection: RectF,
    pub scale: f64,
    pub committed_revision: u64,
    pub moving_id: u64,
    pub rgba: Arc<RgbaImage>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct AnnotationRasterDiagnostics {
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
    pub scratch_layer: Option<RgbaImage>,
    pub committed_rebuilds: u64,
    pub composed_rebuilds: u64,
    pub interaction_base_rebuilds: u64,
    pub drawing_fast_path_hits: u64,
    pub moving_fast_path_hits: u64,
}

impl AnnotationRasterCache {
    pub(crate) fn clear(&mut self) {
        self.committed = None;
        self.composed = None;
        self.interaction_base = None;
        self.scratch_layer = None;
        self.committed_rebuilds = 0;
        self.composed_rebuilds = 0;
        self.interaction_base_rebuilds = 0;
        self.drawing_fast_path_hits = 0;
        self.moving_fast_path_hits = 0;
    }

    pub(crate) fn invalidate_composed(&mut self) {
        self.composed = None;
    }

    pub(crate) fn invalidate_interaction_base(&mut self) {
        self.interaction_base = None;
    }

    #[cfg(any(feature = "overlay-diagnostics", test))]
    pub(crate) fn diagnostics(&self) -> AnnotationRasterDiagnostics {
        AnnotationRasterDiagnostics {
            committed_rebuilds: self.committed_rebuilds,
            composed_rebuilds: self.composed_rebuilds,
            interaction_base_rebuilds: self.interaction_base_rebuilds,
            drawing_fast_path_hits: self.drawing_fast_path_hits,
            moving_fast_path_hits: self.moving_fast_path_hits,
        }
    }
}

use gpui::RenderImage;
use image::{GenericImage, RgbaImage};
use std::sync::Arc;

use crate::services::capture::PREVIEW_SOURCE;
use crate::services::capture::service::CaptureService;
use crate::services::geometry::RectF;
use crate::ui::support::render_image;

use super::super::model::{
    AnnotationInteractionState, AnnotationItem, AnnotationKind, AnnotationKindTag, AnnotationLayerState, AnnotationOutline, AnnotationSelectionInfo,
    AnnotationUiState,
};
use super::super::ops::{build_drawing_item, contains_point_with_bounds};
use super::super::raster::{compose_background_with_annotations, compose_selection_background, compose_selection_base, draw_items_on_selection};
#[cfg(any(feature = "overlay-diagnostics", test))]
use super::super::raster_cache::AnnotationRasterDiagnostics;
use super::super::raster_cache::{CommittedLayerCache, ComposedLayerCache, InteractionBaseCache};
use super::AnnotationEngine;

impl AnnotationEngine {
    pub(crate) fn hit_test(&self, point: (f64, f64), selection: Option<RectF>, idle_mode: bool) -> Option<u64> {
        if !self.mode_enabled(selection, idle_mode) || !self.point_in_selection(point, selection) {
            return None;
        }
        let mut proxy = self.hit_proxy.borrow_mut();
        proxy.sync(self.committed_revision, self.store.visible_items());
        proxy.hit_test(point, |id, bounds, _| {
            self.store
                .visible_item(id)
                .is_some_and(|item| contains_point_with_bounds(item, point, bounds))
        })
    }

    pub(crate) fn ui_state(
        &mut self,
        selection: Option<RectF>,
        background: Option<&Arc<RgbaImage>>,
        scale: f64,
        preview_translate: Option<(f64, f64)>,
    ) -> AnnotationUiState {
        let layer = self.layer_state(selection, background, scale, preview_translate);
        AnnotationUiState {
            layer,
            selected: self.selected_summary(),
            tool: self.tool,
            style: self.style,
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
            text_editing: self.text_editing.is_some(),
        }
    }

    pub(crate) fn composed_background_source(&self, background: Option<&Arc<RgbaImage>>, scale: f64) -> Option<String> {
        if self.store.visible_len() == 0 {
            return background.map(|_| PREVIEW_SOURCE.to_string());
        }
        let background = background?;
        let composed = compose_background_with_annotations(background.as_ref(), self.store.visible_items(), scale);
        CaptureService::save_temp(&composed)
    }

    #[cfg(any(feature = "overlay-diagnostics", test))]
    pub(crate) fn raster_diagnostics(&self) -> AnnotationRasterDiagnostics {
        self.raster_cache.diagnostics()
    }

    #[cfg(test)]
    pub(crate) fn raster_rebuilds(&self) -> (u64, u64) {
        (self.raster_cache.committed_rebuilds, self.raster_cache.composed_rebuilds)
    }

    fn layer_state(
        &mut self,
        selection: Option<RectF>,
        background: Option<&Arc<RgbaImage>>,
        scale: f64,
        preview_translate: Option<(f64, f64)>,
    ) -> AnnotationLayerState {
        let Some(selection) = selection else {
            return AnnotationLayerState::default();
        };

        let mut outlines = self.committed_outlines();
        let transient_item = self.transient_item();

        if let Some(item) = transient_item.as_ref() {
            outlines.retain(|outline| outline.id != item.id);
            outlines.push(AnnotationOutline {
                id: item.id,
                bounds: item.bounds(),
                selected: Some(item.id) == self.selected_id,
                transient: true,
            });
        }

        if let Some(editing) = &self.text_editing
            && let Some(item) = self.store.visible_item(editing.id)
            && let AnnotationKind::Text { origin, .. } = item.kind
        {
            let mut draft_item = item.clone();
            draft_item.kind = AnnotationKind::Text {
                origin,
                text: editing.draft.clone(),
            };
            outlines.retain(|outline| outline.id != editing.id);
            outlines.push(AnnotationOutline {
                id: editing.id,
                bounds: draft_item.bounds(),
                selected: Some(editing.id) == self.selected_id,
                transient: true,
            });
        }

        if let Some((dx, dy)) = preview_translate
            && (dx.abs() > f64::EPSILON || dy.abs() > f64::EPSILON)
        {
            for outline in &mut outlines {
                outline.bounds.x += dx;
                outline.bounds.y += dy;
            }
        }

        let image = self.layer_image(selection, background, scale, transient_item.as_ref(), preview_translate);

        AnnotationLayerState { image, outlines }
    }

    fn layer_image(
        &mut self,
        selection: RectF,
        background: Option<&Arc<RgbaImage>>,
        scale: f64,
        transient_item: Option<&AnnotationItem>,
        preview_translate: Option<(f64, f64)>,
    ) -> Option<Arc<RenderImage>> {
        if self.store.visible_len() == 0 && transient_item.is_none() {
            return None;
        }
        let background = background?;

        let preview_translate = preview_translate.filter(|(dx, dy)| dx.abs() > f64::EPSILON || dy.abs() > f64::EPSILON);
        let committed = self.ensure_committed_layer(selection, background, scale)?;
        let editing = self.text_editing.clone();
        let moving_id = match &self.interaction {
            AnnotationInteractionState::Moving { id, .. } => Some(*id),
            _ => None,
        };

        if transient_item.is_none() && editing.is_none() && preview_translate.is_none() {
            return Some(committed.image.clone());
        }

        if let Some(cache) = &self.raster_cache.composed
            && cache.selection == selection
            && (cache.scale - scale).abs() <= f64::EPSILON
            && cache.committed_revision == self.committed_revision
            && cache.transient_revision == self.transient_revision
            && cache.preview_translate == preview_translate
        {
            return Some(cache.image.clone());
        }

        if let Some((dx, dy)) = preview_translate
            && transient_item.is_none()
        {
            let mut items = self.store.clone_visible_items();
            if let Some(editing) = &editing
                && let Some(item) = items.iter_mut().find(|item| item.id == editing.id)
                && let AnnotationKind::Text { origin, .. } = item.kind
            {
                item.kind = AnnotationKind::Text {
                    origin,
                    text: editing.draft.clone(),
                };
            }

            for item in &mut items {
                item.move_by(dx, dy);
            }

            let image = self.render_items_on_base(committed.background_rgba.as_ref(), selection, scale, &items);
            self.cache_composed(selection, scale, preview_translate, image.clone());
            return Some(image);
        }

        if editing.is_none()
            && let Some(transient) = transient_item
        {
            if matches!(self.interaction, AnnotationInteractionState::Drawing { .. }) {
                self.raster_cache.drawing_fast_path_hits = self.raster_cache.drawing_fast_path_hits.saturating_add(1);
                let image = self.render_transient_on_base(committed.rgba.as_ref(), selection, scale, transient);
                self.cache_composed(selection, scale, None, image.clone());
                return Some(image);
            }
            if let Some(moving_id) = moving_id {
                let base = self.ensure_interaction_base_layer(selection, background, scale, moving_id)?;
                self.raster_cache.moving_fast_path_hits = self.raster_cache.moving_fast_path_hits.saturating_add(1);
                let image = self.render_transient_on_base(base.as_ref(), selection, scale, transient);
                self.cache_composed(selection, scale, None, image.clone());
                return Some(image);
            }
        }

        let mut items = self.store.clone_visible_items();
        if let Some(editing) = editing
            && let Some(item) = items.iter_mut().find(|item| item.id == editing.id)
            && let AnnotationKind::Text { origin, .. } = item.kind
        {
            item.kind = AnnotationKind::Text { origin, text: editing.draft };
        }

        if let Some(transient) = transient_item {
            if let Some(item) = items.iter_mut().find(|item| item.id == transient.id) {
                *item = transient.clone();
            } else {
                items.push(transient.clone());
            }
        }

        let layer = compose_selection_base(background.as_ref(), selection, &items, scale)?;
        let image = render_image::from_rgba(layer);
        self.cache_composed(selection, scale, None, image.clone());
        Some(image)
    }

    fn ensure_committed_layer(&mut self, selection: RectF, background: &Arc<RgbaImage>, scale: f64) -> Option<CommittedLayerCache> {
        let cache_valid = self.raster_cache.committed.as_ref().is_some_and(|cache| {
            cache.selection == selection && (cache.scale - scale).abs() <= f64::EPSILON && cache.revision == self.committed_revision
        });
        if cache_valid {
            return self.raster_cache.committed.clone();
        }

        let background_rgba = compose_selection_background(background.as_ref(), selection, scale)?;
        let mut layer = background_rgba.clone();
        draw_items_on_selection(&mut layer, selection, self.store.visible_items(), scale);
        let image = render_image::from_rgba(layer.clone());
        self.raster_cache.committed_rebuilds = self.raster_cache.committed_rebuilds.saturating_add(1);
        self.raster_cache.committed = Some(CommittedLayerCache {
            selection,
            scale,
            revision: self.committed_revision,
            background_rgba: Arc::new(background_rgba),
            rgba: Arc::new(layer),
            image,
        });
        self.raster_cache.committed.clone()
    }

    fn ensure_interaction_base_layer(&mut self, selection: RectF, background: &Arc<RgbaImage>, scale: f64, moving_id: u64) -> Option<Arc<RgbaImage>> {
        let cache_valid = self.raster_cache.interaction_base.as_ref().is_some_and(|cache| {
            cache.selection == selection
                && (cache.scale - scale).abs() <= f64::EPSILON
                && cache.committed_revision == self.committed_revision
                && cache.moving_id == moving_id
        });
        if cache_valid {
            return self.raster_cache.interaction_base.as_ref().map(|cache| cache.rgba.clone());
        }

        let items: Vec<AnnotationItem> = self.store.visible_items().iter().filter(|item| item.id != moving_id).cloned().collect();
        let layer = compose_selection_base(background.as_ref(), selection, &items, scale)?;
        self.raster_cache.interaction_base_rebuilds = self.raster_cache.interaction_base_rebuilds.saturating_add(1);
        self.raster_cache.interaction_base = Some(InteractionBaseCache {
            selection,
            scale,
            committed_revision: self.committed_revision,
            moving_id,
            rgba: Arc::new(layer),
        });
        self.raster_cache.interaction_base.as_ref().map(|cache| cache.rgba.clone())
    }

    fn prepare_scratch_layer<'a>(&'a mut self, base: &RgbaImage) -> &'a mut RgbaImage {
        let base_dimensions = base.dimensions();
        let scratch = self.raster_cache.scratch_layer.get_or_insert_with(|| base.clone());
        if scratch.dimensions() != base_dimensions {
            *scratch = base.clone();
        } else {
            scratch
                .copy_from(base, 0, 0)
                .expect("scratch dimensions were validated to match base dimensions");
        }
        scratch
    }

    fn render_transient_on_base(&mut self, base: &RgbaImage, selection: RectF, scale: f64, transient: &AnnotationItem) -> Arc<RenderImage> {
        let scratch = self.prepare_scratch_layer(base);
        draw_items_on_selection(scratch, selection, std::slice::from_ref(transient), scale);
        render_image::from_rgba(scratch.clone())
    }

    fn render_items_on_base(&mut self, base: &RgbaImage, selection: RectF, scale: f64, items: &[AnnotationItem]) -> Arc<RenderImage> {
        let scratch = self.prepare_scratch_layer(base);
        draw_items_on_selection(scratch, selection, items, scale);
        render_image::from_rgba(scratch.clone())
    }

    fn cache_composed(&mut self, selection: RectF, scale: f64, preview_translate: Option<(f64, f64)>, image: Arc<RenderImage>) {
        self.raster_cache.composed_rebuilds = self.raster_cache.composed_rebuilds.saturating_add(1);
        self.raster_cache.composed = Some(ComposedLayerCache {
            selection,
            scale,
            committed_revision: self.committed_revision,
            transient_revision: self.transient_revision,
            preview_translate,
            image,
        });
    }

    fn committed_outlines(&self) -> Vec<AnnotationOutline> {
        let mut proxy = self.hit_proxy.borrow_mut();
        proxy.sync(self.committed_revision, self.store.visible_items());
        proxy.outlines(self.selected_id)
    }

    fn transient_item(&self) -> Option<AnnotationItem> {
        match &self.interaction {
            AnnotationInteractionState::Drawing { tool, start, current, style } => {
                let mut preview = build_drawing_item(*tool, *start, *current, *style, self.next_id)?;
                preview.style.stroke_color = preview.style.stroke_color & 0xffffff00 | 0xcc;
                preview.style.fill_color = preview.style.fill_color & 0xffffff00 | 0x88;
                Some(preview)
            }
            AnnotationInteractionState::Moving { id, start, current, origin } => {
                let source = self.store.visible_item(*id)?;
                let mut item = source.clone();
                item.kind = origin.clone();
                let dx = current.0 - start.0;
                let dy = current.1 - start.1;
                item.move_by(dx, dy);
                Some(item)
            }
            AnnotationInteractionState::Idle => None,
        }
    }

    fn selected_summary(&self) -> Option<AnnotationSelectionInfo> {
        let item = self.selected_item()?;
        let mosaic_mode = match &item.kind {
            AnnotationKind::Mosaic { mode, .. } => Some(*mode),
            _ => None,
        };
        Some(AnnotationSelectionInfo {
            id: item.id,
            kind: kind_tag(item),
            metric: item.primary_metric(),
            mosaic_mode,
        })
    }
}

fn kind_tag(item: &AnnotationItem) -> AnnotationKindTag {
    item.kind.tag()
}

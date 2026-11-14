use image::RgbaImage;
use std::sync::{Arc, Weak};

use minnow_imaging::geometry::RectF;
use minnow_imaging::{GeometryError, ResolvedSelection, ViewRect};

use super::super::model::{
    AnnotationInteractionState, AnnotationItem, AnnotationKind, AnnotationKindTag, AnnotationLayerState, AnnotationOutline, AnnotationSelectionInfo,
    AnnotationUiState,
};
use super::super::ops::{build_drawing_item, contains_point_with_bounds, finite_point, positive_finite};
use super::super::raster::{compose_background_with_annotations, compose_selection_base, draw_items_on_selection};
use super::super::raster_cache::AnnotationRasterDiagnostics;
use super::super::raster_cache::{CommittedLayerCache, ComposedLayerCache, InteractionBaseCache};
use super::AnnotationEngine;

struct AnnotationPreview {
    transient: Option<AnnotationItem>,
    text_draft: Option<AnnotationItem>,
    translated: Option<Vec<AnnotationItem>>,
    translation: Option<(f64, f64)>,
}

impl AnnotationEngine {
    pub fn hit_test(&self, point: (f64, f64), selection: Option<RectF>, idle_mode: bool) -> Option<u64> {
        if !self.mode_enabled(selection, idle_mode) || !self.point_in_selection(point, selection) {
            return None;
        }
        let moving_item = if matches!(self.interaction, AnnotationInteractionState::Moving { .. }) {
            self.transient_item().ok()?
        } else {
            None
        };
        let mut proxy = self.hit_proxy.borrow_mut();
        proxy.sync(self.committed_revision, self.store.visible_items(), self.text_font.as_ref());
        proxy.hit_test(point, moving_item.as_ref(), self.text_font.as_ref(), |id, bounds, _| {
            moving_item
                .as_ref()
                .filter(|item| item.id == id)
                .or_else(|| self.store.visible_item(id))
                .is_some_and(|item| contains_point_with_bounds(item, point, bounds))
        })
    }

    pub fn ui_state(
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

    /// Composes committed annotations without publishing a temporary file.
    /// An unmodified document returns the original shared pixel allocation.
    pub fn composed_background_image(&self, background: Option<&Arc<RgbaImage>>, scale: f64) -> Option<Arc<RgbaImage>> {
        if !positive_finite(scale) {
            return None;
        }
        let background = background?;
        if self.store.visible_len() == 0 {
            return Some(background.clone());
        }
        Some(Arc::new(compose_background_with_annotations(
            background.as_ref(),
            self.store.visible_items(),
            scale,
            self.text_font.as_ref(),
        )))
    }

    pub fn raster_diagnostics(&self) -> AnnotationRasterDiagnostics {
        self.raster_cache.diagnostics()
    }

    #[cfg(test)]
    pub fn raster_rebuilds(&self) -> (u64, u64) {
        (self.raster_cache.committed_rebuilds, self.raster_cache.composed_rebuilds)
    }

    fn layer_state(
        &mut self,
        selection: Option<RectF>,
        background: Option<&Arc<RgbaImage>>,
        scale: f64,
        preview_translate: Option<(f64, f64)>,
    ) -> AnnotationLayerState {
        let preview = match self.preview_items(preview_translate) {
            Ok(preview) => preview,
            Err(error) => {
                return AnnotationLayerState {
                    raster_error: Some(error),
                    ..AnnotationLayerState::default()
                };
            }
        };
        let Some(selection) = selection else {
            return AnnotationLayerState::default();
        };
        let committed_outlines = self.committed_outlines();
        let outlines = self
            .effective_items(&preview)
            .enumerate()
            .map(|(index, item)| {
                let transient = preview.transient.as_ref().is_some_and(|draft| draft.id == item.id)
                    || preview.text_draft.as_ref().is_some_and(|draft| draft.id == item.id);
                let bounds = if let Some(items) = &preview.translated {
                    items[index].bounds_with_font(self.text_font.as_ref())
                } else if transient {
                    item.bounds_with_font(self.text_font.as_ref())
                } else {
                    committed_outlines[index].bounds
                };
                AnnotationOutline {
                    id: item.id,
                    bounds,
                    selected: Some(item.id) == self.selected_id,
                    transient,
                }
            })
            .collect();

        let (resolved, raster_error) = match background.map(|image| ResolvedSelection::resolve(ViewRect(selection), scale, image.dimensions())) {
            Some(Ok(resolved)) => (Some(resolved), None),
            Some(Err(error)) => (None, Some(error)),
            None => (None, None),
        };
        let image = resolved
            .as_ref()
            .and_then(|resolved| self.layer_image(resolved, background, scale, &preview));
        let image_bounds = image.as_ref().and(resolved.as_ref()).map(|resolved| resolved.view_rect().0);

        AnnotationLayerState {
            image,
            image_bounds,
            raster_error,
            outlines,
        }
    }

    fn layer_image(
        &mut self,
        resolved: &ResolvedSelection,
        background: Option<&Arc<RgbaImage>>,
        scale: f64,
        preview: &AnnotationPreview,
    ) -> Option<Arc<RgbaImage>> {
        let transient_item = preview.transient.as_ref();
        let preview_translate = preview.translation;
        if self.store.visible_len() == 0 && transient_item.is_none() {
            self.raster_cache.invalidate_layers();
            return None;
        }
        let background = background?;
        let selection = resolved.view_rect().0;
        let pixels = resolved.pixel_rect();
        let bytes = u64::from(pixels.width).saturating_mul(u64::from(pixels.height)).saturating_mul(4);
        let cacheable = bytes <= self.raster_cache.budget.max_cached_bytes as u64;
        if !cacheable {
            self.raster_cache.invalidate_layers();
            self.raster_cache.uncached_renders = self.raster_cache.uncached_renders.saturating_add(1);
        }
        if transient_item.is_none() && preview.text_draft.is_none() && preview_translate.is_none() {
            return Some(self.ensure_committed_layer(resolved, background, scale, cacheable));
        }
        if let Some(cache) = &self.raster_cache.composed
            && cache.selection == selection
            && (cache.scale - scale).abs() <= f64::EPSILON
            && cache.committed_revision == self.committed_revision
            && cache.transient_revision == self.transient_revision
            && cache.preview_translate == preview_translate
            && Weak::ptr_eq(&cache.source, &Arc::downgrade(background))
        {
            return Some(cache.image.clone());
        }
        if cacheable
            && preview.text_draft.is_none()
            && preview_translate.is_none()
            && let Some(transient) = transient_item
        {
            let base = match &self.interaction {
                AnnotationInteractionState::Drawing { .. } => {
                    self.raster_cache.drawing_fast_path_hits = self.raster_cache.drawing_fast_path_hits.saturating_add(1);
                    Some(self.ensure_committed_layer(resolved, background, scale, true))
                }
                // Drawing a moved item last is valid only for the topmost item.
                // Other moves must preserve the document's original stacking order.
                AnnotationInteractionState::Moving { id, .. } if self.store.visible_items().last().is_some_and(|item| item.id == *id) => {
                    let id = *id;
                    self.raster_cache.moving_fast_path_hits = self.raster_cache.moving_fast_path_hits.saturating_add(1);
                    Some(self.ensure_interaction_base_layer(resolved, background, scale, id))
                }
                _ => None,
            };
            if let Some(base) = base {
                let mut layer = base.as_ref().clone();
                draw_items_on_selection(&mut layer, resolved, std::slice::from_ref(transient), scale, self.text_font.as_ref());
                let image = Arc::new(layer);
                self.cache_composed(selection, background, scale, preview_translate, image.clone());
                return Some(image);
            }
        }
        let untransformed;
        let items = if let Some(items) = &preview.translated {
            items.as_slice()
        } else {
            untransformed = self.effective_items(preview).cloned().collect::<Vec<_>>();
            &untransformed
        };
        let image = Arc::new(compose_selection_base(
            background.as_ref(),
            resolved,
            items,
            scale,
            self.text_font.as_ref(),
        ));
        if cacheable {
            self.cache_composed(selection, background, scale, preview_translate, image.clone());
        }
        Some(image)
    }

    fn ensure_committed_layer(&mut self, resolved: &ResolvedSelection, background: &Arc<RgbaImage>, scale: f64, cacheable: bool) -> Arc<RgbaImage> {
        let selection = resolved.view_rect().0;
        if let Some(cache) = &self.raster_cache.committed
            && cache.selection == selection
            && (cache.scale - scale).abs() <= f64::EPSILON
            && cache.committed_revision == self.committed_revision
            && Weak::ptr_eq(&cache.source, &Arc::downgrade(background))
        {
            return cache.image.clone();
        }
        self.raster_cache.invalidate_composed();
        self.raster_cache.invalidate_interaction_base();
        let image = Arc::new(compose_selection_base(
            background.as_ref(),
            resolved,
            self.store.visible_items(),
            scale,
            self.text_font.as_ref(),
        ));
        self.raster_cache.committed_rebuilds = self.raster_cache.committed_rebuilds.saturating_add(1);
        if cacheable {
            self.raster_cache.committed = Some(CommittedLayerCache {
                selection,
                scale,
                committed_revision: self.committed_revision,
                source: Arc::downgrade(background),
                image: image.clone(),
            });
            self.raster_cache.enforce_budget();
        }
        image
    }

    fn ensure_interaction_base_layer(
        &mut self,
        resolved: &ResolvedSelection,
        background: &Arc<RgbaImage>,
        scale: f64,
        moving_id: u64,
    ) -> Arc<RgbaImage> {
        let selection = resolved.view_rect().0;
        if let Some(cache) = &self.raster_cache.interaction_base
            && cache.selection == selection
            && (cache.scale - scale).abs() <= f64::EPSILON
            && cache.committed_revision == self.committed_revision
            && cache.moving_id == moving_id
            && Weak::ptr_eq(&cache.source, &Arc::downgrade(background))
        {
            return cache.rgba.clone();
        }
        let items: Vec<AnnotationItem> = self.store.visible_items().iter().filter(|item| item.id != moving_id).cloned().collect();
        let image = Arc::new(compose_selection_base(
            background.as_ref(),
            resolved,
            &items,
            scale,
            self.text_font.as_ref(),
        ));
        self.raster_cache.interaction_base_rebuilds = self.raster_cache.interaction_base_rebuilds.saturating_add(1);
        self.raster_cache.interaction_base = Some(InteractionBaseCache {
            selection,
            scale,
            committed_revision: self.committed_revision,
            moving_id,
            source: Arc::downgrade(background),
            rgba: image.clone(),
        });
        self.raster_cache.enforce_budget();
        image
    }

    fn cache_composed(
        &mut self,
        selection: RectF,
        source: &Arc<RgbaImage>,
        scale: f64,
        preview_translate: Option<(f64, f64)>,
        image: Arc<RgbaImage>,
    ) {
        self.raster_cache.composed_rebuilds = self.raster_cache.composed_rebuilds.saturating_add(1);
        self.raster_cache.composed = Some(ComposedLayerCache {
            selection,
            source: Arc::downgrade(source),
            scale,
            committed_revision: self.committed_revision,
            transient_revision: self.transient_revision,
            preview_translate,
            image,
        });
        self.raster_cache.enforce_budget();
    }

    fn committed_outlines(&self) -> Vec<AnnotationOutline> {
        let mut proxy = self.hit_proxy.borrow_mut();
        proxy.sync(self.committed_revision, self.store.visible_items(), self.text_font.as_ref());
        proxy.outlines(self.selected_id)
    }

    /// One logical view supplies both outlines and pixels. Only the transient
    /// and text draft are owned during ordinary pointer updates; a whole-document
    /// projection is allocated after every effective item passes its preflight.
    fn preview_items(&self, translation: Option<(f64, f64)>) -> Result<AnnotationPreview, GeometryError> {
        if translation.is_some_and(|delta| !finite_point(delta)) {
            return Err(GeometryError::InvalidTransform);
        }
        let transient = self.transient_item()?;
        let text_draft = self.text_editing.as_ref().and_then(|editing| {
            let mut item = transient
                .as_ref()
                .filter(|item| item.id == editing.id)
                .or_else(|| self.store.visible_item(editing.id))?
                .clone();
            let AnnotationKind::Text { text, .. } = &mut item.kind else {
                return None;
            };
            *text = editing.draft.clone();
            Some(item)
        });
        let mut preview = AnnotationPreview {
            transient,
            text_draft,
            translated: None,
            translation: translation.filter(|(dx, dy)| dx.abs() > f64::EPSILON || dy.abs() > f64::EPSILON),
        };
        if let Some((dx, dy)) = preview.translation {
            if !self.effective_items(&preview).all(|item| item.can_move_by(dx, dy)) {
                return Err(GeometryError::InvalidTransform);
            }
            let mut items: Vec<_> = self.effective_items(&preview).cloned().collect();
            for item in &mut items {
                // The complete projection has already passed the same check
                // used by this mutator, before any result is published.
                item.move_by(dx, dy);
            }
            preview.translated = Some(items);
        }
        Ok(preview)
    }

    fn effective_items<'a>(&'a self, preview: &'a AnnotationPreview) -> impl Iterator<Item = &'a AnnotationItem> {
        self.store
            .visible_items()
            .iter()
            .map(|item| {
                preview
                    .text_draft
                    .as_ref()
                    .filter(|draft| draft.id == item.id)
                    .or_else(|| preview.transient.as_ref().filter(|draft| draft.id == item.id))
                    .unwrap_or(item)
            })
            .chain(preview.transient.iter().filter(|item| !self.store.visible_contains(item.id)))
    }

    fn transient_item(&self) -> Result<Option<AnnotationItem>, GeometryError> {
        match &self.interaction {
            AnnotationInteractionState::Drawing { tool, start, current, style } => {
                let mut preview = build_drawing_item(*tool, *start, *current, *style, self.next_id).ok_or(GeometryError::InvalidTransform)?;
                preview.style.stroke_color = preview.style.stroke_color & 0xffffff00 | 0xcc;
                preview.style.fill_color = preview.style.fill_color & 0xffffff00 | 0x88;
                Ok(Some(preview))
            }
            AnnotationInteractionState::Moving { id, start, current } => {
                // The gesture owns only displacement. Document edits (including
                // Mosaic's kind fields) immediately become its live baseline,
                // exactly as they do when finish_interaction commits the move.
                let Some(source) = self.store.visible_item(*id) else {
                    return Ok(None);
                };
                let mut item = source.clone();
                let dx = current.0 - start.0;
                let dy = current.1 - start.1;
                if !item.can_move_by(dx, dy) {
                    return Err(GeometryError::InvalidTransform);
                }
                item.move_by(dx, dy);
                Ok(Some(item))
            }
            AnnotationInteractionState::Idle => Ok(None),
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

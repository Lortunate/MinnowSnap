use gpui::RenderImage;
use image::{GenericImage, RgbaImage};
use std::cell::RefCell;
use std::sync::Arc;

use minnow_core::capture::service::CaptureService;
use minnow_core::capture::source::PREVIEW_SOURCE;
use minnow_core::geometry::RectF;

use super::hit_test::AnnotationHitProxy;
use super::model::{
    AnnotationInteractionState, AnnotationItem, AnnotationKind, AnnotationKindTag, AnnotationLayerState, AnnotationOutline, AnnotationSelectionInfo,
    AnnotationStyleState, AnnotationTool, AnnotationUiState, COLOR_PRESETS, MosaicMode, TEXT_DEFAULT, TextEditState,
};
use super::ops::{annotation_item_large_enough, build_drawing_item, ensure_mosaic_kind_style, sync_style_from_item};
use super::raster::{compose_background_with_annotations, compose_selection_base, draw_items_on_selection};
#[cfg(any(feature = "overlay-diagnostics", test))]
use super::raster_cache::AnnotationRasterDiagnostics;
use super::raster_cache::{AnnotationRasterCache, CommittedLayerCache, ComposedLayerCache, InteractionBaseCache};
use super::store::AnnotationStore;
use crate::support::render_image;

#[derive(Clone, Debug)]
pub(crate) struct AnnotationEngine {
    store: AnnotationStore,
    tool: Option<AnnotationTool>,
    style: AnnotationStyleState,
    selected_id: Option<u64>,
    interaction: AnnotationInteractionState,
    text_editing: Option<TextEditState>,
    next_id: u64,
    next_counter: u32,
    committed_revision: u64,
    transient_revision: u64,
    hit_proxy: RefCell<AnnotationHitProxy>,
    raster_cache: AnnotationRasterCache,
}

impl Default for AnnotationEngine {
    fn default() -> Self {
        Self {
            store: AnnotationStore::default(),
            tool: None,
            style: AnnotationStyleState::default(),
            selected_id: None,
            interaction: AnnotationInteractionState::Idle,
            text_editing: None,
            next_id: 1,
            next_counter: 1,
            committed_revision: 1,
            transient_revision: 1,
            hit_proxy: RefCell::new(AnnotationHitProxy::default()),
            raster_cache: AnnotationRasterCache::default(),
        }
    }
}

impl AnnotationEngine {
    pub(crate) fn clear(&mut self) {
        self.store.clear();
        self.tool = None;
        self.style = AnnotationStyleState::default();
        self.selected_id = None;
        self.interaction = AnnotationInteractionState::Idle;
        self.text_editing = None;
        self.next_id = 1;
        self.next_counter = 1;
        self.committed_revision = 1;
        self.transient_revision = 1;
        self.hit_proxy.borrow_mut().clear();
        self.raster_cache.clear();
    }

    pub(crate) fn cancel_interaction_state(&mut self) {
        let had_interaction = !matches!(self.interaction, AnnotationInteractionState::Idle);
        let had_text_edit = self.text_editing.is_some();
        self.interaction = AnnotationInteractionState::Idle;
        self.text_editing = None;
        if had_interaction || had_text_edit {
            self.bump_transient();
        }
    }

    pub(crate) fn selected_item(&self) -> Option<&AnnotationItem> {
        let id = self.selected_id?;
        self.store.visible_item(id)
    }

    pub(crate) fn kind_for(&self, id: u64) -> Option<AnnotationKind> {
        Some(self.store.visible_item(id)?.kind.clone())
    }

    pub(crate) fn text_editing_id(&self) -> Option<u64> {
        self.text_editing.as_ref().map(|state| state.id)
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.store.can_undo()
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.store.can_redo()
    }

    pub(crate) fn has_active_interaction(&self) -> bool {
        !matches!(self.interaction, AnnotationInteractionState::Idle)
    }

    pub(crate) fn has_active_tool(&self) -> bool {
        self.tool.is_some()
    }

    pub(crate) fn set_tool(&mut self, tool: AnnotationTool) -> bool {
        self.tool = if self.tool == Some(tool) { None } else { Some(tool) };
        true
    }

    pub(crate) fn cycle_color(&mut self) -> bool {
        let current = self.style.stroke_color;
        let idx = COLOR_PRESETS.iter().position(|value| *value == current).unwrap_or(0);
        let next = COLOR_PRESETS[(idx + 1) % COLOR_PRESETS.len()];
        self.set_color(next)
    }

    pub(crate) fn set_color(&mut self, color: u32) -> bool {
        let color_rgb = color & 0xffffff00;
        let mut changed_style = false;
        if self.style.stroke_color != color {
            self.style.stroke_color = color;
            changed_style = true;
        }
        let style_fill = color_rgb | (self.style.fill_color & 0x000000ff);
        if self.style.fill_color != style_fill {
            self.style.fill_color = style_fill;
            changed_style = true;
        }

        let mut changed_item = false;
        if let Some(item) = self.selected_item_mut() {
            if item.style.stroke_color != color {
                item.style.stroke_color = color;
                changed_item = true;
            }
            let item_fill = color_rgb | (item.style.fill_color & 0x000000ff);
            if item.style.fill_color != item_fill {
                item.style.fill_color = item_fill;
                changed_item = true;
            }
        }

        if !changed_style && !changed_item {
            return false;
        }
        if changed_item {
            self.bump_committed();
        } else {
            self.bump_transient();
        }
        true
    }

    pub(crate) fn toggle_fill(&mut self) -> bool {
        let next_fill_enabled = !self.style.fill_enabled;
        let style_changed = self.style.fill_enabled != next_fill_enabled;
        self.style.fill_enabled = next_fill_enabled;
        let mut changed_item = false;
        if let Some(item) = self.selected_item_mut()
            && item.style.fill_enabled != next_fill_enabled
        {
            item.style.fill_enabled = next_fill_enabled;
            changed_item = true;
        }
        if changed_item {
            self.bump_committed();
        } else {
            self.bump_transient();
        }
        changed_item || style_changed
    }

    pub(crate) fn adjust_stroke(&mut self, delta: f64) -> bool {
        let next_stroke = (self.style.stroke_width + delta).clamp(1.0, 18.0);
        let next_text = (self.style.text_size + delta).clamp(12.0, 96.0);
        let next_counter = (self.style.counter_radius + delta * 4.0).clamp(10.0, 64.0);
        let style_changed = (self.style.stroke_width - next_stroke).abs() > f64::EPSILON
            || (self.style.text_size - next_text).abs() > f64::EPSILON
            || (self.style.counter_radius - next_counter).abs() > f64::EPSILON;
        self.style.stroke_width = next_stroke;
        self.style.text_size = next_text;
        self.style.counter_radius = next_counter;

        let had_selected = self.selected_id.is_some();
        let mut changed_item = false;
        if let Some(item) = self.selected_item_mut() {
            changed_item = item.resize_by_wheel(delta);
        }
        if changed_item {
            self.sync_style_from_selected();
            self.bump_committed();
            return true;
        }
        if had_selected {
            self.sync_style_from_selected();
            return false;
        }
        if style_changed {
            self.bump_transient();
            return true;
        }
        false
    }

    pub(crate) fn set_mosaic_mode(&mut self, mode: MosaicMode) -> bool {
        let style_changed = self.style.mosaic_mode != mode;
        self.style.mosaic_mode = mode;
        let had_selected = self.selected_id.is_some();
        let mut changed = false;
        let style = self.style;
        if let Some(item) = self.selected_item_mut() {
            let before = item.kind.clone();
            ensure_mosaic_kind_style(&mut item.kind, &style);
            changed = before != item.kind;
        }
        if changed {
            self.sync_style_from_selected();
            self.bump_committed();
            return true;
        }
        if had_selected {
            self.sync_style_from_selected();
            return false;
        }
        if style_changed {
            self.bump_transient();
            return true;
        }
        false
    }

    pub(crate) fn adjust_mosaic_intensity(&mut self, delta: f64) -> bool {
        let next_intensity = (self.style.mosaic_intensity + delta).clamp(2.0, 64.0);
        let style_changed = (self.style.mosaic_intensity - next_intensity).abs() > f64::EPSILON;
        self.style.mosaic_intensity = next_intensity;
        let had_selected = self.selected_id.is_some();
        let mut changed = false;
        if let Some(item) = self.selected_item_mut()
            && let AnnotationKind::Mosaic { intensity, .. } = &mut item.kind
            && (*intensity - next_intensity).abs() > f64::EPSILON
        {
            *intensity = next_intensity;
            item.style.mosaic_intensity = *intensity;
            changed = true;
        }
        if changed {
            self.sync_style_from_selected();
            self.bump_committed();
            return true;
        }
        if had_selected {
            self.sync_style_from_selected();
            return false;
        }
        if style_changed {
            self.bump_transient();
            return true;
        }
        false
    }

    pub(crate) fn adjust_selected_by_wheel(&mut self, point: (f64, f64), delta_steps: f64, selection: Option<RectF>, idle_mode: bool) -> bool {
        if !self.mode_enabled(selection, idle_mode) {
            return false;
        }
        let Some(selected_id) = self.selected_id else {
            return false;
        };
        let hit = self.hit_test(point, selection, idle_mode);
        if hit != Some(selected_id) {
            return false;
        }
        let Some(item) = self.selected_item_mut() else {
            return false;
        };
        let changed = item.resize_by_wheel(delta_steps);
        if changed {
            self.sync_style_from_selected();
            self.bump_committed();
        }
        changed
    }

    pub(crate) fn select(&mut self, id: Option<u64>) -> bool {
        let next = id.filter(|item_id| self.store.visible_contains(*item_id));
        let changed = self.selected_id != next;
        self.selected_id = next;
        if self.text_editing.as_ref().is_some_and(|state| Some(state.id) != self.selected_id) {
            self.text_editing = None;
            self.bump_transient();
        }
        self.sync_style_from_selected();
        changed
    }

    pub(crate) fn begin_text_edit_selected(&mut self) -> bool {
        let Some(id) = self.selected_id else {
            return false;
        };
        let Some(item) = self.store.visible_item(id) else {
            return false;
        };
        let AnnotationKind::Text { text, .. } = &item.kind else {
            return false;
        };
        self.text_editing = Some(TextEditState { id, draft: text.clone() });
        self.bump_transient();
        true
    }

    pub(crate) fn append_text_edit(&mut self, text: &str) -> bool {
        let Some(edit) = &mut self.text_editing else {
            return false;
        };
        edit.draft.push_str(text);
        self.bump_transient();
        true
    }

    pub(crate) fn backspace_text_edit(&mut self) -> bool {
        let Some(edit) = &mut self.text_editing else {
            return false;
        };
        let changed = edit.draft.pop().is_some();
        if changed {
            self.bump_transient();
        }
        changed
    }

    pub(crate) fn insert_newline_text_edit(&mut self) -> bool {
        self.append_text_edit("\n")
    }

    pub(crate) fn commit_text_edit(&mut self) -> bool {
        let Some(edit) = self.text_editing.take() else {
            return false;
        };
        let Some(item) = self.store.visible_item_mut(edit.id) else {
            return false;
        };
        let AnnotationKind::Text { text: value, .. } = &mut item.kind else {
            return false;
        };
        let next = edit.draft.trim_end_matches('\n').to_string();
        if next.trim().is_empty() {
            *value = TEXT_DEFAULT.to_string();
        } else {
            *value = next;
        }
        self.bump_committed();
        true
    }

    pub(crate) fn cancel_text_edit(&mut self) -> bool {
        let had = self.text_editing.is_some();
        self.text_editing = None;
        if had {
            self.bump_transient();
        }
        had
    }

    pub(crate) fn start_draw(&mut self, point: (f64, f64), selection: Option<RectF>, idle_mode: bool) -> bool {
        if !self.mode_enabled(selection, idle_mode) {
            return false;
        }
        let Some(tool) = self.tool else {
            return false;
        };
        let point = self.clamp_to_selection(point, selection);
        if !self.point_in_selection(point, selection) {
            return false;
        }
        self.text_editing = None;

        match tool {
            AnnotationTool::Counter => {
                let id = self.consume_id();
                let number = self.next_counter.max(1);
                self.next_counter = number.saturating_add(1);
                let item = AnnotationItem {
                    id,
                    style: self.style,
                    kind: AnnotationKind::Counter { center: point, number },
                };
                self.commit_item(item);
                true
            }
            AnnotationTool::Text => {
                let id = self.consume_id();
                let item = AnnotationItem {
                    id,
                    style: self.style,
                    kind: AnnotationKind::Text {
                        origin: point,
                        text: TEXT_DEFAULT.to_string(),
                    },
                };
                self.commit_item(item);
                self.text_editing = Some(TextEditState {
                    id,
                    draft: TEXT_DEFAULT.to_string(),
                });
                self.bump_transient();
                true
            }
            tool => {
                self.interaction = AnnotationInteractionState::Drawing {
                    tool,
                    start: point,
                    current: point,
                    style: self.style,
                };
                self.bump_transient();
                true
            }
        }
    }

    pub(crate) fn start_move(&mut self, id: u64, point: (f64, f64), selection: Option<RectF>, idle_mode: bool) -> bool {
        if !self.mode_enabled(selection, idle_mode) {
            return false;
        }
        let Some(item) = self.store.visible_item(id) else {
            return false;
        };
        let anchor = item.kind.clone();
        let point = self.clamp_to_selection(point, selection);
        self.selected_id = Some(id);
        self.text_editing = None;
        self.interaction = AnnotationInteractionState::Moving {
            id,
            start: point,
            current: point,
            origin: anchor,
        };
        self.sync_style_from_selected();
        self.bump_transient();
        true
    }

    pub(crate) fn update_interaction(&mut self, point: (f64, f64), selection: Option<RectF>) -> bool {
        let interaction = std::mem::take(&mut self.interaction);
        match interaction {
            AnnotationInteractionState::Idle => {
                self.interaction = AnnotationInteractionState::Idle;
                false
            }
            AnnotationInteractionState::Drawing { tool, start, current, style } => {
                let next = self.clamp_to_selection(point, selection);
                let changed = current != next;
                self.interaction = AnnotationInteractionState::Drawing {
                    tool,
                    start,
                    current: next,
                    style,
                };
                if changed {
                    self.bump_transient();
                }
                changed
            }
            AnnotationInteractionState::Moving { id, start, current, origin } => {
                let next = self.clamp_to_selection(point, selection);
                let changed = current != next;
                self.interaction = AnnotationInteractionState::Moving {
                    id,
                    start,
                    current: next,
                    origin,
                };
                if changed {
                    self.bump_transient();
                }
                changed
            }
        }
    }

    pub(crate) fn finish_interaction(&mut self, min_selection_size: f64) -> bool {
        match std::mem::take(&mut self.interaction) {
            AnnotationInteractionState::Idle => false,
            AnnotationInteractionState::Drawing { tool, start, current, style } => {
                let id = self.consume_id();
                if let Some(item) = build_drawing_item(tool, start, current, style, id)
                    && annotation_item_large_enough(&item, min_selection_size)
                {
                    self.commit_item(item);
                    return true;
                }
                self.bump_transient();
                false
            }
            AnnotationInteractionState::Moving { id, start, current, .. } => {
                let dx = current.0 - start.0;
                let dy = current.1 - start.1;
                if dx.abs() <= f64::EPSILON && dy.abs() <= f64::EPSILON {
                    self.bump_transient();
                    return false;
                }
                if let Some(item) = self.store.visible_item_mut(id) {
                    item.move_by(dx, dy);
                }
                self.bump_committed();
                true
            }
        }
    }

    pub(crate) fn delete_selected(&mut self) -> bool {
        let Some(id) = self.selected_id else {
            return false;
        };
        if !self.store.remove_visible_by_id(id) {
            return false;
        }
        self.selected_id = None;
        self.text_editing = None;
        self.bump_committed();
        true
    }

    pub(crate) fn undo(&mut self) -> bool {
        if !self.store.undo() {
            return false;
        }
        self.selected_id = self.selected_id.filter(|id| self.store.visible_contains(*id));
        if self.text_editing.as_ref().is_some_and(|state| !self.store.visible_contains(state.id)) {
            self.text_editing = None;
        }
        self.bump_committed();
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        if !self.store.redo() {
            return false;
        }
        self.bump_committed();
        true
    }
}

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
                .is_some_and(|item| super::ops::contains_point_with_bounds(item, point, bounds))
        })
    }

    pub(crate) fn ui_state(&mut self, selection: Option<RectF>, background: Option<&Arc<RgbaImage>>, scale: f64) -> AnnotationUiState {
        let layer = self.layer_state(selection, background, scale);
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
}

impl AnnotationEngine {
    fn layer_state(&mut self, selection: Option<RectF>, background: Option<&Arc<RgbaImage>>, scale: f64) -> AnnotationLayerState {
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

        let image = self.layer_image(selection, background, scale, transient_item.as_ref());

        AnnotationLayerState { image, outlines }
    }

    fn layer_image(
        &mut self,
        selection: RectF,
        background: Option<&Arc<RgbaImage>>,
        scale: f64,
        transient_item: Option<&AnnotationItem>,
    ) -> Option<Arc<RenderImage>> {
        if self.store.visible_len() == 0 && transient_item.is_none() {
            return None;
        }
        let background = background?;

        let committed = self.ensure_committed_layer(selection, background, scale)?;
        let editing = self.text_editing.clone();
        let moving_id = match &self.interaction {
            AnnotationInteractionState::Moving { id, .. } => Some(*id),
            _ => None,
        };

        if transient_item.is_none() && editing.is_none() {
            return Some(committed.image.clone());
        }

        if let Some(cache) = &self.raster_cache.composed
            && cache.selection == selection
            && (cache.scale - scale).abs() <= f64::EPSILON
            && cache.committed_revision == self.committed_revision
            && cache.transient_revision == self.transient_revision
        {
            return Some(cache.image.clone());
        }

        if editing.is_none()
            && let Some(transient) = transient_item
        {
            if matches!(self.interaction, AnnotationInteractionState::Drawing { .. }) {
                self.raster_cache.drawing_fast_path_hits = self.raster_cache.drawing_fast_path_hits.saturating_add(1);
                let image = self.render_transient_on_base(committed.rgba.as_ref(), selection, scale, transient);
                self.cache_composed(selection, scale, image.clone());
                return Some(image);
            }
            if let Some(moving_id) = moving_id {
                let base = self.ensure_interaction_base_layer(selection, background, scale, moving_id)?;
                self.raster_cache.moving_fast_path_hits = self.raster_cache.moving_fast_path_hits.saturating_add(1);
                let image = self.render_transient_on_base(base.as_ref(), selection, scale, transient);
                self.cache_composed(selection, scale, image.clone());
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
        self.cache_composed(selection, scale, image.clone());
        Some(image)
    }

    fn ensure_committed_layer(&mut self, selection: RectF, background: &Arc<RgbaImage>, scale: f64) -> Option<CommittedLayerCache> {
        let cache_valid = self.raster_cache.committed.as_ref().is_some_and(|cache| {
            cache.selection == selection && (cache.scale - scale).abs() <= f64::EPSILON && cache.revision == self.committed_revision
        });
        if cache_valid {
            return self.raster_cache.committed.clone();
        }

        let layer = compose_selection_base(background.as_ref(), selection, self.store.visible_items(), scale)?;
        let image = render_image::from_rgba(layer.clone());
        self.raster_cache.committed_rebuilds = self.raster_cache.committed_rebuilds.saturating_add(1);
        self.raster_cache.committed = Some(CommittedLayerCache {
            selection,
            scale,
            revision: self.committed_revision,
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

    fn cache_composed(&mut self, selection: RectF, scale: f64, image: Arc<RenderImage>) {
        self.raster_cache.composed_rebuilds = self.raster_cache.composed_rebuilds.saturating_add(1);
        self.raster_cache.composed = Some(ComposedLayerCache {
            selection,
            scale,
            committed_revision: self.committed_revision,
            transient_revision: self.transient_revision,
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

    fn consume_id(&mut self) -> u64 {
        let id = self.next_id.max(1);
        self.next_id = id.saturating_add(1);
        id
    }

    fn commit_item(&mut self, item: AnnotationItem) {
        self.store.push(item);
        self.selected_id = self.store.visible_items().last().map(|annotation| annotation.id);
        self.sync_style_from_selected();
        self.bump_committed();
    }

    fn selected_item_mut(&mut self) -> Option<&mut AnnotationItem> {
        let id = self.selected_id?;
        self.store.visible_item_mut(id)
    }

    fn sync_style_from_selected(&mut self) {
        let Some(item) = self.selected_item().cloned() else {
            return;
        };
        sync_style_from_item(&mut self.style, &item);
    }

    fn bump_committed(&mut self) {
        self.committed_revision = self.committed_revision.saturating_add(1);
        self.transient_revision = self.transient_revision.saturating_add(1);
        self.raster_cache.invalidate_composed();
        self.raster_cache.invalidate_interaction_base();
    }

    fn bump_transient(&mut self) {
        self.transient_revision = self.transient_revision.saturating_add(1);
        self.raster_cache.invalidate_composed();
    }

    fn mode_enabled(&self, selection: Option<RectF>, idle_mode: bool) -> bool {
        selection.is_some() && idle_mode
    }

    fn point_in_selection(&self, point: (f64, f64), selection: Option<RectF>) -> bool {
        let Some(selection) = selection else {
            return false;
        };
        point.0 >= selection.x && point.0 <= selection.x + selection.width && point.1 >= selection.y && point.1 <= selection.y + selection.height
    }

    fn clamp_to_selection(&self, point: (f64, f64), selection: Option<RectF>) -> (f64, f64) {
        let Some(selection) = selection else {
            return point;
        };
        (
            point.0.clamp(selection.x, selection.x + selection.width),
            point.1.clamp(selection.y, selection.y + selection.height),
        )
    }
}

fn kind_tag(item: &AnnotationItem) -> AnnotationKindTag {
    item.kind.tag()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection() -> RectF {
        RectF::new(20.0, 20.0, 160.0, 100.0)
    }

    fn fake_background() -> Arc<RgbaImage> {
        Arc::new(RgbaImage::new(400, 240))
    }

    #[test]
    fn default_and_clear_leave_tool_unselected() {
        let mut engine = AnnotationEngine::default();
        assert_eq!(engine.tool, None);

        assert!(engine.set_tool(AnnotationTool::Rectangle));
        assert_eq!(engine.tool, Some(AnnotationTool::Rectangle));

        engine.clear();
        assert_eq!(engine.tool, None);
    }

    #[test]
    fn set_tool_toggles_when_reselecting_same_tool() {
        let mut engine = AnnotationEngine::default();
        assert!(engine.set_tool(AnnotationTool::Rectangle));
        assert_eq!(engine.tool, Some(AnnotationTool::Rectangle));

        assert!(engine.set_tool(AnnotationTool::Rectangle));
        assert_eq!(engine.tool, None);

        assert!(engine.set_tool(AnnotationTool::Arrow));
        assert_eq!(engine.tool, Some(AnnotationTool::Arrow));
    }

    #[test]
    fn no_tool_start_draw_is_noop() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());

        assert!(!engine.start_draw((40.0, 60.0), sel, true));
        assert!(engine.selected_item().is_none());
        assert!(!engine.has_active_interaction());
    }

    #[test]
    fn draft_preview_does_not_mutate_committed_text_before_commit() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());

        engine.set_tool(AnnotationTool::Text);
        assert!(engine.start_draw((40.0, 60.0), sel, true));
        assert!(engine.append_text_edit("X"));
        let selected_before = engine.selected_item().cloned().unwrap();
        let AnnotationKind::Text { text, .. } = selected_before.kind else {
            panic!("expected text");
        };
        assert_eq!(text, TEXT_DEFAULT);
        assert!(engine.commit_text_edit());

        let selected_after = engine.selected_item().cloned().unwrap();
        let AnnotationKind::Text { text, .. } = selected_after.kind else {
            panic!("expected text");
        };
        assert_eq!(text, "TextX");
    }

    #[test]
    fn moving_interaction_applies_only_on_finish() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());
        let bg = fake_background();

        engine.set_tool(AnnotationTool::Rectangle);
        assert!(engine.start_draw((30.0, 40.0), sel, true));
        assert!(engine.update_interaction((90.0, 80.0), sel));
        assert!(engine.finish_interaction(8.0));
        let id = engine.selected_item().unwrap().id;
        let before = engine.selected_item().unwrap().bounds();

        assert!(engine.start_move(id, (40.0, 50.0), sel, true));
        assert!(engine.update_interaction((80.0, 70.0), sel));
        let during = engine.selected_item().unwrap().bounds();
        assert_eq!(before, during);
        assert!(engine.finish_interaction(8.0));
        let after = engine.selected_item().unwrap().bounds();
        assert!(after.x > before.x);

        let _ = engine.ui_state(sel, Some(&bg), 1.0);
    }

    #[test]
    fn committed_cache_not_rebuilt_when_only_tool_changes() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());
        let bg = fake_background();

        engine.set_tool(AnnotationTool::Rectangle);
        assert!(engine.start_draw((30.0, 40.0), sel, true));
        assert!(engine.update_interaction((90.0, 80.0), sel));
        assert!(engine.finish_interaction(8.0));

        let _ = engine.ui_state(sel, Some(&bg), 1.0);
        let before = engine.raster_rebuilds();

        engine.set_tool(AnnotationTool::Arrow);
        let _ = engine.ui_state(sel, Some(&bg), 1.0);
        let after = engine.raster_rebuilds();

        assert_eq!(before.0, after.0);
    }

    #[test]
    fn set_color_updates_selected_stroke_and_fill_rgb() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());

        engine.set_tool(AnnotationTool::Rectangle);
        assert!(engine.start_draw((30.0, 40.0), sel, true));
        assert!(engine.update_interaction((90.0, 80.0), sel));
        assert!(engine.finish_interaction(8.0));

        let new_color = 0x55aaffff;
        assert!(engine.set_color(new_color));
        let selected = engine.selected_item().cloned().unwrap();
        assert_eq!(selected.style.stroke_color, new_color);
        assert_eq!(selected.style.fill_color & 0xffffff00, new_color & 0xffffff00);
    }

    #[test]
    fn set_color_same_value_is_noop() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());
        let bg = fake_background();

        engine.set_tool(AnnotationTool::Rectangle);
        assert!(engine.start_draw((30.0, 40.0), sel, true));
        assert!(engine.update_interaction((90.0, 80.0), sel));
        assert!(engine.finish_interaction(8.0));

        let selected = engine.selected_item().unwrap().style.stroke_color;
        assert!(engine.set_color(selected));
        let _ = engine.ui_state(sel, Some(&bg), 1.0);
        let before = engine.raster_rebuilds();
        assert!(!engine.set_color(selected));
        let _ = engine.ui_state(sel, Some(&bg), 1.0);
        let after = engine.raster_rebuilds();
        assert_eq!(before, after);
    }

    #[test]
    fn set_color_updates_default_style_without_selection() {
        let mut engine = AnnotationEngine::default();
        let next_color = 0x33bb66ff;
        assert!(engine.set_color(next_color));
        assert_eq!(engine.style.stroke_color, next_color);
        assert_eq!(engine.style.fill_color & 0xffffff00, next_color & 0xffffff00);
    }

    #[test]
    fn adjust_stroke_noop_at_clamp_without_selection() {
        let mut engine = AnnotationEngine::default();
        engine.style.stroke_width = 18.0;
        engine.style.text_size = 96.0;
        engine.style.counter_radius = 64.0;
        let before = engine.transient_revision;
        assert!(!engine.adjust_stroke(1.0));
        assert_eq!(engine.transient_revision, before);
    }

    #[test]
    fn set_mosaic_mode_same_value_is_noop_without_selection() {
        let mut engine = AnnotationEngine::default();
        let before = engine.transient_revision;
        assert!(!engine.set_mosaic_mode(MosaicMode::Pixelate));
        assert_eq!(engine.transient_revision, before);
    }

    #[test]
    fn adjust_mosaic_intensity_noop_when_clamped_without_selection() {
        let mut engine = AnnotationEngine::default();
        engine.style.mosaic_intensity = 64.0;
        let before = engine.transient_revision;
        assert!(!engine.adjust_mosaic_intensity(4.0));
        assert_eq!(engine.transient_revision, before);
    }

    #[test]
    fn committed_cache_rebuilds_when_scale_changes() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());
        let bg = fake_background();

        engine.set_tool(AnnotationTool::Rectangle);
        assert!(engine.start_draw((30.0, 40.0), sel, true));
        assert!(engine.update_interaction((90.0, 80.0), sel));
        assert!(engine.finish_interaction(8.0));

        let _ = engine.ui_state(sel, Some(&bg), 1.0);
        let before = engine.raster_diagnostics();
        let _ = engine.ui_state(sel, Some(&bg), 1.5);
        let after = engine.raster_diagnostics();
        assert_eq!(after.committed_rebuilds, before.committed_rebuilds + 1);
    }

    #[test]
    fn moving_fast_path_reuses_interaction_base_layer() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());
        let bg = fake_background();

        engine.set_tool(AnnotationTool::Rectangle);
        assert!(engine.start_draw((30.0, 40.0), sel, true));
        assert!(engine.update_interaction((90.0, 80.0), sel));
        assert!(engine.finish_interaction(8.0));

        assert!(engine.start_draw((100.0, 50.0), sel, true));
        assert!(engine.update_interaction((150.0, 90.0), sel));
        assert!(engine.finish_interaction(8.0));

        let moving_id = engine.selected_item().unwrap().id;
        let _ = engine.ui_state(sel, Some(&bg), 1.0);
        let before = engine.raster_diagnostics();

        assert!(engine.start_move(moving_id, (120.0, 70.0), sel, true));
        assert!(engine.update_interaction((130.0, 75.0), sel));
        let _ = engine.ui_state(sel, Some(&bg), 1.0);
        assert!(engine.update_interaction((145.0, 85.0), sel));
        let _ = engine.ui_state(sel, Some(&bg), 1.0);

        let after = engine.raster_diagnostics();
        assert_eq!(after.interaction_base_rebuilds, before.interaction_base_rebuilds + 1);
        assert_eq!(after.committed_rebuilds, before.committed_rebuilds);
        assert!(after.moving_fast_path_hits >= before.moving_fast_path_hits + 2);
    }

    #[test]
    fn moving_fast_path_scales_in_high_frequency_updates() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());
        let bg = fake_background();

        engine.set_tool(AnnotationTool::Rectangle);
        assert!(engine.start_draw((30.0, 40.0), sel, true));
        assert!(engine.update_interaction((90.0, 80.0), sel));
        assert!(engine.finish_interaction(8.0));
        assert!(engine.start_draw((100.0, 50.0), sel, true));
        assert!(engine.update_interaction((150.0, 90.0), sel));
        assert!(engine.finish_interaction(8.0));

        let moving_id = engine.selected_item().unwrap().id;
        let _ = engine.ui_state(sel, Some(&bg), 1.0);
        let before = engine.raster_diagnostics();

        assert!(engine.start_move(moving_id, (120.0, 70.0), sel, true));
        let mut changed_steps = 0u64;
        for step in 0..120 {
            let x = 120.0 + f64::from(step) * 0.6;
            let y = 70.0 + f64::from(step) * 0.4;
            if engine.update_interaction((x, y), sel) {
                changed_steps += 1;
                let _ = engine.ui_state(sel, Some(&bg), 1.0);
            }
        }

        let after = engine.raster_diagnostics();
        assert_eq!(after.interaction_base_rebuilds, before.interaction_base_rebuilds + 1);
        assert_eq!(after.committed_rebuilds, before.committed_rebuilds);
        assert!(changed_steps > 0);
        assert!(after.moving_fast_path_hits >= before.moving_fast_path_hits + changed_steps);
    }

    #[test]
    fn drawing_fast_path_hits_in_high_frequency_updates() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());
        let bg = fake_background();

        engine.set_tool(AnnotationTool::Arrow);
        assert!(engine.start_draw((40.0, 55.0), sel, true));
        let _ = engine.ui_state(sel, Some(&bg), 1.0);
        let before = engine.raster_diagnostics();
        let mut changed_steps = 0u64;
        for step in 0..120 {
            let x = 40.0 + f64::from(step) * 0.7;
            let y = 55.0 + f64::from(step) * 0.45;
            if engine.update_interaction((x, y), sel) {
                changed_steps += 1;
                let _ = engine.ui_state(sel, Some(&bg), 1.0);
            }
        }
        let after = engine.raster_diagnostics();
        assert_eq!(after.interaction_base_rebuilds, before.interaction_base_rebuilds);
        assert!(changed_steps > 0);
        assert!(after.drawing_fast_path_hits >= before.drawing_fast_path_hits + changed_steps);
        assert_eq!(after.committed_rebuilds, before.committed_rebuilds);
    }
}

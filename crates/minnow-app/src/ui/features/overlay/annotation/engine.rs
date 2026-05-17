use std::cell::RefCell;

use crate::services::geometry::RectF;

use super::hit_test::AnnotationHitProxy;
use super::model::{AnnotationInteractionState, AnnotationItem, AnnotationKind, AnnotationStyleState, AnnotationTool, TextEditState};
use super::ops::sync_style_from_item;
use super::raster_cache::AnnotationRasterCache;
use super::store::AnnotationStore;

mod document;
mod interaction;
mod render_state;
mod style;
mod text;

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
}

impl AnnotationEngine {
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

#[cfg(test)]
mod tests {
    use super::super::model::{MosaicMode, TEXT_DEFAULT};
    use super::*;
    use image::RgbaImage;
    use std::sync::Arc;

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

        let _ = engine.ui_state(sel, Some(&bg), 1.0, None);
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

        let _ = engine.ui_state(sel, Some(&bg), 1.0, None);
        let before = engine.raster_rebuilds();

        engine.set_tool(AnnotationTool::Arrow);
        let _ = engine.ui_state(sel, Some(&bg), 1.0, None);
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
        let _ = engine.ui_state(sel, Some(&bg), 1.0, None);
        let before = engine.raster_rebuilds();
        assert!(!engine.set_color(selected));
        let _ = engine.ui_state(sel, Some(&bg), 1.0, None);
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

        let _ = engine.ui_state(sel, Some(&bg), 1.0, None);
        let before = engine.raster_diagnostics();
        let _ = engine.ui_state(sel, Some(&bg), 1.5, None);
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
        let _ = engine.ui_state(sel, Some(&bg), 1.0, None);
        let before = engine.raster_diagnostics();

        assert!(engine.start_move(moving_id, (120.0, 70.0), sel, true));
        assert!(engine.update_interaction((130.0, 75.0), sel));
        let _ = engine.ui_state(sel, Some(&bg), 1.0, None);
        assert!(engine.update_interaction((145.0, 85.0), sel));
        let _ = engine.ui_state(sel, Some(&bg), 1.0, None);

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
        let _ = engine.ui_state(sel, Some(&bg), 1.0, None);
        let before = engine.raster_diagnostics();

        assert!(engine.start_move(moving_id, (120.0, 70.0), sel, true));
        let mut changed_steps = 0u64;
        for step in 0..120 {
            let x = 120.0 + f64::from(step) * 0.6;
            let y = 70.0 + f64::from(step) * 0.4;
            if engine.update_interaction((x, y), sel) {
                changed_steps += 1;
                let _ = engine.ui_state(sel, Some(&bg), 1.0, None);
            }
        }

        let after = engine.raster_diagnostics();
        assert_eq!(after.interaction_base_rebuilds, before.interaction_base_rebuilds + 1);
        assert_eq!(after.committed_rebuilds, before.committed_rebuilds);
        assert!(changed_steps > 0);
        assert!(after.moving_fast_path_hits >= before.moving_fast_path_hits + changed_steps);
    }

    #[test]
    fn selection_move_preview_reuses_cached_composed_layer_for_same_delta() {
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

        let _ = engine.ui_state(sel, Some(&bg), 1.0, None);
        let before = engine.raster_diagnostics();

        let first = engine
            .ui_state(sel, Some(&bg), 1.0, Some((18.0, 12.0)))
            .layer
            .image
            .expect("preview translate should render a layer");
        let after_first = engine.raster_diagnostics();
        assert_eq!(after_first.committed_rebuilds, before.committed_rebuilds);
        assert_eq!(after_first.composed_rebuilds, before.composed_rebuilds + 1);

        let second = engine
            .ui_state(sel, Some(&bg), 1.0, Some((18.0, 12.0)))
            .layer
            .image
            .expect("same preview delta should reuse cached layer");
        let after_second = engine.raster_diagnostics();

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(after_second.composed_rebuilds, after_first.composed_rebuilds);
        assert_eq!(after_second.committed_rebuilds, after_first.committed_rebuilds);
    }

    #[test]
    fn drawing_fast_path_hits_in_high_frequency_updates() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());
        let bg = fake_background();

        engine.set_tool(AnnotationTool::Arrow);
        assert!(engine.start_draw((40.0, 55.0), sel, true));
        let _ = engine.ui_state(sel, Some(&bg), 1.0, None);
        let before = engine.raster_diagnostics();
        let mut changed_steps = 0u64;
        for step in 0..120 {
            let x = 40.0 + f64::from(step) * 0.7;
            let y = 55.0 + f64::from(step) * 0.45;
            if engine.update_interaction((x, y), sel) {
                changed_steps += 1;
                let _ = engine.ui_state(sel, Some(&bg), 1.0, None);
            }
        }
        let after = engine.raster_diagnostics();
        assert_eq!(after.interaction_base_rebuilds, before.interaction_base_rebuilds);
        assert!(changed_steps > 0);
        assert!(after.drawing_fast_path_hits >= before.drawing_fast_path_hits + changed_steps);
        assert_eq!(after.committed_rebuilds, before.committed_rebuilds);
    }
}

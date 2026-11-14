use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use image::RgbaImage;
use minnow_imaging::geometry::RectF;
use minnow_imaging::{GeometryError, ImageAsset, ImageRef, ResolvedSelection};

use super::hit_test::AnnotationHitProxy;
use super::model::{AnnotationInteractionState, AnnotationItem, AnnotationKind, AnnotationStyleState, AnnotationTool, TextEditState};
use super::ops::{annotation_item_large_enough, build_drawing_item, finite_point, positive_finite, sync_style_from_item, valid_selection};
use super::raster_cache::AnnotationRasterCache;
use super::store::{AnnotationStore, HistoryBudget, HistoryUsage};

#[cfg(test)]
mod contracts;
mod document;
mod interaction;
mod render_state;
mod style;
mod text;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnnotationRevisions {
    pub committed: u64,
    pub transient: u64,
    /// Output content and effective drafts, excluding selection and defaults.
    pub output: u64,
}

#[derive(Clone, Debug, Default)]
struct OutputDraft {
    drawing: Option<AnnotationItem>,
    moving: Option<(u64, f64, f64)>,
    text: Option<(u64, String)>,
}

impl PartialEq for OutputDraft {
    fn eq(&self, other: &Self) -> bool {
        let drawing_equal = match (&self.drawing, &other.drawing) {
            (Some(before), Some(after)) => before.output_eq(after),
            (None, None) => true,
            _ => false,
        };
        drawing_equal && self.moving == other.moving && self.text == other.text
    }
}

static NEXT_OUTPUT_IDENTITY: AtomicU64 = AtomicU64::new(1);

/// Opaque identity of frozen annotation output. Repeated snapshots reuse it;
/// independently edited engine clones receive distinct identities.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AnnotationOutputIdentity(u64);

impl AnnotationOutputIdentity {
    fn fresh() -> Self {
        Self(NEXT_OUTPUT_IDENTITY.fetch_add(1, Ordering::Relaxed))
    }
}

/// A frozen committed document, suitable for use by an export worker.
#[derive(Clone, Debug)]
pub struct AnnotationSnapshot {
    /// Identifies the committed document used by raster caches.
    committed_revision: u64,
    /// Identifies output content, including the engine's effective draft state
    /// at capture time. Output preparation settles those drafts before capture.
    output_revision: u64,
    output_identity: AnnotationOutputIdentity,
    items: Arc<[AnnotationItem]>,
    text_font: Option<ab_glyph::FontArc>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnnotationInputError {
    InvalidMinimumSize,
}

impl std::fmt::Display for AnnotationInputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMinimumSize => formatter.write_str("minimum annotation size must be finite and positive"),
        }
    }
}

impl std::error::Error for AnnotationInputError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnnotationOutputError {
    MissingFont,
    InvalidScale,
    Geometry(GeometryError),
}

impl std::fmt::Display for AnnotationOutputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingFont => formatter.write_str("text and counter output requires a text font"),
            Self::InvalidScale => formatter.write_str("annotation output scale must be finite and positive"),
            Self::Geometry(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for AnnotationOutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Geometry(error) => Some(error),
            _ => None,
        }
    }
}

impl From<GeometryError> for AnnotationOutputError {
    fn from(error: GeometryError) -> Self {
        Self::Geometry(error)
    }
}

impl AnnotationSnapshot {
    /// The committed document revision used by raster caches.
    pub fn committed_revision(&self) -> u64 {
        self.committed_revision
    }

    /// The output validity token captured with these items. Use
    /// `prepare_output_snapshot` to settle effective drafts before capture.
    pub fn output_revision(&self) -> u64 {
        self.output_revision
    }

    pub fn output_identity(&self) -> AnnotationOutputIdentity {
        self.output_identity
    }

    /// Frozen annotations in document stacking order.
    pub fn items(&self) -> &[AnnotationItem] {
        &self.items
    }

    /// Checks output prerequisites without allocating pixels or rendering.
    /// Callers may use this before registering an asynchronous output job.
    pub fn validate_for_composition(&self, scale: f64) -> Result<(), AnnotationOutputError> {
        if !positive_finite(scale) {
            return Err(AnnotationOutputError::InvalidScale);
        }
        if self.text_font.is_none()
            && self
                .items
                .iter()
                .any(|item| matches!(item.kind, AnnotationKind::Text { .. } | AnnotationKind::Counter { .. }))
        {
            return Err(AnnotationOutputError::MissingFont);
        }
        Ok(())
    }

    /// Composes a complete output, rejecting invalid scales or missing glyph
    /// resources before rendering any annotations.
    pub fn compose_background(&self, background: &Arc<RgbaImage>, scale: f64) -> Result<Arc<RgbaImage>, AnnotationOutputError> {
        self.validate_for_composition(scale)?;
        if self.items.is_empty() {
            return Ok(background.clone());
        }
        Ok(Arc::new(super::raster::compose_background_with_annotations(
            background,
            &self.items,
            scale,
            self.text_font.as_ref(),
        )))
    }

    /// Renders the validated source selection with the same clipping as preview.
    /// Pixels outside the selection do not participate in blur or pixelation.
    pub fn compose_selection(&self, source: &ImageRef, selection: ResolvedSelection) -> Result<ImageRef, AnnotationOutputError> {
        selection.validate_source(source.dimensions())?;
        self.validate_for_composition(selection.scale_factor())?;
        let rect = selection.pixel_rect();
        if self.items.is_empty() && rect.x == 0 && rect.y == 0 && (rect.width, rect.height) == source.dimensions() {
            return Ok(source.clone());
        }
        let image = super::raster::compose_selection_base(
            source.pixels(),
            &selection,
            &self.items,
            selection.scale_factor(),
            self.text_font.as_ref(),
        );
        Ok(ImageAsset::new(image))
    }

    pub fn compose_asset(&self, source: &ImageRef, scale: f64) -> Result<ImageRef, AnnotationOutputError> {
        let image = self.compose_background(&source.shared_pixels(), scale)?;
        if self.items.is_empty() {
            return Ok(source.clone());
        }
        Ok(ImageAsset::from_shared(image))
    }
}

#[derive(Clone, Debug)]
pub struct AnnotationEngine {
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
    output_revision: u64,
    output_identity: AnnotationOutputIdentity,
    output_content_revision: u64,
    output_draft: OutputDraft,
    hit_proxy: RefCell<AnnotationHitProxy>,
    raster_cache: AnnotationRasterCache,
    text_font: Option<ab_glyph::FontArc>,
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
            output_revision: 1,
            output_identity: AnnotationOutputIdentity::fresh(),
            output_content_revision: 0,
            output_draft: OutputDraft::default(),
            hit_proxy: RefCell::new(AnnotationHitProxy::default()),
            raster_cache: AnnotationRasterCache::default(),
            text_font: None,
        }
    }
}

impl AnnotationEngine {
    pub fn style(&self) -> AnnotationStyleState {
        self.style
    }

    pub fn revisions(&self) -> AnnotationRevisions {
        AnnotationRevisions {
            committed: self.committed_revision,
            transient: self.transient_revision,
            output: self.output_revision,
        }
    }

    /// A validity token for output jobs. Selection, active tools and default
    /// styles do not invalidate it. Committed edits and effective pointer/text
    /// drafts do; callers still track source and selection changes separately.
    pub fn output_revision(&self) -> u64 {
        self.output_revision
    }

    /// Freezes committed items. Export callers must first settle any active text
    /// draft or pointer interaction according to their application policy.
    pub fn snapshot(&self) -> AnnotationSnapshot {
        AnnotationSnapshot {
            committed_revision: self.committed_revision,
            output_revision: self.output_revision,
            output_identity: self.output_identity,
            items: self.store.clone_visible_items().into(),
            text_font: self.text_font.clone(),
        }
    }

    /// Commits a valid pointer gesture and the current text draft, closes any
    /// wheel gesture, and freezes the exact document to send to an output job.
    /// An invalid threshold is rejected before changing any interaction,
    /// text draft, revision or history transaction.
    pub fn prepare_output_snapshot(&mut self, min_selection_size: f64) -> Result<AnnotationSnapshot, AnnotationInputError> {
        if !positive_finite(min_selection_size) {
            return Err(AnnotationInputError::InvalidMinimumSize);
        }
        self.finish_interaction(min_selection_size);
        self.commit_text_edit();
        self.store.finish_edit_group();
        Ok(self.snapshot())
    }

    /// Replaces the caller-resolved font and invalidates the raster context.
    /// `None` disables text and counter glyph rasterization without changing the
    /// editable document; callers resolve and report font availability.
    pub fn set_text_font(&mut self, font: Option<ab_glyph::FontArc>) {
        let output_may_change = self.text_font.is_some() || font.is_some();
        self.text_font = font;
        if output_may_change
            && self
                .store
                .visible_items()
                .iter()
                .any(|item| matches!(item.kind, AnnotationKind::Text { .. } | AnnotationKind::Counter { .. }))
        {
            self.advance_output_revision();
        }
        self.transient_revision = self.transient_revision.saturating_add(1);
        self.hit_proxy.borrow_mut().clear();
        self.raster_cache.invalidate_layers();
    }

    pub fn set_history_budget(&mut self, budget: HistoryBudget) {
        self.store.set_history_budget(budget);
    }

    pub fn set_raster_budget(&mut self, budget: super::raster_cache::RasterBudget) {
        self.raster_cache.budget = budget;
        self.raster_cache.enforce_budget();
    }

    pub fn history_usage(&self) -> HistoryUsage {
        self.store.history_usage()
    }

    pub fn clear(&mut self) {
        let had_content = self.store.visible_len() != 0 || self.output_draft != OutputDraft::default();
        self.store.clear();
        self.tool = None;
        self.style = AnnotationStyleState::default();
        self.selected_id = None;
        self.interaction = AnnotationInteractionState::Idle;
        self.text_editing = None;
        self.next_id = 1;
        self.next_counter = 1;
        self.committed_revision = self.committed_revision.saturating_add(1);
        self.transient_revision = self.transient_revision.saturating_add(1);
        if had_content {
            self.advance_output_revision();
        }
        self.output_draft = OutputDraft::default();
        self.output_content_revision = self.store.content_revision();
        self.hit_proxy.borrow_mut().clear();
        self.raster_cache.clear();
    }

    pub fn selected_item(&self) -> Option<&AnnotationItem> {
        let id = self.selected_id?;
        self.store.visible_item(id)
    }

    pub fn kind_for(&self, id: u64) -> Option<AnnotationKind> {
        Some(self.store.visible_item(id)?.kind.clone())
    }

    pub fn text_editing_id(&self) -> Option<u64> {
        self.text_editing.as_ref().map(|state| state.id)
    }

    pub fn can_undo(&self) -> bool {
        self.store.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.store.can_redo()
    }

    pub fn has_active_interaction(&self) -> bool {
        !matches!(self.interaction, AnnotationInteractionState::Idle)
    }

    pub fn has_active_tool(&self) -> bool {
        self.tool.is_some()
    }

    pub fn active_tool(&self) -> Option<AnnotationTool> {
        self.tool
    }

    pub fn set_tool(&mut self, tool: AnnotationTool) -> bool {
        self.store.finish_edit_group();
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

    fn sync_style_from_selected(&mut self) {
        let Some(item) = self.selected_item().cloned() else {
            return;
        };
        sync_style_from_item(&mut self.style, &item);
    }

    fn bump_committed(&mut self) {
        self.committed_revision = self.committed_revision.saturating_add(1);
        self.transient_revision = self.transient_revision.saturating_add(1);
        self.refresh_output_revision();
        self.raster_cache.invalidate_composed();
        self.raster_cache.invalidate_interaction_base();
    }

    fn bump_transient(&mut self) {
        self.transient_revision = self.transient_revision.saturating_add(1);
        self.refresh_output_revision();
        self.raster_cache.invalidate_composed();
    }

    fn advance_output_revision(&mut self) {
        self.output_revision = self.output_revision.saturating_add(1);
        self.output_identity = AnnotationOutputIdentity::fresh();
    }

    fn refresh_output_revision(&mut self) {
        let draft = self.current_output_draft();
        let content = self.store.content_revision();
        if draft != self.output_draft || content != self.output_content_revision {
            self.advance_output_revision();
            self.output_draft = draft;
            self.output_content_revision = content;
        }
    }

    fn current_output_draft(&self) -> OutputDraft {
        let mut draft = OutputDraft::default();
        match &self.interaction {
            AnnotationInteractionState::Drawing { tool, start, current, style } => {
                draft.drawing = build_drawing_item(*tool, *start, *current, *style, 0)
                    // Any nonempty rectangular draft may be accepted by the
                    // caller's finish threshold. Arrows have a fixed minimum.
                    .filter(|item| annotation_item_large_enough(item, f64::MIN_POSITIVE));
            }
            AnnotationInteractionState::Moving { id, start, current } if self.store.visible_contains(*id) => {
                let dx = current.0 - start.0;
                let dy = current.1 - start.1;
                if dx.abs() > f64::EPSILON || dy.abs() > f64::EPSILON {
                    draft.moving = Some((*id, dx, dy));
                }
            }
            _ => {}
        }
        if let Some(edit) = &self.text_editing
            && let Some(item) = self.store.visible_item(edit.id)
            && let AnnotationKind::Text { text, .. } = &item.kind
        {
            let next = text::output_text(&edit.draft);
            if next != text {
                draft.text = Some((edit.id, next.to_owned()));
            }
        }
        draft
    }

    fn mode_enabled(&self, selection: Option<RectF>, idle_mode: bool) -> bool {
        selection.is_some() && idle_mode
    }

    fn point_in_selection(&self, point: (f64, f64), selection: Option<RectF>) -> bool {
        let Some(selection) = selection else {
            return false;
        };
        finite_point(point) && valid_selection(selection) && selection.contains_point(point.0, point.1)
    }

    fn clamp_to_selection(&self, point: (f64, f64), selection: Option<RectF>) -> Option<(f64, f64)> {
        let selection = selection?;
        if !finite_point(point) || !valid_selection(selection) {
            return None;
        }
        Some((
            point.0.clamp(selection.x, selection.x + selection.width),
            point.1.clamp(selection.y, selection.y + selection.height),
        ))
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

    #[test]
    fn unmodified_exports_share_the_original_image() {
        let engine = AnnotationEngine::default();
        let background = fake_background();
        let result = engine.composed_background_image(Some(&background), 1.0).unwrap();
        assert!(Arc::ptr_eq(&background, &result));

        let asset = ImageAsset::from_shared(background);
        let exported = engine.snapshot().compose_asset(&asset, 1.0).unwrap();
        assert!(Arc::ptr_eq(&asset, &exported));
    }

    #[test]
    fn snapshot_remains_unchanged_after_more_edits() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());
        engine.set_tool(AnnotationTool::Rectangle);
        assert!(engine.start_draw((30.0, 40.0), sel, true));
        assert!(engine.update_interaction((90.0, 80.0), sel));
        assert!(engine.finish_interaction(8.0));
        let snapshot = engine.snapshot();
        let items = snapshot.items.clone();

        assert!(engine.set_color(0x00ff00ff));
        assert_ne!(engine.revisions().committed, snapshot.committed_revision);
        assert_eq!(snapshot.items, items);
        assert_ne!(engine.snapshot().items, snapshot.items);
    }

    #[test]
    fn replacing_equal_sized_background_invalidates_committed_and_transient_pixels() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());
        let red = Arc::new(RgbaImage::from_pixel(400, 240, image::Rgba([255, 0, 0, 255])));
        let blue = Arc::new(RgbaImage::from_pixel(400, 240, image::Rgba([0, 0, 255, 255])));
        engine.set_tool(AnnotationTool::Rectangle);
        assert!(engine.start_draw((30.0, 40.0), sel, true));
        assert!(engine.update_interaction((90.0, 80.0), sel));
        assert!(engine.finish_interaction(8.0));

        let first = engine.ui_state(sel, Some(&red), 1.0, Some((2.0, 2.0))).layer.image.unwrap();
        let before = engine.raster_diagnostics();
        let second = engine.ui_state(sel, Some(&blue), 1.0, Some((2.0, 2.0))).layer.image.unwrap();
        let after = engine.raster_diagnostics();

        assert_eq!(first.get_pixel(0, 0), &image::Rgba([255, 0, 0, 255]));
        assert_eq!(second.get_pixel(0, 0), &image::Rgba([0, 0, 255, 255]));
        assert!(!Arc::ptr_eq(&first, &second));
        assert_eq!(after.composed_rebuilds, before.composed_rebuilds + 1);
    }

    #[test]
    fn replacing_font_context_invalidates_cached_layers() {
        let mut engine = AnnotationEngine::default();
        let sel = Some(selection());
        let background = fake_background();
        engine.set_tool(AnnotationTool::Rectangle);
        assert!(engine.start_draw((30.0, 40.0), sel, true));
        assert!(engine.update_interaction((90.0, 80.0), sel));
        assert!(engine.finish_interaction(8.0));
        let first = engine.ui_state(sel, Some(&background), 1.0, None).layer.image.unwrap();
        let before = engine.revisions();

        engine.set_text_font(None);
        let second = engine.ui_state(sel, Some(&background), 1.0, None).layer.image.unwrap();
        assert!(!Arc::ptr_eq(&first, &second));
        assert_eq!(engine.revisions().committed, before.committed);
        assert!(engine.revisions().transient > before.transient);
    }

    #[test]
    fn frozen_document_can_be_composed_by_a_background_worker() {
        let engine = AnnotationEngine::default();
        let snapshot = engine.snapshot();
        let asset = ImageAsset::new(RgbaImage::from_pixel(8, 6, image::Rgba([10, 20, 30, 255])));
        let expected_id = asset.id();
        let exported = std::thread::spawn(move || snapshot.compose_asset(&asset, 1.0)).join().unwrap().unwrap();
        assert_eq!(exported.id(), expected_id);
        assert_eq!(exported.pixels().get_pixel(0, 0), &image::Rgba([10, 20, 30, 255]));
    }
}

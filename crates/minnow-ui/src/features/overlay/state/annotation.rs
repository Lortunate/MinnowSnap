use gpui::{Pixels, Point};

#[cfg(test)]
use crate::features::overlay::annotation::AnnotationItem;
use crate::features::overlay::annotation::{AnnotationKind, AnnotationTool, AnnotationUiState, MosaicMode};
use minnow_core::capture::active_monitor_scale;

use super::{DragMode, OverlaySession};

impl OverlaySession {
    pub(crate) fn annotation_ui_state(&mut self) -> AnnotationUiState {
        let scale = f64::from(active_monitor_scale()).max(1.0);
        self.annotation.ui_state(self.viewport.selection, self.background_pixels.as_ref(), scale)
    }

    pub(crate) fn text_editing_id(&self) -> Option<u64> {
        self.annotation.text_editing_id()
    }

    #[cfg(test)]
    pub(crate) fn selected_annotation_item(&self) -> Option<AnnotationItem> {
        self.annotation.selected_item().cloned()
    }

    pub(crate) fn annotation_kind_for(&self, id: u64) -> Option<AnnotationKind> {
        self.annotation.kind_for(id)
    }

    pub(crate) fn annotation_hit_test(&self, point: Point<Pixels>) -> Option<u64> {
        let (x, y) = self.clamp_point_to_viewport(point);
        self.annotation
            .hit_test((x, y), self.viewport.selection, self.viewport.mode == DragMode::Idle)
    }

    pub(crate) fn set_annotation_tool(&mut self, tool: AnnotationTool) -> bool {
        self.annotation.set_tool(tool)
    }

    pub(crate) fn has_active_annotation_tool(&self) -> bool {
        self.annotation.has_active_tool()
    }

    pub(crate) fn cycle_annotation_color(&mut self) -> bool {
        self.annotation.cycle_color()
    }

    pub(crate) fn set_annotation_color(&mut self, color: u32) -> bool {
        self.annotation.set_color(color)
    }

    pub(crate) fn toggle_annotation_fill(&mut self) -> bool {
        self.annotation.toggle_fill()
    }

    pub(crate) fn adjust_annotation_stroke(&mut self, delta: f64) -> bool {
        self.annotation.adjust_stroke(delta)
    }

    pub(crate) fn set_annotation_mosaic_mode(&mut self, mode: MosaicMode) -> bool {
        self.annotation.set_mosaic_mode(mode)
    }

    pub(crate) fn adjust_annotation_mosaic_intensity(&mut self, delta: f64) -> bool {
        self.annotation.adjust_mosaic_intensity(delta)
    }

    pub(crate) fn adjust_selected_annotation_by_wheel(&mut self, point: Point<Pixels>, delta_steps: f64) -> bool {
        let (x, y) = self.clamp_point_to_viewport(point);
        self.annotation
            .adjust_selected_by_wheel((x, y), delta_steps, self.viewport.selection, self.viewport.mode == DragMode::Idle)
    }

    pub(crate) fn select_annotation(&mut self, id: Option<u64>) -> bool {
        self.annotation.select(id)
    }

    pub(crate) fn begin_text_edit_selected(&mut self) -> bool {
        self.annotation.begin_text_edit_selected()
    }

    pub(crate) fn append_text_edit(&mut self, text: &str) -> bool {
        self.annotation.append_text_edit(text)
    }

    pub(crate) fn backspace_text_edit(&mut self) -> bool {
        self.annotation.backspace_text_edit()
    }

    pub(crate) fn insert_newline_text_edit(&mut self) -> bool {
        self.annotation.insert_newline_text_edit()
    }

    pub(crate) fn commit_text_edit(&mut self) -> bool {
        self.annotation.commit_text_edit()
    }

    pub(crate) fn cancel_text_edit(&mut self) -> bool {
        self.annotation.cancel_text_edit()
    }

    pub(crate) fn start_annotation_draw(&mut self, point: Point<Pixels>) -> bool {
        let point = self.clamp_point_to_viewport(point);
        self.annotation
            .start_draw(point, self.viewport.selection, self.viewport.mode == DragMode::Idle)
    }

    pub(crate) fn start_annotation_move(&mut self, id: u64, point: Point<Pixels>) -> bool {
        let point = self.clamp_point_to_viewport(point);
        self.annotation
            .start_move(id, point, self.viewport.selection, self.viewport.mode == DragMode::Idle)
    }

    pub(crate) fn has_active_annotation_interaction(&self) -> bool {
        self.annotation.has_active_interaction()
    }

    pub(crate) fn update_annotation_interaction(&mut self, point: Point<Pixels>) -> bool {
        let point = self.clamp_point_to_viewport(point);
        self.annotation.update_interaction(point, self.viewport.selection)
    }

    pub(crate) fn finish_annotation_interaction(&mut self) -> bool {
        self.annotation.finish_interaction(Self::MIN_SELECTION_SIZE)
    }

    pub(crate) fn delete_selected_annotation(&mut self) -> bool {
        self.annotation.delete_selected()
    }

    pub(crate) fn undo_annotation(&mut self) -> bool {
        self.annotation.undo()
    }

    pub(crate) fn redo_annotation(&mut self) -> bool {
        self.annotation.redo()
    }

    pub(crate) fn clear_annotation_state(&mut self) {
        self.annotation.clear();
    }

    pub(crate) fn cancel_annotation_interaction_state(&mut self) {
        self.annotation.cancel_interaction_state();
    }

    pub(crate) fn composed_background_source(&self) -> Option<String> {
        let scale = f64::from(active_monitor_scale()).max(1.0);
        self.annotation.composed_background_source(self.background_pixels.as_ref(), scale)
    }
}

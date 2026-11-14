use super::AnnotationEngine;

impl AnnotationEngine {
    pub fn select(&mut self, id: Option<u64>) -> bool {
        self.store.finish_edit_group();
        let next = id.filter(|item_id| self.store.visible_contains(*item_id));
        let selection_changed = self.selected_id != next;
        let previous_style = self.style;
        self.selected_id = next;
        let draft_cancelled = self.text_editing.as_ref().is_some_and(|state| Some(state.id) != self.selected_id);
        if draft_cancelled {
            self.text_editing = None;
        }
        self.sync_style_from_selected();
        let changed = selection_changed || draft_cancelled || previous_style != self.style;
        if changed {
            self.bump_transient();
        }
        changed
    }

    pub fn translate_all_annotations(&mut self, dx: f64, dy: f64) -> bool {
        if !self.store.translate_all_visible(dx, dy) {
            return false;
        }

        self.bump_committed();
        true
    }

    pub fn delete_selected(&mut self) -> bool {
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

    pub fn undo(&mut self) -> bool {
        let interaction_changed = self.cancel_interaction_state();
        if !self.store.undo() {
            return interaction_changed;
        }
        self.selected_id = self.selected_id.filter(|id| self.store.visible_contains(*id));
        self.sync_style_from_selected();
        self.bump_committed();
        true
    }

    pub fn redo(&mut self) -> bool {
        let interaction_changed = self.cancel_interaction_state();
        if !self.store.redo() {
            return interaction_changed;
        }
        self.selected_id = self.selected_id.filter(|id| self.store.visible_contains(*id));
        self.sync_style_from_selected();
        self.bump_committed();
        true
    }
}

use super::AnnotationEngine;

impl AnnotationEngine {
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

    pub(crate) fn translate_all_annotations(&mut self, dx: f64, dy: f64) -> bool {
        if !self.store.translate_all_visible(dx, dy) {
            return false;
        }

        self.bump_committed();
        true
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

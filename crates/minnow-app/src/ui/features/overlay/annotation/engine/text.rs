use super::super::model::{AnnotationKind, TEXT_DEFAULT, TextEditState};
use super::AnnotationEngine;

impl AnnotationEngine {
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
}

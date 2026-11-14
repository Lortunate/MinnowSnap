use super::super::model::{AnnotationKind, TEXT_DEFAULT, TextEditState};
use super::AnnotationEngine;

/// The same normalization is used for output validity and text commit, so an
/// editing caret or a trailing newline alone cannot invalidate an output job.
pub(super) fn output_text(draft: &str) -> &str {
    let next = draft.trim_end_matches('\n');
    if next.trim().is_empty() { TEXT_DEFAULT } else { next }
}

impl AnnotationEngine {
    pub fn begin_text_edit_selected(&mut self) -> bool {
        self.store.finish_edit_group();
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

    pub fn append_text_edit(&mut self, text: &str) -> bool {
        let Some(edit) = &mut self.text_editing else {
            return false;
        };
        edit.draft.push_str(text);
        self.bump_transient();
        true
    }

    pub fn backspace_text_edit(&mut self) -> bool {
        let Some(edit) = &mut self.text_editing else {
            return false;
        };
        let changed = edit.draft.pop().is_some();
        if changed {
            self.bump_transient();
        }
        changed
    }

    pub fn insert_newline_text_edit(&mut self) -> bool {
        self.append_text_edit("\n")
    }

    pub fn commit_text_edit(&mut self) -> bool {
        let Some(edit) = self.text_editing.take() else {
            return false;
        };
        let next = output_text(&edit.draft).to_string();
        let changed = self.store.update_item(edit.id, None, |item| {
            if let AnnotationKind::Text { text, .. } = &mut item.kind {
                *text = next;
            }
        });
        if changed {
            self.bump_committed();
        } else {
            self.bump_transient();
        }
        true
    }

    pub fn cancel_text_edit(&mut self) -> bool {
        let had = self.text_editing.is_some();
        self.text_editing = None;
        if had {
            self.bump_transient();
        }
        had
    }
}

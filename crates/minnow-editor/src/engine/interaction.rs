use minnow_imaging::geometry::RectF;

use super::super::model::{AnnotationInteractionState, AnnotationItem, AnnotationKind, AnnotationTool, TEXT_DEFAULT, TextEditState};
use super::super::ops::{annotation_item_large_enough, build_drawing_item, positive_finite};
use super::AnnotationEngine;

impl AnnotationEngine {
    /// Returns whether a pointer interaction or text draft was removed.
    pub fn cancel_interaction_state(&mut self) -> bool {
        self.store.finish_edit_group();
        let had_interaction = !matches!(self.interaction, AnnotationInteractionState::Idle);
        let had_text_edit = self.text_editing.is_some();
        self.interaction = AnnotationInteractionState::Idle;
        self.text_editing = None;
        let changed = had_interaction || had_text_edit;
        if changed {
            self.bump_transient();
        }
        changed
    }

    pub fn start_draw(&mut self, point: (f64, f64), selection: Option<RectF>, idle_mode: bool) -> bool {
        if !self.mode_enabled(selection, idle_mode) || !self.style.is_valid() {
            return false;
        }
        let Some(tool) = self.tool else {
            return false;
        };
        let Some(point) = self.clamp_to_selection(point, selection) else {
            return false;
        };
        self.store.finish_edit_group();
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

    pub fn start_move(&mut self, id: u64, point: (f64, f64), selection: Option<RectF>, idle_mode: bool) -> bool {
        if !self.mode_enabled(selection, idle_mode) {
            return false;
        }
        if !self.store.visible_contains(id) {
            return false;
        }
        let Some(point) = self.clamp_to_selection(point, selection) else {
            return false;
        };
        self.store.finish_edit_group();
        self.selected_id = Some(id);
        self.text_editing = None;
        self.interaction = AnnotationInteractionState::Moving {
            id,
            start: point,
            current: point,
        };
        self.sync_style_from_selected();
        self.bump_transient();
        true
    }

    pub fn update_interaction(&mut self, point: (f64, f64), selection: Option<RectF>) -> bool {
        let Some(next) = self.clamp_to_selection(point, selection) else {
            return false;
        };
        let valid = match &self.interaction {
            AnnotationInteractionState::Idle => false,
            AnnotationInteractionState::Drawing { tool, start, style, .. } => build_drawing_item(*tool, *start, next, *style, 0).is_some(),
            AnnotationInteractionState::Moving { id, start, .. } => self
                .store
                .visible_item(*id)
                .is_some_and(|item| item.can_move_by(next.0 - start.0, next.1 - start.1)),
        };
        if !valid {
            return false;
        }
        let interaction = std::mem::take(&mut self.interaction);
        match interaction {
            AnnotationInteractionState::Idle => {
                self.interaction = AnnotationInteractionState::Idle;
                false
            }
            AnnotationInteractionState::Drawing { tool, start, current, style } => {
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
            AnnotationInteractionState::Moving { id, start, current } => {
                let changed = current != next;
                self.interaction = AnnotationInteractionState::Moving { id, start, current: next };
                if changed {
                    self.bump_transient();
                }
                changed
            }
        }
    }

    /// Returns whether interaction state changed, including discarding a draft
    /// that is too small to commit or finishing a move without displacement.
    pub fn finish_interaction(&mut self, min_selection_size: f64) -> bool {
        if !positive_finite(min_selection_size) {
            return false;
        }
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
                true
            }
            AnnotationInteractionState::Moving { id, start, current, .. } => {
                let dx = current.0 - start.0;
                let dy = current.1 - start.1;
                if dx.abs() <= f64::EPSILON && dy.abs() <= f64::EPSILON {
                    self.bump_transient();
                    return true;
                }
                if !self.store.move_visible_item_by(id, dx, dy) {
                    self.bump_transient();
                    return true;
                }
                self.bump_committed();
                true
            }
        }
    }
}

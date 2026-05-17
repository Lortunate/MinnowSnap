use crate::services::geometry::RectF;

use super::super::model::{AnnotationInteractionState, AnnotationItem, AnnotationKind, AnnotationTool, TEXT_DEFAULT, TextEditState};
use super::super::ops::{annotation_item_large_enough, build_drawing_item};
use super::AnnotationEngine;

impl AnnotationEngine {
    pub(crate) fn cancel_interaction_state(&mut self) {
        let had_interaction = !matches!(self.interaction, AnnotationInteractionState::Idle);
        let had_text_edit = self.text_editing.is_some();
        self.interaction = AnnotationInteractionState::Idle;
        self.text_editing = None;
        if had_interaction || had_text_edit {
            self.bump_transient();
        }
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
                if !self.store.move_visible_item_by(id, dx, dy) {
                    self.bump_transient();
                    return false;
                }
                self.bump_committed();
                true
            }
        }
    }
}

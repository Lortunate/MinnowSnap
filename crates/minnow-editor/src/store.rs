use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::model::AnnotationItem;

/// Bounds retained undo and redo snapshots together, excluding the live document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoryBudget {
    pub max_steps: usize,
    /// Retained item buffers and owned text, excluding allocator/container
    /// overhead. This is independent from the live document's storage.
    pub max_bytes: usize,
}

impl Default for HistoryBudget {
    fn default() -> Self {
        Self {
            max_steps: 128,
            max_bytes: 8 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HistoryUsage {
    pub undo_steps: usize,
    pub redo_steps: usize,
    pub estimated_bytes: usize,
}

#[derive(Clone, Debug)]
struct Snapshot {
    items: Vec<AnnotationItem>,
    bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WheelEdit {
    Stroke,
    MosaicIntensity,
    Resize,
}

impl Snapshot {
    fn estimated_bytes(items: &[AnnotationItem]) -> usize {
        items
            .iter()
            .fold(items.len().saturating_mul(std::mem::size_of::<AnnotationItem>()), |bytes, item| {
                bytes.saturating_add(match &item.kind {
                    super::model::AnnotationKind::Text { text, .. } => text.len(),
                    _ => 0,
                })
            })
    }

    fn new(items: Vec<AnnotationItem>) -> Self {
        let bytes = items
            .iter()
            .fold(items.capacity().saturating_mul(std::mem::size_of::<AnnotationItem>()), |bytes, item| {
                let text_bytes = match &item.kind {
                    super::model::AnnotationKind::Text { text, .. } => text.capacity(),
                    _ => 0,
                };
                bytes.saturating_add(text_bytes)
            });
        Self { items, bytes }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AnnotationStore {
    items: Vec<AnnotationItem>,
    id_to_index: HashMap<u64, usize>,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    budget: HistoryBudget,
    wheel_edit: Option<(u64, WheelEdit, Instant)>,
    content_revision: u64,
}

impl AnnotationStore {
    pub(crate) fn content_revision(&self) -> u64 {
        self.content_revision
    }

    pub(crate) fn set_history_budget(&mut self, budget: HistoryBudget) {
        self.finish_edit_group();
        self.budget = budget;
        self.trim_history();
    }

    pub(crate) fn history_usage(&self) -> HistoryUsage {
        HistoryUsage {
            undo_steps: self.undo_stack.len(),
            redo_steps: self.redo_stack.len(),
            estimated_bytes: self.undo_stack.iter().chain(&self.redo_stack).map(|snapshot| snapshot.bytes).sum(),
        }
    }

    pub(crate) fn clear(&mut self) {
        if !self.items.is_empty() {
            self.content_revision = self.content_revision.saturating_add(1);
        }
        self.items = Vec::new();
        self.id_to_index = HashMap::new();
        self.undo_stack = Vec::new();
        self.redo_stack = Vec::new();
        self.finish_edit_group();
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub(crate) fn visible_len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn visible_items(&self) -> &[AnnotationItem] {
        &self.items
    }

    pub(crate) fn clone_visible_items(&self) -> Vec<AnnotationItem> {
        self.items.clone()
    }

    pub(crate) fn visible_index(&self, id: u64) -> Option<usize> {
        self.id_to_index.get(&id).copied()
    }

    pub(crate) fn visible_contains(&self, id: u64) -> bool {
        self.visible_index(id).is_some()
    }

    pub(crate) fn visible_item(&self, id: u64) -> Option<&AnnotationItem> {
        let index = self.visible_index(id)?;
        self.items.get(index)
    }

    /// All in-place document changes pass through this transaction boundary.
    /// Nearby events in one wheel gesture share a single undo step.
    pub(crate) fn update_item(&mut self, id: u64, wheel: Option<WheelEdit>, edit: impl FnOnce(&mut AnnotationItem)) -> bool {
        let Some(index) = self.visible_index(id) else {
            return false;
        };
        let mut next = self.items[index].clone();
        edit(&mut next);
        if !next.is_valid() || next == self.items[index] {
            return false;
        }
        let output_changed = !next.output_eq(&self.items[index]);
        let now = Instant::now();
        let coalesced = wheel.is_some_and(|kind| {
            self.wheel_edit.is_some_and(|(previous_id, previous_kind, last)| {
                previous_id == id && previous_kind == kind && now.duration_since(last) <= Duration::from_millis(350)
            })
        });
        if !coalesced {
            self.push_undo_snapshot();
        }
        self.items[index] = next;
        if output_changed {
            self.content_revision = self.content_revision.saturating_add(1);
        }
        self.wheel_edit = wheel.map(|kind| (id, kind, now));
        true
    }

    pub(crate) fn finish_edit_group(&mut self) {
        self.wheel_edit = None;
    }

    pub(crate) fn push(&mut self, item: AnnotationItem) {
        self.push_undo_snapshot();
        self.items.push(item);
        self.content_revision = self.content_revision.saturating_add(1);
        self.rebuild_visible_index();
    }

    pub(crate) fn remove_visible_by_id(&mut self, id: u64) -> bool {
        let Some(index) = self.visible_index(id) else {
            return false;
        };
        self.push_undo_snapshot();
        self.items.remove(index);
        self.content_revision = self.content_revision.saturating_add(1);
        self.rebuild_visible_index();
        true
    }

    pub(crate) fn move_visible_item_by(&mut self, id: u64, dx: f64, dy: f64) -> bool {
        self.update_item(id, None, |item| {
            item.move_by(dx, dy);
        })
    }

    pub(crate) fn translate_all_visible(&mut self, dx: f64, dy: f64) -> bool {
        if dx.abs() <= f64::EPSILON && dy.abs() <= f64::EPSILON {
            return false;
        }
        if self.items.is_empty() || !self.items.iter().all(|item| item.can_move_by(dx, dy)) {
            return false;
        }

        self.push_undo_snapshot();
        for item in &mut self.items {
            item.move_by(dx, dy);
        }
        self.content_revision = self.content_revision.saturating_add(1);
        true
    }

    pub(crate) fn undo(&mut self) -> bool {
        self.finish_edit_group();
        let Some(snapshot) = self.undo_stack.pop() else {
            return false;
        };
        self.record_restored_output(&snapshot.items);
        self.redo_stack.push(Snapshot::new(std::mem::take(&mut self.items)));
        self.items = snapshot.items;
        self.rebuild_visible_index();
        self.trim_history();
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        self.finish_edit_group();
        let Some(snapshot) = self.redo_stack.pop() else {
            return false;
        };
        self.record_restored_output(&snapshot.items);
        self.undo_stack.push(Snapshot::new(std::mem::take(&mut self.items)));
        self.items = snapshot.items;
        self.rebuild_visible_index();
        self.trim_history();
        true
    }

    fn rebuild_visible_index(&mut self) {
        self.id_to_index.clear();
        for index in 0..self.items.len() {
            let id = self.items[index].id;
            self.id_to_index.insert(id, index);
        }
    }

    fn record_restored_output(&mut self, items: &[AnnotationItem]) {
        if self.items.len() != items.len() || !self.items.iter().zip(items).all(|(before, after)| before.output_eq(after)) {
            self.content_revision = self.content_revision.saturating_add(1);
        }
    }

    fn push_undo_snapshot(&mut self) {
        self.finish_edit_group();
        self.redo_stack.clear();
        // A document larger than the retained-history budget cannot yield a
        // retained snapshot. Avoid cloning it merely to discard it afterwards.
        let bytes = Snapshot::estimated_bytes(&self.items);
        if self.budget.max_steps == 0 || bytes > self.budget.max_bytes {
            self.undo_stack.clear();
            return;
        }
        self.undo_stack.push(Snapshot::new(self.items.clone()));
        self.trim_history();
    }

    fn trim_history(&mut self) {
        loop {
            let usage = self.history_usage();
            if usage.undo_steps + usage.redo_steps <= self.budget.max_steps && usage.estimated_bytes <= self.budget.max_bytes {
                break;
            }
            if !self.undo_stack.is_empty() {
                self.undo_stack.remove(0);
            } else if !self.redo_stack.is_empty() {
                self.redo_stack.remove(0);
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AnnotationItem, AnnotationKind, AnnotationStyleState};

    fn item(id: u64, x: f64) -> AnnotationItem {
        AnnotationItem {
            id,
            style: AnnotationStyleState::default(),
            kind: AnnotationKind::Rectangle {
                rect: minnow_imaging::geometry::RectF::new(x, x, 10.0, 10.0),
            },
        }
    }

    #[test]
    fn id_index_stays_consistent_through_mutations() {
        let mut store = AnnotationStore::default();
        store.push(item(1, 1.0));
        store.push(item(2, 2.0));
        store.push(item(3, 3.0));

        assert_eq!(store.visible_index(2), Some(1));
        assert!(store.undo());
        assert_eq!(store.visible_index(3), None);
        assert!(store.redo());
        assert_eq!(store.visible_index(3), Some(2));
        assert!(store.remove_visible_by_id(2));
        assert_eq!(store.visible_index(1), Some(0));
        assert_eq!(store.visible_index(3), Some(1));
    }

    #[test]
    fn undo_redo_restores_translated_annotations_without_dropping_visibility() {
        let mut store = AnnotationStore::default();
        store.push(item(1, 1.0));
        store.push(item(2, 2.0));

        let before = store.clone_visible_items();
        assert!(store.translate_all_visible(10.0, 20.0));
        assert_eq!(store.visible_len(), 2);
        assert_ne!(store.clone_visible_items(), before);

        assert!(store.undo());
        assert_eq!(store.clone_visible_items(), before);
        assert_eq!(store.visible_len(), 2);

        assert!(store.redo());
        assert_eq!(store.visible_len(), 2);
        assert_eq!(store.visible_item(1).unwrap().bounds().x, 11.0);
        assert_eq!(store.visible_item(2).unwrap().bounds().x, 12.0);
    }

    #[test]
    fn step_budget_retains_recent_undo_and_redo() {
        let mut store = AnnotationStore::default();
        store.set_history_budget(HistoryBudget {
            max_steps: 2,
            max_bytes: usize::MAX,
        });
        for id in 1..=5 {
            store.push(item(id, id as f64));
        }
        assert_eq!(store.history_usage().undo_steps, 2);
        assert!(store.undo());
        assert_eq!(store.visible_len(), 4);
        assert!(store.undo());
        assert_eq!(store.visible_len(), 3);
        assert!(!store.undo());
        assert!(store.redo());
        assert_eq!(store.visible_len(), 4);
        assert!(store.redo());
        assert_eq!(store.visible_len(), 5);
    }

    #[test]
    fn byte_budget_includes_text_storage_and_does_not_remove_live_document() {
        let mut store = AnnotationStore::default();
        let mut text = item(1, 0.0);
        text.kind = super::super::model::AnnotationKind::Text {
            origin: (0.0, 0.0),
            text: "large text".repeat(200),
        };
        store.push(text);
        store.push(item(2, 2.0));
        assert!(store.history_usage().estimated_bytes >= 2_000);

        store.set_history_budget(HistoryBudget {
            max_steps: 128,
            max_bytes: 256,
        });
        assert!(store.history_usage().estimated_bytes <= 256);
        assert_eq!(store.visible_len(), 2);
        assert_eq!(store.visible_item(1).unwrap().id, 1);
    }
}

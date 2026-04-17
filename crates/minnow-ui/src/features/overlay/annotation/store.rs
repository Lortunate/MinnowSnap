use std::collections::HashMap;

use super::model::AnnotationItem;

#[derive(Clone, Debug, Default)]
pub(crate) struct AnnotationStore {
    items: Vec<AnnotationItem>,
    id_to_index: HashMap<u64, usize>,
    undo_stack: Vec<Vec<AnnotationItem>>,
    redo_stack: Vec<Vec<AnnotationItem>>,
}

impl AnnotationStore {
    pub(crate) fn clear(&mut self) {
        self.items.clear();
        self.id_to_index.clear();
        self.undo_stack.clear();
        self.redo_stack.clear();
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

    pub(crate) fn visible_item_mut(&mut self, id: u64) -> Option<&mut AnnotationItem> {
        let index = self.visible_index(id)?;
        self.items.get_mut(index)
    }

    pub(crate) fn push(&mut self, item: AnnotationItem) {
        self.push_undo_snapshot();
        self.items.push(item);
        self.rebuild_visible_index();
    }

    pub(crate) fn remove_visible_by_id(&mut self, id: u64) -> bool {
        let Some(index) = self.visible_index(id) else {
            return false;
        };
        self.push_undo_snapshot();
        self.items.remove(index);
        self.rebuild_visible_index();
        true
    }

    pub(crate) fn move_visible_item_by(&mut self, id: u64, dx: f64, dy: f64) -> bool {
        if dx.abs() <= f64::EPSILON && dy.abs() <= f64::EPSILON {
            return false;
        }
        let Some(index) = self.visible_index(id) else {
            return false;
        };
        self.push_undo_snapshot();
        self.items[index].move_by(dx, dy);
        true
    }

    pub(crate) fn translate_all_visible(&mut self, dx: f64, dy: f64) -> bool {
        if dx.abs() <= f64::EPSILON && dy.abs() <= f64::EPSILON {
            return false;
        }
        if self.items.is_empty() {
            return false;
        }

        self.push_undo_snapshot();
        for item in &mut self.items {
            item.move_by(dx, dy);
        }
        true
    }

    pub(crate) fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack.push(std::mem::take(&mut self.items));
        self.items = snapshot;
        self.rebuild_visible_index();
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push(std::mem::take(&mut self.items));
        self.items = snapshot;
        self.rebuild_visible_index();
        true
    }

    fn rebuild_visible_index(&mut self) {
        self.id_to_index.clear();
        for index in 0..self.items.len() {
            let id = self.items[index].id;
            self.id_to_index.insert(id, index);
        }
    }

    fn push_undo_snapshot(&mut self) {
        self.undo_stack.push(self.items.clone());
        self.redo_stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::overlay::annotation::model::{AnnotationItem, AnnotationKind, AnnotationStyleState};

    fn item(id: u64, x: f64) -> AnnotationItem {
        AnnotationItem {
            id,
            style: AnnotationStyleState::default(),
            kind: AnnotationKind::Rectangle {
                rect: minnow_core::geometry::RectF::new(x, x, 10.0, 10.0),
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
}

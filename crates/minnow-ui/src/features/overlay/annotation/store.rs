use std::collections::HashMap;

use super::model::AnnotationItem;

#[derive(Clone, Debug, Default)]
pub(crate) struct AnnotationStore {
    items: Vec<AnnotationItem>,
    visible_len: usize,
    id_to_index: HashMap<u64, usize>,
}

impl AnnotationStore {
    pub(crate) fn clear(&mut self) {
        self.items.clear();
        self.visible_len = 0;
        self.id_to_index.clear();
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.visible_len > 0
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.visible_len < self.items.len()
    }

    pub(crate) fn visible_len(&self) -> usize {
        self.visible_len
    }

    pub(crate) fn visible_items(&self) -> &[AnnotationItem] {
        &self.items[..self.visible_len]
    }

    pub(crate) fn clone_visible_items(&self) -> Vec<AnnotationItem> {
        self.visible_items().to_vec()
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
        self.truncate_redo();
        self.items.push(item);
        self.visible_len = self.items.len();
        self.rebuild_visible_index();
    }

    pub(crate) fn remove_visible_by_id(&mut self, id: u64) -> bool {
        let Some(index) = self.visible_index(id) else {
            return false;
        };
        self.truncate_redo();
        self.items.remove(index);
        self.visible_len = self.items.len();
        self.rebuild_visible_index();
        true
    }

    pub(crate) fn undo(&mut self) -> bool {
        if self.visible_len == 0 {
            return false;
        }
        self.visible_len -= 1;
        self.rebuild_visible_index();
        true
    }

    pub(crate) fn redo(&mut self) -> bool {
        if self.visible_len >= self.items.len() {
            return false;
        }
        self.visible_len += 1;
        self.rebuild_visible_index();
        true
    }

    pub(crate) fn truncate_redo(&mut self) {
        if self.visible_len < self.items.len() {
            self.items.truncate(self.visible_len);
        }
        self.rebuild_visible_index();
    }

    fn rebuild_visible_index(&mut self) {
        self.id_to_index.clear();
        for index in 0..self.visible_len {
            let id = self.items[index].id;
            self.id_to_index.insert(id, index);
        }
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
}

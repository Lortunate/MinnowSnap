use minnow_core::geometry::RectF;

use super::model::{AnnotationItem, AnnotationKindTag, AnnotationOutline};

#[derive(Clone, Copy, Debug)]
struct HitEntry {
    id: u64,
    bounds: RectF,
    kind: AnnotationKindTag,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AnnotationHitProxy {
    revision: u64,
    entries: Vec<HitEntry>,
}

impl AnnotationHitProxy {
    pub(crate) fn clear(&mut self) {
        self.revision = 0;
        self.entries.clear();
    }

    pub(crate) fn sync(&mut self, revision: u64, items: &[AnnotationItem]) {
        if self.revision == revision {
            return;
        }
        self.entries = items
            .iter()
            .map(|item| HitEntry {
                id: item.id,
                bounds: item.bounds(),
                kind: item.kind.tag(),
            })
            .collect();
        self.revision = revision;
    }

    pub(crate) fn hit_test(&self, point: (f64, f64), mut precise_hit: impl FnMut(u64, RectF, AnnotationKindTag) -> bool) -> Option<u64> {
        self.entries.iter().rev().find_map(|entry| {
            if !entry.bounds.contains_point(point.0, point.1) {
                return None;
            }
            if matches!(entry.kind, AnnotationKindTag::Text) {
                return Some(entry.id);
            }
            precise_hit(entry.id, entry.bounds, entry.kind).then_some(entry.id)
        })
    }

    pub(crate) fn outlines(&self, selected_id: Option<u64>) -> Vec<AnnotationOutline> {
        self.entries
            .iter()
            .map(|entry| AnnotationOutline {
                id: entry.id,
                bounds: entry.bounds,
                selected: Some(entry.id) == selected_id,
                transient: false,
            })
            .collect()
    }
}

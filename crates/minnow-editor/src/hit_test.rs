use minnow_imaging::geometry::RectF;

use super::model::{AnnotationItem, AnnotationKindTag, AnnotationOutline};

#[derive(Clone, Copy, Debug)]
struct HitEntry {
    id: u64,
    bounds: RectF,
    hit_bounds: RectF,
    kind: AnnotationKindTag,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AnnotationHitProxy {
    committed_revision: u64,
    entries: Vec<HitEntry>,
}

impl AnnotationHitProxy {
    pub(crate) fn clear(&mut self) {
        self.committed_revision = 0;
        self.entries.clear();
    }

    pub(crate) fn sync(&mut self, committed_revision: u64, items: &[AnnotationItem], font: Option<&ab_glyph::FontArc>) {
        if self.committed_revision == committed_revision {
            return;
        }
        self.entries = items
            .iter()
            .map(|item| HitEntry {
                id: item.id,
                bounds: item.bounds_with_font(font),
                hit_bounds: item.hit_bounds_with_font(font),
                kind: item.kind.tag(),
            })
            .collect();
        self.committed_revision = committed_revision;
    }

    pub(crate) fn hit_test(
        &self,
        point: (f64, f64),
        moving_item: Option<&AnnotationItem>,
        font: Option<&ab_glyph::FontArc>,
        mut precise_hit: impl FnMut(u64, RectF, AnnotationKindTag) -> bool,
    ) -> Option<u64> {
        self.entries.iter().rev().find_map(|entry| {
            let moving = moving_item.filter(|item| item.id == entry.id);
            let bounds = moving.map_or(entry.bounds, |item| item.bounds_with_font(font));
            let hit_bounds = moving.map_or(entry.hit_bounds, |item| item.hit_bounds_with_font(font));
            if !hit_bounds.contains_point(point.0, point.1) {
                return None;
            }
            if matches!(entry.kind, AnnotationKindTag::Text) {
                return Some(entry.id);
            }
            precise_hit(entry.id, bounds, entry.kind).then_some(entry.id)
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

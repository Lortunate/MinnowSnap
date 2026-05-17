use crate::services::geometry::RectF;

use super::super::model::{AnnotationKind, COLOR_PRESETS, MosaicMode};
use super::super::ops::ensure_mosaic_kind_style;
use super::AnnotationEngine;

impl AnnotationEngine {
    pub(crate) fn cycle_color(&mut self) -> bool {
        let current = self.style.stroke_color;
        let idx = COLOR_PRESETS.iter().position(|value| *value == current).unwrap_or(0);
        let next = COLOR_PRESETS[(idx + 1) % COLOR_PRESETS.len()];
        self.set_color(next)
    }

    pub(crate) fn set_color(&mut self, color: u32) -> bool {
        let color_rgb = color & 0xffffff00;
        let mut changed_style = false;
        if self.style.stroke_color != color {
            self.style.stroke_color = color;
            changed_style = true;
        }
        let style_fill = color_rgb | (self.style.fill_color & 0x000000ff);
        if self.style.fill_color != style_fill {
            self.style.fill_color = style_fill;
            changed_style = true;
        }

        let mut changed_item = false;
        if let Some(item) = self.selected_item_mut() {
            if item.style.stroke_color != color {
                item.style.stroke_color = color;
                changed_item = true;
            }
            let item_fill = color_rgb | (item.style.fill_color & 0x000000ff);
            if item.style.fill_color != item_fill {
                item.style.fill_color = item_fill;
                changed_item = true;
            }
        }

        if !changed_style && !changed_item {
            return false;
        }
        if changed_item {
            self.bump_committed();
        } else {
            self.bump_transient();
        }
        true
    }

    pub(crate) fn toggle_fill(&mut self) -> bool {
        let next_fill_enabled = !self.style.fill_enabled;
        let style_changed = self.style.fill_enabled != next_fill_enabled;
        self.style.fill_enabled = next_fill_enabled;
        let mut changed_item = false;
        if let Some(item) = self.selected_item_mut()
            && item.style.fill_enabled != next_fill_enabled
        {
            item.style.fill_enabled = next_fill_enabled;
            changed_item = true;
        }
        if changed_item {
            self.bump_committed();
        } else {
            self.bump_transient();
        }
        changed_item || style_changed
    }

    pub(crate) fn adjust_stroke(&mut self, delta: f64) -> bool {
        let next_stroke = (self.style.stroke_width + delta).clamp(1.0, 18.0);
        let next_text = (self.style.text_size + delta).clamp(12.0, 96.0);
        let next_counter = (self.style.counter_radius + delta * 4.0).clamp(10.0, 64.0);
        let style_changed = (self.style.stroke_width - next_stroke).abs() > f64::EPSILON
            || (self.style.text_size - next_text).abs() > f64::EPSILON
            || (self.style.counter_radius - next_counter).abs() > f64::EPSILON;
        self.style.stroke_width = next_stroke;
        self.style.text_size = next_text;
        self.style.counter_radius = next_counter;

        let had_selected = self.selected_id.is_some();
        let mut changed_item = false;
        if let Some(item) = self.selected_item_mut() {
            changed_item = item.resize_by_wheel(delta);
        }
        if changed_item {
            self.sync_style_from_selected();
            self.bump_committed();
            return true;
        }
        if had_selected {
            self.sync_style_from_selected();
            return false;
        }
        if style_changed {
            self.bump_transient();
            return true;
        }
        false
    }

    pub(crate) fn set_mosaic_mode(&mut self, mode: MosaicMode) -> bool {
        let style_changed = self.style.mosaic_mode != mode;
        self.style.mosaic_mode = mode;
        let had_selected = self.selected_id.is_some();
        let mut changed = false;
        let style = self.style;
        if let Some(item) = self.selected_item_mut() {
            let before = item.kind.clone();
            ensure_mosaic_kind_style(&mut item.kind, &style);
            changed = before != item.kind;
        }
        if changed {
            self.sync_style_from_selected();
            self.bump_committed();
            return true;
        }
        if had_selected {
            self.sync_style_from_selected();
            return false;
        }
        if style_changed {
            self.bump_transient();
            return true;
        }
        false
    }

    pub(crate) fn adjust_mosaic_intensity(&mut self, delta: f64) -> bool {
        let next_intensity = (self.style.mosaic_intensity + delta).clamp(2.0, 64.0);
        let style_changed = (self.style.mosaic_intensity - next_intensity).abs() > f64::EPSILON;
        self.style.mosaic_intensity = next_intensity;
        let had_selected = self.selected_id.is_some();
        let mut changed = false;
        if let Some(item) = self.selected_item_mut()
            && let AnnotationKind::Mosaic { intensity, .. } = &mut item.kind
            && (*intensity - next_intensity).abs() > f64::EPSILON
        {
            *intensity = next_intensity;
            item.style.mosaic_intensity = *intensity;
            changed = true;
        }
        if changed {
            self.sync_style_from_selected();
            self.bump_committed();
            return true;
        }
        if had_selected {
            self.sync_style_from_selected();
            return false;
        }
        if style_changed {
            self.bump_transient();
            return true;
        }
        false
    }

    pub(crate) fn adjust_selected_by_wheel(&mut self, point: (f64, f64), delta_steps: f64, selection: Option<RectF>, idle_mode: bool) -> bool {
        if !self.mode_enabled(selection, idle_mode) {
            return false;
        }
        let Some(selected_id) = self.selected_id else {
            return false;
        };
        let hit = self.hit_test(point, selection, idle_mode);
        if hit != Some(selected_id) {
            return false;
        }
        let Some(item) = self.selected_item_mut() else {
            return false;
        };
        let changed = item.resize_by_wheel(delta_steps);
        if changed {
            self.sync_style_from_selected();
            self.bump_committed();
        }
        changed
    }
}

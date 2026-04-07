use super::ocr_geometry::{bounds_from_points, compute_block_geometries, hit_test_block, point_in_bounds, point_to_char_index};
use super::{CloseAllPins, ClosePin, CopyPinContent, PinView, PointerMode, SavePinImage};
use crate::core::capture::action::CaptureAction;
use gpui::{Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollWheelEvent, Window, px};
use std::borrow::BorrowMut;
use std::collections::BTreeSet;

impl PinView {
    pub(super) fn on_action_close(&mut self, _: &ClosePin, window: &mut Window, cx: &mut Context<Self>) {
        let cleared = self.session.update(cx, |session, _| session.clear_ocr_selection());
        if cleared {
            self.pointer_mode = PointerMode::Idle;
            cx.notify();
            return;
        }
        Self::request_close(&self.manager, window, BorrowMut::borrow_mut(cx));
    }

    pub(super) fn on_action_close_all(&mut self, _: &CloseAllPins, _window: &mut Window, cx: &mut Context<Self>) {
        let manager = self.manager.clone();
        cx.defer(move |cx| {
            manager.close_all(cx);
        });
    }

    pub(super) fn on_action_copy_content(&mut self, _: &CopyPinContent, _window: &mut Window, cx: &mut Context<Self>) {
        Self::copy_selection_or_image(&self.session, BorrowMut::borrow_mut(cx));
    }

    pub(super) fn on_action_save_image(&mut self, _: &SavePinImage, _window: &mut Window, cx: &mut Context<Self>) {
        Self::run_capture_action(&self.session, CaptureAction::Save, BorrowMut::borrow_mut(cx));
    }

    pub(super) fn on_scroll_wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta_y = event.delta.pixel_delta(px(24.0)).y.to_f64() as f32;
        if delta_y.abs() < f32::EPSILON {
            return;
        }

        let step = if delta_y > 0.0 { 1.0 } else { -1.0 };
        let changed = if event.modifiers.alt {
            self.session.update(cx, |session, _| session.apply_opacity_step(step))
        } else if let Some(next_size) = self.session.update(cx, |session, _| session.apply_zoom_step(step)) {
            window.resize(next_size);
            true
        } else {
            false
        };

        if changed {
            cx.notify();
        }
    }

    pub(super) fn on_mouse_down_left(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.modifiers.control {
            return;
        }

        let frame = self.session.read(cx).frame();
        if frame.ocr.blocks.is_empty() {
            return;
        }

        let (_, geometries) = compute_block_geometries(&frame, window.viewport_size());
        if let Some(geometry) = hit_test_block(event.position, &geometries) {
            let text = &frame.ocr.blocks[geometry.index].text;
            let anchor = point_to_char_index(event.position, geometry, text);
            let changed = self.session.update(cx, |session, _| session.start_text_selection(geometry.index, anchor));
            self.pointer_mode = PointerMode::TextSelect { block_index: geometry.index };
            if changed {
                cx.notify();
            }
            cx.stop_propagation();
            return;
        }

        let base_selection = if event.modifiers.shift {
            frame.ocr.selected_indices
        } else {
            BTreeSet::new()
        };
        let changed = self.session.update(cx, |session, _| {
            let mut changed = session.start_selection_rect(event.position);
            if event.modifiers.shift {
                changed |= session.clear_active_text_selection();
            } else {
                changed |= session.set_selected_indices(BTreeSet::new());
            }
            changed
        });
        self.pointer_mode = PointerMode::BoxSelect {
            start: event.position,
            base_selection,
        };
        if changed {
            cx.notify();
        }
        cx.stop_propagation();
    }

    pub(super) fn on_mouse_down_right(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let frame = self.session.read(cx).frame();
        if frame.ocr.blocks.is_empty() {
            return;
        }

        let (_, geometries) = compute_block_geometries(&frame, window.viewport_size());
        if let Some(geometry) = hit_test_block(event.position, &geometries) {
            let changed = self.session.update(cx, |session, _| {
                if !session.is_block_selected(geometry.index) && session.active_text_block_index() != Some(geometry.index) {
                    session.set_single_selected_index(geometry.index)
                } else {
                    false
                }
            });
            if changed {
                cx.notify();
            }
        }
    }

    pub(super) fn on_mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        let frame = self.session.read(cx).frame();
        let (_, geometries) = compute_block_geometries(&frame, window.viewport_size());
        let mut changed = false;
        match &self.pointer_mode {
            PointerMode::Idle => {
                let hovered = hit_test_block(event.position, &geometries).map(|geometry| geometry.index);
                changed = self.session.update(cx, |session, _| session.set_hovered_block(hovered));
            }
            PointerMode::TextSelect { block_index } => {
                if let Some(geometry) = geometries.iter().find(|geometry| geometry.index == *block_index) {
                    let text = &frame.ocr.blocks[*block_index].text;
                    let head = point_to_char_index(event.position, geometry, text);
                    changed = self.session.update(cx, |session, _| session.update_text_selection_head(head));
                }
                cx.stop_propagation();
            }
            PointerMode::BoxSelect { start, base_selection } => {
                let mut next = base_selection.clone();
                let rect = bounds_from_points(*start, event.position);
                for geometry in &geometries {
                    if point_in_bounds(geometry.center, rect) {
                        next.insert(geometry.index);
                    }
                }
                changed = self.session.update(cx, |session, _| {
                    let mut changed = session.update_selection_rect(event.position);
                    changed |= session.set_selected_indices(next);
                    changed
                });
                cx.stop_propagation();
            }
        }

        if changed {
            cx.notify();
        }
    }

    pub(super) fn on_mouse_up_left(&mut self, _event: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let was_box_selecting = matches!(self.pointer_mode, PointerMode::BoxSelect { .. });
        self.pointer_mode = PointerMode::Idle;
        if was_box_selecting {
            let changed = self.session.update(cx, |session, _| session.clear_selection_rect());
            if changed {
                cx.notify();
            }
        }
    }
}

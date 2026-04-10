use super::ocr_geometry::{OcrBlockGeometry, paint_local_rect, point_in_rotated_rect, world_to_local};
use crate::features::pin::state::PinSession;
use gpui::{
    App, Bounds, CursorStyle, DispatchPhase, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, Size, Style, StyledText, TextLayout, Window, point, px,
    size,
};
use gpui_component::ActiveTheme as _;
use std::cell::Cell;
use std::rc::Rc;

#[derive(Default)]
struct OcrTextBlockState {
    mouse_down_anchor: Rc<Cell<Option<usize>>>,
}

fn block_center(geometry: &OcrBlockGeometry) -> Point<Pixels> {
    point(geometry.width / 2.0, geometry.height / 2.0)
}

fn scaled_point(point_value: Point<Pixels>, center: Point<Pixels>, scale: f32) -> Point<Pixels> {
    let dx = (point_value.x - center.x).to_f64() as f32;
    let dy = (point_value.y - center.y).to_f64() as f32;
    point(center.x + px(dx * scale), center.y + px(dy * scale))
}

fn clamp_utf8_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn text_layout_scale(layout: &TextLayout, block_width: Pixels) -> f32 {
    let Some(start) = layout.position_for_index(0) else {
        return 1.0;
    };
    let Some(end) = layout.position_for_index(layout.len()) else {
        return 1.0;
    };
    let content_width = (end.x - start.x).to_f64().abs() as f32;
    let max_width = (block_width.to_f64() as f32 * 0.95).max(1.0);
    if content_width <= 0.0 {
        1.0
    } else {
        (max_width / content_width).min(1.0)
    }
}

fn world_to_unscaled_text_position(position: Point<Pixels>, geometry: &OcrBlockGeometry, text_scale: f32) -> Point<Pixels> {
    let center = block_center(geometry);
    let local = world_to_local(position, geometry);
    let dx = (local.x - center.x).to_f64() as f32;
    let dy = (local.y - center.y).to_f64() as f32;
    point(center.x + px(dx / text_scale), center.y + px(dy / text_scale))
}

fn clamped_text_index_from_world_position(
    position: Point<Pixels>,
    geometry: &OcrBlockGeometry,
    text: &str,
    text_scale: f32,
    index_for_position: impl FnOnce(Point<Pixels>) -> usize,
) -> usize {
    let unscaled = world_to_unscaled_text_position(position, geometry, text_scale.max(0.0001));
    let index = index_for_position(unscaled);
    clamp_utf8_boundary(text, index)
}

fn text_index_at_world_position(position: Point<Pixels>, geometry: &OcrBlockGeometry, layout: &TextLayout, text: &str) -> usize {
    let scale = text_layout_scale(layout, geometry.width);
    clamped_text_index_from_world_position(position, geometry, text, scale, |unscaled| match layout.index_for_position(unscaled) {
        Ok(index) | Err(index) => index,
    })
}

pub(super) struct OcrTextBlock {
    id: ElementId,
    session: gpui::Entity<PinSession>,
    geometry: OcrBlockGeometry,
    text: SharedString,
    styled_text: StyledText,
    layout_size: Size<Pixels>,
}

impl OcrTextBlock {
    pub(super) fn new(
        id: impl Into<ElementId>,
        session: gpui::Entity<PinSession>,
        geometry: OcrBlockGeometry,
        text: impl Into<SharedString>,
        layout_size: Size<Pixels>,
    ) -> Self {
        let text = text.into();
        Self {
            id: id.into(),
            session,
            geometry,
            styled_text: StyledText::new(text.clone()),
            text,
            layout_size,
        }
    }

    fn text_bounds(&self, window: &Window) -> Bounds<Pixels> {
        let font_size = window.text_style().font_size.to_pixels(window.rem_size());
        let line_height = window.text_style().line_height.to_pixels(font_size.into(), window.rem_size());
        let y_offset = ((self.geometry.height.to_f64() as f32 - line_height.to_f64() as f32) / 2.0).max(0.0);
        Bounds::new(point(px(0.0), px(y_offset)), size(self.geometry.width, line_height))
    }

    fn text_scale(&self, layout: &TextLayout) -> f32 {
        text_layout_scale(layout, self.geometry.width)
    }

    fn selection_local_bounds(&self, layout: &TextLayout, range: std::ops::Range<usize>) -> Option<Bounds<Pixels>> {
        if range.is_empty() {
            return None;
        }
        let scale = self.text_scale(layout);
        let center = block_center(&self.geometry);
        let start = scaled_point(layout.position_for_index(range.start)?, center, scale);
        let end = scaled_point(layout.position_for_index(range.end)?, center, scale);
        let height = px(layout.line_height().to_f64() as f32 * scale);
        Some(Bounds::new(start, size(end.x - start.x, height)))
    }
}

impl IntoElement for OcrTextBlock {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for OcrTextBlock {
    type RequestLayoutState = ();
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.styled_text = StyledText::new(self.text.clone());
        let text_layout_id = self.styled_text.request_layout(global_id, inspector_id, window, cx).0;
        let layout_id = window.request_layout(
            Style {
                size: size(self.layout_size.width.into(), self.layout_size.height.into()),
                ..Default::default()
            },
            [text_layout_id],
            cx,
        );
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.styled_text
            .prepaint(global_id, inspector_id, self.text_bounds(window), &mut (), window, cx);
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(global_id) = global_id else {
            return;
        };
        let current_view = window.current_view();
        let text_layout = self.styled_text.layout().clone();
        let session_for_events = self.session.clone();
        let geometry_for_events = self.geometry.clone();
        let text_for_events = self.text.clone();
        let block_index = self.geometry.index;

        let mouse_position = window.mouse_position();
        let inside_rotated = bounds.contains(&mouse_position) && point_in_rotated_rect(mouse_position, &self.geometry);
        if inside_rotated {
            window.set_cursor_style(CursorStyle::IBeam, hitbox);
        }

        let active_selection = self.session.read(cx).frame().ocr.active_text.clone();
        if let Some(active_selection) = active_selection
            && active_selection.block_index == self.geometry.index
            && let Some(selection_bounds) = self.selection_local_bounds(&text_layout, active_selection.range())
        {
            paint_local_rect(window, &self.geometry, selection_bounds, cx.theme().primary.opacity(0.4));
        }

        window.with_element_state::<OcrTextBlockState, _>(global_id, |state, window| {
            let state = state.unwrap_or_default();
            let mouse_down_anchor = state.mouse_down_anchor.clone();

            window.on_mouse_event({
                let hitbox = hitbox.clone();
                let text_layout = text_layout.clone();
                let session = session_for_events.clone();
                let geometry = geometry_for_events.clone();
                let text = text_for_events.clone();
                let mouse_down_anchor = mouse_down_anchor.clone();
                move |event: &MouseDownEvent, phase: DispatchPhase, window, cx| {
                    if phase != DispatchPhase::Bubble || !hitbox.is_hovered(window) || !point_in_rotated_rect(event.position, &geometry) {
                        return;
                    }

                    match event.button {
                        MouseButton::Left => {
                            let anchor = text_index_at_world_position(event.position, &geometry, &text_layout, &text);
                            let changed = session.update(cx, |session, _| session.start_text_selection(block_index, anchor));
                            mouse_down_anchor.set(Some(anchor));
                            if changed {
                                cx.notify(current_view);
                            }
                            cx.stop_propagation();
                        }
                        MouseButton::Right => {
                            let changed = session.update(cx, |session, _| {
                                if !session.is_block_selected(block_index) && session.active_text_block_index() != Some(block_index) {
                                    session.set_single_selected_index(block_index)
                                } else {
                                    false
                                }
                            });
                            if changed {
                                cx.notify(current_view);
                            }
                        }
                        _ => {}
                    }
                }
            });

            window.on_mouse_event({
                let session = session_for_events.clone();
                let geometry = geometry_for_events.clone();
                let text = text_for_events.clone();
                let mouse_down_anchor = mouse_down_anchor.clone();
                let text_layout = text_layout.clone();
                move |event: &MouseMoveEvent, phase: DispatchPhase, _window, cx| {
                    let Some(_anchor) = mouse_down_anchor.get() else {
                        return;
                    };
                    if phase != DispatchPhase::Bubble {
                        return;
                    }

                    let head = text_index_at_world_position(event.position, &geometry, &text_layout, &text);
                    let changed = session.update(cx, |session, _| session.update_text_selection_head(head));
                    if changed {
                        cx.notify(current_view);
                    }
                    cx.stop_propagation();
                }
            });

            window.on_mouse_event({
                let session = session_for_events.clone();
                let geometry = geometry_for_events.clone();
                let text = text_for_events.clone();
                let mouse_down_anchor = mouse_down_anchor.clone();
                let text_layout = text_layout.clone();
                move |event: &MouseUpEvent, phase: DispatchPhase, _window, cx| {
                    let Some(_) = mouse_down_anchor.take() else {
                        return;
                    };
                    if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                        return;
                    }

                    let head = text_index_at_world_position(event.position, &geometry, &text_layout, &text);
                    let changed = session.update(cx, |session, _| session.update_text_selection_head(head));
                    if changed {
                        cx.notify(current_view);
                    }
                    cx.stop_propagation();
                }
            });

            ((), state)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::ocr_geometry::{compute_block_geometries, ocr_canvas_bounds};
    use super::*;
    use minnow_core::ocr::OcrBlock;
    use crate::features::pin::state::{PinFrame, PinOcrState};
    use std::path::PathBuf;

    fn test_geometry(angle_deg: f64) -> OcrBlockGeometry {
        let frame = PinFrame {
            image_path: PathBuf::from("pin.png"),
            opacity: 1.0,
            base_size: (320.0, 200.0),
            ocr: PinOcrState {
                processing: false,
                blocks: vec![OcrBlock {
                    text: "test".to_string(),
                    cx: 0.5,
                    cy: 0.5,
                    width: 0.3,
                    height: 0.2,
                    angle: angle_deg,
                    percentage_coordinates: true,
                }],
                selected_indices: Default::default(),
                hovered_index: None,
                active_text: None,
                selection_rect: None,
                last_error: None,
            },
        };
        let (_, geometries) = compute_block_geometries(&frame, ocr_canvas_bounds(size(px(320.0), px(240.0))));
        geometries.into_iter().next().expect("test geometry should exist")
    }

    #[test]
    fn clamped_text_index_handles_rotated_and_outside_positions() {
        let geometry = test_geometry(45.0);
        let text = "你好 Rust";
        let outside = point(px(-120.0), px(260.0));
        let index = clamped_text_index_from_world_position(outside, &geometry, text, 0.4, |unscaled| {
            assert!(unscaled.x.to_f64().is_finite());
            assert!(unscaled.y.to_f64().is_finite());
            text.len() + 8
        });

        assert_eq!(index, text.len());
        assert!(text.is_char_boundary(index));
    }

    #[test]
    fn clamped_text_index_rewinds_to_utf8_boundary() {
        let geometry = test_geometry(0.0);
        let text = "你好Rust";
        let index = clamped_text_index_from_world_position(point(px(10.0), px(10.0)), &geometry, text, 1.0, |_| 1);

        assert_eq!(index, 0);
        assert!(text.is_char_boundary(index));
    }
}

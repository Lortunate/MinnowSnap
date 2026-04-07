use super::PinView;
use super::ocr_geometry::{bounds_from_points, compute_block_geometries, paint_rotated_rect, paint_rotated_stroke, sub_geometry_by_ratio};
use crate::core::capture::action::CaptureAction;
use crate::core::i18n;
use crate::ui::pin::{actions::PIN_CONTEXT, render, session::PinFrame};
use gpui::InteractiveElement;
use gpui::{App, Context, IntoElement, MouseButton, ParentElement, Styled, Window, WindowControlArea, canvas, div, px, quad};
use gpui_component::ActiveTheme as _;
use gpui_component::menu::ContextMenuExt;
use std::borrow::BorrowMut;

impl PinView {
    fn render_ocr_overlay(frame: PinFrame, cx: &App) -> impl IntoElement {
        let frame_for_prepaint = frame.clone();
        let frame_for_paint = frame;
        let selection_color = cx.theme().primary;
        let selected_fill = selection_color.opacity(0.3);
        let hovered_fill = cx.theme().foreground.opacity(0.16);
        let selection_border = selection_color.opacity(0.45);
        let drag_fill = selection_color.opacity(0.1);

        canvas(
            move |bounds, _window, _cx| {
                let (_, geometries) = compute_block_geometries(&frame_for_prepaint, bounds.size);
                geometries
            },
            move |_bounds, geometries, window, _cx| {
                for geometry in &geometries {
                    let is_selected = frame_for_paint.ocr.selected_indices.contains(&geometry.index);
                    let is_hovered = frame_for_paint.ocr.hovered_index == Some(geometry.index);
                    let mut fill = gpui::transparent_black();
                    if is_selected {
                        fill = selected_fill;
                    } else if is_hovered {
                        fill = hovered_fill;
                    }
                    if fill.a > 0.0 {
                        paint_rotated_rect(window, geometry, fill, gpui::transparent_black());
                    }

                    if is_selected {
                        paint_rotated_stroke(window, geometry, selection_border, px(1.0));
                    }
                }

                if let Some(active_text) = frame_for_paint.ocr.active_text.as_ref()
                    && let Some(geometry) = geometries.iter().find(|geometry| geometry.index == active_text.block_index)
                {
                    let text = &frame_for_paint.ocr.blocks[active_text.block_index].text;
                    let char_count = text.chars().count();
                    let range = active_text.range();
                    if char_count > 0 && !range.is_empty() {
                        let start_ratio = (range.start as f32 / char_count as f32).clamp(0.0, 1.0);
                        let end_ratio = (range.end as f32 / char_count as f32).clamp(0.0, 1.0);
                        let highlight = sub_geometry_by_ratio(geometry, start_ratio, end_ratio);
                        paint_rotated_rect(window, &highlight, selection_color.opacity(0.4), gpui::transparent_black());
                    }
                }

                if let Some((start, current)) = frame_for_paint.ocr.selection_rect {
                    let bounds = bounds_from_points(start, current);
                    window.paint_quad(quad(bounds, px(2.0), drag_fill, px(1.0), selection_border, Default::default()));
                }
            },
        )
        .size_full()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
    }
}

impl gpui::Render for PinView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.handle_auto_ocr(cx);

        let copy_text = i18n::common::copy();
        let save_text = i18n::common::save();
        let ocr_text = i18n::common::ocr();
        let close_text = i18n::common::close();
        let close_all_text = i18n::pin::close_all();
        let manager = self.manager.clone();
        let close_manager = self.manager.clone();
        let session = self.session.clone();
        let frame = self.session.read(cx).frame();
        let show_close_all = self.manager.prune_closed(BorrowMut::borrow_mut(cx)).len() > 1;

        div()
            .id("pin-view")
            .track_focus(&self.focus_handle)
            .key_context(PIN_CONTEXT)
            .on_action(cx.listener(Self::on_action_close))
            .on_action(cx.listener(Self::on_action_close_all))
            .on_action(cx.listener(Self::on_action_copy_content))
            .on_action(cx.listener(Self::on_action_save_image))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down_left))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down_right))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up_left))
            .size_full()
            .context_menu(move |menu, _, _| {
                let mut menu = menu
                    .item(render::menu_item(copy_text.clone().into(), {
                        let session = session.clone();
                        move |_, _, cx| {
                            Self::copy_selection_or_image(&session, cx);
                        }
                    }))
                    .item(render::menu_item(save_text.clone().into(), {
                        let session = session.clone();
                        move |_, _, cx| {
                            Self::run_capture_action(&session, CaptureAction::Save, cx);
                        }
                    }))
                    .item(render::menu_item(ocr_text.clone().into(), {
                        let session = session.clone();
                        move |_, _, cx| {
                            Self::start_ocr(&session, cx);
                        }
                    }))
                    .separator()
                    .item(render::menu_item(close_text.clone().into(), {
                        let manager = close_manager.clone();
                        move |_, window, cx| {
                            Self::request_close(&manager, window, cx);
                        }
                    }));

                if show_close_all {
                    menu = menu.item(render::menu_item(close_all_text.clone().into(), {
                        let manager = manager.clone();
                        move |_, _, cx| {
                            let manager = manager.clone();
                            cx.defer(move |cx| {
                                manager.close_all(cx);
                            });
                        }
                    }));
                }
                menu
            })
            .child(
                div()
                    .size_full()
                    .relative()
                    .window_control_area(WindowControlArea::Drag)
                    .child(render::panel(frame.image_path.clone(), frame.opacity, cx).absolute().size_full())
                    .children((!frame.ocr.blocks.is_empty() || frame.ocr.processing).then(|| Self::render_ocr_overlay(frame.clone(), cx))),
            )
    }
}

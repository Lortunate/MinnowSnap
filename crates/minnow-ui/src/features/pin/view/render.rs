use super::PinView;
use super::ocr_geometry::{bounds_from_points, compute_block_geometries, paint_rotated_rect, paint_rotated_stroke, sub_geometry_by_ratio};
use crate::features::pin::{PIN_CONTEXT, render, state::PinFrame};
use crate::presentation::i18n;
use crate::windowing::{PopupDragBehavior, PopupDragRegionExt};
use gpui::InteractiveElement;
use gpui::prelude::FluentBuilder as _;
use gpui::{App, Context, IntoElement, MouseButton, ParentElement, Styled, Window, canvas, div, px, quad};
use gpui_component::button::Button;
use gpui_component::menu::ContextMenuExt;
use gpui_component::{ActiveTheme as _, Sizable};
use minnow_application::{ImageCommand, SaveDestination};

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
        let copy_text = i18n::common::copy();
        let save_text = i18n::common::save();
        let ocr_text = i18n::common::ocr();
        let close_text = i18n::common::close();
        let close_all_text = i18n::pin::close_all();
        let manager = self.manager.clone();
        let close_manager = self.manager.clone();
        let frame = self.session.read(cx).frame();
        let view = cx.entity().downgrade();
        let preview_reduced = frame.preview_reduced;
        let show_close_all = self.manager.live_count(cx) > 1;
        let ocr_available = self.services.recognition_availability().is_available();
        let ocr_processing = frame.ocr.processing;
        let show_output_status = frame.output_pending || frame.output_request_error.is_some() || frame.output_preflight_error.is_some();

        div()
            .id("pin-view")
            .track_focus(&self.focus_handle)
            .key_context(PIN_CONTEXT)
            .on_action(cx.listener(Self::on_action_close))
            .on_action(cx.listener(Self::on_action_close_all))
            .on_action(cx.listener(Self::on_action_copy_content))
            .on_action(cx.listener(Self::on_action_save_image))
            .on_scroll(cx.listener(Self::on_scroll))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down_left))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down_right))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up_left))
            .size_full()
            .context_menu(move |menu, _, _| {
                let mut menu = menu
                    .item(render::menu_item(copy_text.clone().into(), {
                        let view = view.clone();
                        move |_, _, cx| {
                            let _ = view.update(cx, |view, cx| view.copy_selection_or_image(cx));
                        }
                    }))
                    .item(render::menu_item(save_text.clone().into(), {
                        let view = view.clone();
                        move |_, _, cx| {
                            let _ = view.update(cx, |view, cx| {
                                view.run_image_command(
                                    ImageCommand::Save {
                                        destination: SaveDestination::Preferences,
                                    },
                                    cx,
                                )
                            });
                        }
                    }));
                if ocr_available && !ocr_processing {
                    menu = menu.item(render::menu_item(ocr_text.clone().into(), {
                        let view = view.clone();
                        move |_, _, cx| {
                            let _ = view.update(cx, |view, cx| view.start_ocr(cx));
                        }
                    }));
                }
                menu = menu.separator().item(render::menu_item(close_text.clone().into(), {
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
                    .popup_drag_region(PopupDragBehavior::HitTest)
                    .child(render::panel(frame.image.clone(), frame.opacity, cx).absolute().size_full())
                    .when(preview_reduced && !show_output_status, |panel| {
                        panel.child(
                            div()
                                .absolute()
                                .top_2()
                                .left_2()
                                .right_2()
                                .rounded(cx.theme().radius)
                                .bg(cx.theme().popover)
                                .text_color(cx.theme().popover_foreground)
                                .px_2()
                                .py_1()
                                .text_sm()
                                .child(i18n::overlay::preview_reduced()),
                        )
                    })
                    .when(show_output_status, |panel| {
                        let mut status = div()
                            .absolute()
                            .top_2()
                            .left_2()
                            .right_2()
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().popover)
                            .text_color(cx.theme().popover_foreground)
                            .px_2()
                            .py_1()
                            .text_sm()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
                        if frame.output_pending {
                            status = status.child(i18n::output::processing());
                        }
                        if let Some(error) = &frame.output_request_error {
                            let mut request_error = div()
                                .id("pin-output-request-error")
                                .flex()
                                .flex_col()
                                .gap_1()
                                .rounded(cx.theme().radius)
                                .border_1()
                                .border_color(cx.theme().border)
                                .px_2()
                                .py_1()
                                .child(div().line_clamp(3).child(crate::flows::output::error_message(error)));
                            if frame.output_can_retry {
                                request_error = request_error.child(
                                    Button::new("pin-retry-output")
                                        .small()
                                        .outline()
                                        .label(i18n::output::retry())
                                        .on_click(cx.listener(|view, _, _, cx| view.retry_output(cx))),
                                );
                            }
                            status = status.child(request_error);
                        }
                        if let Some(error) = &frame.output_preflight_error {
                            status = status.child(
                                div()
                                    .id("pin-output-preflight-error")
                                    .rounded(cx.theme().radius)
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .px_2()
                                    .py_1()
                                    .line_clamp(3)
                                    .child(crate::flows::output::error_message(error)),
                            );
                        }
                        panel.child(status)
                    })
                    .children((!frame.ocr.blocks.is_empty() || frame.ocr.processing).then(|| Self::render_ocr_overlay(frame.clone(), cx)))
                    .when(frame.ocr.processing || frame.ocr.last_error.is_some(), |panel| {
                        let mut status = div()
                            .absolute()
                            .left_2()
                            .right_2()
                            .bottom_2()
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().popover)
                            .text_color(cx.theme().popover_foreground)
                            .px_2()
                            .py_1()
                            .text_sm()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
                        if frame.ocr.processing {
                            status = status.child(i18n::pin::ocr_processing());
                        } else if let Some(error) = &frame.ocr.last_error {
                            status = status.child(div().line_clamp(3).child(i18n::pin::ocr_failed(error)));
                            if !frame.ocr.blocks.is_empty() {
                                status = status.child(i18n::pin::ocr_previous_result());
                            }
                            if ocr_available {
                                status = status.child(
                                    Button::new("pin-retry-ocr")
                                        .small()
                                        .outline()
                                        .label(i18n::pin::ocr_retry())
                                        .on_click(cx.listener(|view, _, _, cx| view.start_ocr(cx))),
                                );
                            }
                        }
                        panel.child(status)
                    }),
            )
    }
}

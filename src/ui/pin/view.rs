use super::{
    actions::{CloseAllPins, ClosePin, PIN_CONTEXT},
    render,
    session::{PinManager, PinSession},
};
use crate::core::i18n;
use gpui::InteractiveElement;
use gpui::{App, Context, Entity, FocusHandle, IntoElement, ParentElement, ScrollWheelEvent, Styled, Subscription, Window, div, px};
use gpui_component::menu::ContextMenuExt;
use std::borrow::BorrowMut;

pub(super) struct PinView {
    session: Entity<PinSession>,
    manager: PinManager,
    focus_handle: FocusHandle,
    _session_observer: Subscription,
}

impl PinView {
    pub(super) fn new(session: Entity<PinSession>, manager: PinManager, focus_handle: FocusHandle, cx: &mut Context<Self>) -> Self {
        let observer = cx.observe(&session, |_, _, cx| {
            cx.notify();
        });
        Self {
            session,
            manager,
            focus_handle,
            _session_observer: observer,
        }
    }

    fn request_close(manager: &PinManager, window: &mut Window, cx: &mut App) {
        manager.unregister(Window::window_handle(window).window_id(), cx);
        window.defer(cx, |window, _| {
            window.remove_window();
        });
    }

    fn on_action_close(&mut self, _: &ClosePin, window: &mut Window, cx: &mut Context<Self>) {
        Self::request_close(&self.manager, window, BorrowMut::borrow_mut(cx));
    }

    fn on_action_close_all(&mut self, _: &CloseAllPins, _window: &mut Window, cx: &mut Context<Self>) {
        let manager = self.manager.clone();
        cx.defer(move |cx| {
            manager.close_all(cx);
        });
    }

    fn on_scroll_wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
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
}

impl gpui::Render for PinView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close_text = i18n::common::close();
        let close_all_text = i18n::pin::close_all();
        let manager = self.manager.clone();
        let close_manager = self.manager.clone();
        let (image_path, opacity) = self.session.read(cx).frame();

        div()
            .id("pin-view")
            .track_focus(&self.focus_handle)
            .key_context(PIN_CONTEXT)
            .on_action(cx.listener(Self::on_action_close))
            .on_action(cx.listener(Self::on_action_close_all))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .size_full()
            .context_menu(move |menu, _, _| {
                menu.item(render::menu_item(close_text.clone().into(), {
                    let manager = close_manager.clone();
                    move |_, window, cx| {
                        Self::request_close(&manager, window, cx);
                    }
                }))
                .separator()
                .item(render::menu_item(close_all_text.clone().into(), {
                    let manager = manager.clone();
                    move |_, _, cx| {
                        let manager = manager.clone();
                        cx.defer(move |cx| {
                            manager.close_all(cx);
                        });
                    }
                }))
            })
            .child(render::panel(image_path, opacity, cx))
    }
}

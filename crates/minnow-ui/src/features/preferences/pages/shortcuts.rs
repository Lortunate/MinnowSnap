use super::{ShortcutsPageActions, components};
use crate::features::preferences::{state::frame::ShortcutsPageProps, view::PreferencesView};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::v_flex;

pub(super) fn render(props: &ShortcutsPageProps, actions: ShortcutsPageActions, cx: &mut Context<PreferencesView>) -> AnyElement {
    let footer = components::surface_card(
        div()
            .px_4()
            .py_3()
            .child(div().flex().items_center().justify_end().child(components::secondary_button(
                &props.restore_defaults,
                cx.listener(actions.restore_defaults),
            ))),
        cx,
    );

    v_flex()
        .w_full()
        .min_w(px(0.))
        .gap_4()
        .children(props.recording_notice.as_ref().map(|notice| components::notice_banner(notice, cx)))
        .children(props.conflict_notice.as_ref().map(|notice| components::notice_banner(notice, cx)))
        .child(components::setting_section(
            [
                components::setting_action(&props.capture, cx.listener(actions.record_capture), cx),
                components::setting_action(&props.quick_capture, cx.listener(actions.record_quick_capture), cx),
            ],
            cx,
        ))
        .child(footer)
        .into_any_element()
}

use super::components;
use crate::features::preferences::{state::frame::ShortcutsPageProps, view::PreferencesView};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::v_flex;

pub(super) fn render(props: &ShortcutsPageProps, cx: &mut Context<PreferencesView>) -> AnyElement {
    let footer = components::surface_card(
        div().px_4().py_3().child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .children(
                    props
                        .retry
                        .as_ref()
                        .map(|retry| components::secondary_button(retry, cx.listener(PreferencesView::on_retry_shortcuts))),
                )
                .child(components::secondary_button(
                    &props.restore_defaults,
                    cx.listener(PreferencesView::on_restore_default_shortcuts),
                )),
        ),
        cx,
    );

    v_flex()
        .w_full()
        .min_w(px(0.))
        .gap_4()
        .children(props.recording_notice.as_ref().map(|notice| components::notice_banner(notice, cx)))
        .children(props.conflict_notice.as_ref().map(|notice| components::notice_banner(notice, cx)))
        .children(props.activation_notice.as_ref().map(|notice| components::notice_banner(notice, cx)))
        .child(components::setting_section(
            [
                components::setting_action(&props.capture, cx.listener(PreferencesView::on_capture_shortcut_record), cx),
                components::setting_action(&props.quick_capture, cx.listener(PreferencesView::on_quick_shortcut_record), cx),
            ],
            cx,
        ))
        .child(footer)
        .into_any_element()
}

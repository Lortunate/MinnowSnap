use super::components;
use crate::features::preferences::{state::frame::OcrPageProps, view::PreferencesView};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{ActiveTheme as _, v_flex};

pub(super) fn render(props: &OcrPageProps, cx: &mut Context<PreferencesView>) -> AnyElement {
    let note = props.note.as_ref().map(|note| {
        components::surface_card(
            div()
                .px_4()
                .py_3()
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child(note.clone())),
            cx,
        )
    });

    v_flex()
        .w_full()
        .min_w(px(0.))
        .gap_4()
        .child(components::setting_section(
            [components::setting_toggle(&props.enabled, PreferencesView::on_ocr_enabled_changed, cx)],
            cx,
        ))
        .child(components::notice_banner(&props.runtime_notice, cx))
        .children(props.model.as_ref().map(|model| {
            components::setting_section(
                [components::setting_action(
                    model,
                    cx.listener(PreferencesView::on_download_ocr_models),
                    cx,
                )],
                cx,
            )
        }))
        .children(note)
        .into_any_element()
}

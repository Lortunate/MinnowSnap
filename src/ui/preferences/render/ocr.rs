use super::{OcrPageActions, components};
use crate::ui::preferences::{session::frame::OcrPageProps, view::PreferencesView};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{ActiveTheme as _, v_flex};

pub(super) fn render(props: &OcrPageProps, actions: OcrPageActions, cx: &mut Context<PreferencesView>) -> AnyElement {
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
            [components::setting_toggle(&props.enabled, actions.enabled, cx)],
            cx,
        ))
        .children(
            props
                .show_model
                .then(|| components::setting_section([components::setting_action(&props.model, cx.listener(actions.download_models), cx)], cx)),
        )
        .children(note)
        .into_any_element()
}

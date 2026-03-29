use super::{GeneralPageActions, components};
use crate::ui::preferences::{session::frame::GeneralPageProps, view::PreferencesView};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, px};
use gpui_component::v_flex;

pub(super) fn render(props: &GeneralPageProps, actions: GeneralPageActions, cx: &mut Context<PreferencesView>) -> AnyElement {
    v_flex()
        .w_full()
        .min_w(px(0.))
        .gap_4()
        .child(components::setting_section(
            [components::setting_toggle(&props.auto_start, actions.auto_start, cx)],
            cx,
        ))
        .child(components::setting_section(
            [
                components::setting_dropdown(&props.language, actions.language, cx),
                components::setting_dropdown(&props.theme, actions.theme, cx),
                components::setting_dropdown(&props.font, actions.font, cx),
            ],
            cx,
        ))
        .child(components::setting_section(
            [
                components::setting_action(&props.save_path, cx.listener(actions.browse_save_path), cx),
                components::setting_toggle(&props.image_compression, actions.image_compression, cx),
            ],
            cx,
        ))
        .into_any_element()
}

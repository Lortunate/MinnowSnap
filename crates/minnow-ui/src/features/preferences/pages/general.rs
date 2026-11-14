use super::components;
use crate::features::preferences::{state::frame::GeneralPageProps, view::PreferencesView};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, px};
use gpui_component::v_flex;

pub(super) fn render(props: &GeneralPageProps, cx: &mut Context<PreferencesView>) -> AnyElement {
    v_flex()
        .w_full()
        .min_w(px(0.))
        .gap_4()
        .child(components::setting_section(
            [
                components::setting_toggle(&props.auto_start, PreferencesView::on_auto_start_changed, cx),
                components::setting_action(&props.refresh_native, cx.listener(PreferencesView::on_refresh_native), cx),
            ],
            cx,
        ))
        .child(components::setting_section(
            [
                components::setting_dropdown(&props.language, PreferencesView::on_language_selected, cx),
                components::setting_dropdown(&props.theme, PreferencesView::on_theme_selected, cx),
                components::setting_dropdown(&props.font, PreferencesView::on_font_selected, cx),
            ],
            cx,
        ))
        .child(components::setting_section(
            [
                components::setting_action(&props.save_path, cx.listener(PreferencesView::on_browse_save_path), cx),
                components::setting_toggle(&props.image_compression, PreferencesView::on_image_compression_changed, cx),
            ],
            cx,
        ))
        .into_any_element()
}

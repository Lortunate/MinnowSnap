use super::{NotificationsPageActions, components};
use crate::features::preferences::{state::frame::NotificationsPageProps, view::PreferencesView};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, px};
use gpui_component::v_flex;

pub(super) fn render(props: &NotificationsPageProps, actions: NotificationsPageActions, cx: &mut Context<PreferencesView>) -> AnyElement {
    v_flex()
        .w_full()
        .min_w(px(0.))
        .gap_4()
        .child(components::setting_section(
            [
                components::setting_toggle(&props.enabled, actions.enabled, cx),
                components::setting_toggle(&props.save_notification, actions.save_notification, cx),
                components::setting_toggle(&props.copy_notification, actions.copy_notification, cx),
                components::setting_toggle(&props.qr_code_notification, actions.qr_code_notification, cx),
            ],
            cx,
        ))
        .child(components::setting_section(
            [components::setting_toggle(&props.shutter_sound, actions.shutter_sound, cx)],
            cx,
        ))
        .into_any_element()
}

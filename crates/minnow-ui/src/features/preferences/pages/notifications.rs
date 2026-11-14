use super::components;
use crate::features::preferences::{state::frame::NotificationsPageProps, view::PreferencesView};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, px};
use gpui_component::v_flex;

pub(super) fn render(props: &NotificationsPageProps, cx: &mut Context<PreferencesView>) -> AnyElement {
    v_flex()
        .w_full()
        .min_w(px(0.))
        .gap_4()
        .child(components::setting_section(
            [
                components::setting_toggle(&props.enabled, PreferencesView::on_notifications_enabled_changed, cx),
                components::setting_toggle(&props.save_notification, PreferencesView::on_save_notification_changed, cx),
                components::setting_toggle(&props.copy_notification, PreferencesView::on_copy_notification_changed, cx),
                components::setting_toggle(&props.qr_code_notification, PreferencesView::on_qr_notification_changed, cx),
            ],
            cx,
        ))
        .child(components::setting_section(
            [components::setting_toggle(
                &props.shutter_sound,
                PreferencesView::on_shutter_sound_changed,
                cx,
            )],
            cx,
        ))
        .into_any_element()
}

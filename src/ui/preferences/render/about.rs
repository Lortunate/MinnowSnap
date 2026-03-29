use super::{AboutPageActions, components};
use crate::{
    core::system,
    ui::preferences::{session::frame::AboutPageProps, view::PreferencesView},
};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, img, px};
use gpui_component::{ActiveTheme as _, v_flex};

pub(super) fn render(props: &AboutPageProps, actions: AboutPageActions, cx: &mut Context<PreferencesView>) -> AnyElement {
    let brand_card = components::surface_card(
        div().px_5().py_4().child(
            v_flex()
                .w_full()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .min_w(px(0.))
                        .child(img("resources/logo.png").w(px(56.)).h(px(56.)))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w(px(0.))
                                .gap_1()
                                .child(props.app_name.clone())
                                .child(div().text_sm().text_color(cx.theme().muted_foreground).child(props.version_label.clone())),
                        ),
                )
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child(props.summary.clone())),
        ),
        cx,
    );

    let log_directory = props.log_directory_path.clone();

    v_flex()
        .w_full()
        .min_w(px(0.))
        .gap_4()
        .child(brand_card)
        .child(components::setting_section(
            [
                components::setting_action(&props.github_repository, actions.open_repository, cx),
                components::setting_action(&props.report_issue, actions.report_issue, cx),
                components::setting_action(
                    &props.open_logs,
                    move |_, _, cx| {
                        system::open_in_file_manager(cx, &log_directory);
                    },
                    cx,
                ),
            ],
            cx,
        ))
        .into_any_element()
}

use gpui::{App, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled, Window, div, px};
use gpui_component::{
    ActiveTheme as _, Icon, IconNamed, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
};
use std::rc::Rc;

use crate::core::capture::action::CaptureAction;
use crate::core::i18n;
use crate::ui::overlay::render::layout::OverlayPanelLayout;
use crate::ui::overlay::session::OverlayCommand;

#[derive(Clone, Copy)]
enum ToolbarIcon {
    QrCode,
    Save,
    Pin,
    Copy,
    Cancel,
}

impl IconNamed for ToolbarIcon {
    fn path(self) -> gpui::SharedString {
        match self {
            Self::QrCode => "resources/icons/crop_free.svg",
            Self::Save => "resources/icons/save.svg",
            Self::Pin => "resources/icons/keep.svg",
            Self::Copy => "resources/icons/file_copy.svg",
            Self::Cancel => "resources/icons/close.svg",
        }
        .into()
    }
}

struct ToolbarActionSpec {
    id: &'static str,
    icon: ToolbarIcon,
    tooltip: fn() -> String,
    command: OverlayCommand,
}

const TOOLBAR_ACTIONS: [ToolbarActionSpec; 5] = [
    ToolbarActionSpec {
        id: "overlay-qr",
        icon: ToolbarIcon::QrCode,
        tooltip: i18n::common::scan_qr,
        command: OverlayCommand::Capture(CaptureAction::QrCode),
    },
    ToolbarActionSpec {
        id: "overlay-save",
        icon: ToolbarIcon::Save,
        tooltip: i18n::common::save,
        command: OverlayCommand::Capture(CaptureAction::Save),
    },
    ToolbarActionSpec {
        id: "overlay-pin",
        icon: ToolbarIcon::Pin,
        tooltip: i18n::common::pin,
        command: OverlayCommand::Capture(CaptureAction::Pin),
    },
    ToolbarActionSpec {
        id: "overlay-copy",
        icon: ToolbarIcon::Copy,
        tooltip: i18n::common::copy,
        command: OverlayCommand::Capture(CaptureAction::Copy),
    },
    ToolbarActionSpec {
        id: "overlay-cancel",
        icon: ToolbarIcon::Cancel,
        tooltip: i18n::common::cancel,
        command: OverlayCommand::Close,
    },
];

pub(crate) fn overlay_toolbar_action_count() -> usize {
    TOOLBAR_ACTIONS.len()
}

pub(crate) fn overlay_toolbar(
    app_ctx: &App,
    layout: OverlayPanelLayout,
    on_action: Rc<dyn Fn(OverlayCommand, &mut Window, &mut App)>,
) -> impl IntoElement {
    let theme = app_ctx.theme();

    let mut row = h_flex().items_center().gap_0p5();
    for action in TOOLBAR_ACTIONS.iter() {
        row = row.child(toolbar_action_button(
            app_ctx,
            action.id,
            action.icon,
            (action.tooltip)(),
            action.command.clone(),
            on_action.clone(),
        ));
    }

    let mut panel = div()
        .absolute()
        .left(px(layout.x as f32))
        .top(px(layout.y as f32))
        .w(px(layout.width as f32))
        .h(px(layout.height as f32))
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme.radius_lg)
        .border_1()
        .border_color(theme.border)
        .bg(theme.popover)
        .overflow_hidden()
        .px_2()
        .py_1()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());
    if theme.shadow {
        panel = panel.shadow_lg();
    }
    panel.child(row)
}

fn toolbar_action_button(
    app_ctx: &App,
    id: &'static str,
    icon_name: ToolbarIcon,
    tooltip: String,
    command: OverlayCommand,
    on_action: Rc<dyn Fn(OverlayCommand, &mut Window, &mut App)>,
) -> Button {
    Button::new(id)
        .ghost()
        .compact()
        .icon(toolbar_icon(app_ctx, icon_name))
        .tooltip(tooltip)
        .on_click(move |_, window: &mut Window, cx: &mut App| {
            on_action(command.clone(), window, cx);
        })
}

fn toolbar_icon(app_ctx: &App, icon_name: ToolbarIcon) -> Icon {
    let theme = app_ctx.theme();
    Icon::new(icon_name).small().text_color(theme.popover_foreground)
}

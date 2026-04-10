use crate::features::overlay::render::OverlayActionHandler;
use crate::features::overlay::render::layout::OverlayPanelLayout;
use crate::features::overlay::state::{AnnotationCommand, AnnotationTool, CaptureCommand, LifecycleCommand, OverlayCommand};
use gpui::{
    AnyElement, App, AppContext, Corner, Div, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, PathPromptOptions, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable, Icon, IconNamed, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    popover::Popover,
};
use minnow_assets::asset_paths;
use minnow_core::capture::action::CaptureAction;
use minnow_core::i18n;
use minnow_core::ocr::service;
use std::time::Duration;

#[derive(Clone, Copy)]
pub(crate) struct OverlayToolbarState {
    pub tool: Option<AnnotationTool>,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum ToolbarIcon {
    Arrow,
    Rectangle,
    Circle,
    Counter,
    Text,
    Mosaic,
    Undo,
    Redo,
    Ocr,
    QrCode,
    Scroll,
    Save,
    Pin,
    Copy,
    Cancel,
}

impl IconNamed for ToolbarIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Arrow => asset_paths::icons::ARROW_INSERT,
            Self::Rectangle => asset_paths::icons::SQUARE,
            Self::Circle => asset_paths::icons::CIRCLE,
            Self::Counter => asset_paths::icons::COUNTER_1,
            Self::Text => asset_paths::icons::TEXT_FIELDS,
            Self::Mosaic => asset_paths::icons::BLUR_ON,
            Self::Undo => asset_paths::icons::UNDO,
            Self::Redo => asset_paths::icons::REDO,
            Self::Ocr => asset_paths::icons::TEXT_FIELDS,
            Self::QrCode => asset_paths::icons::CROP_FREE,
            Self::Scroll => asset_paths::icons::SCROLL,
            Self::Save => asset_paths::icons::SAVE,
            Self::Pin => asset_paths::icons::KEEP,
            Self::Copy => asset_paths::icons::FILE_COPY,
            Self::Cancel => asset_paths::icons::CLOSE,
        }
        .into()
    }
}

#[derive(Clone, Copy)]
struct ToolbarButtonSpec {
    id: &'static str,
    icon: ToolbarIcon,
    tooltip: fn() -> String,
    command: fn() -> OverlayCommand,
    active: fn(OverlayToolbarState) -> bool,
    disabled: fn(OverlayToolbarState) -> bool,
    visible: fn() -> bool,
}

const fn always_inactive(_: OverlayToolbarState) -> bool {
    false
}

const fn never_disabled(_: OverlayToolbarState) -> bool {
    false
}

const fn always_visible() -> bool {
    true
}

fn ocr_enabled() -> bool {
    service::is_enabled()
}

fn is_arrow_active(state: OverlayToolbarState) -> bool {
    state.tool == Some(AnnotationTool::Arrow)
}

fn is_rectangle_active(state: OverlayToolbarState) -> bool {
    state.tool == Some(AnnotationTool::Rectangle)
}

fn is_circle_active(state: OverlayToolbarState) -> bool {
    state.tool == Some(AnnotationTool::Circle)
}

fn is_counter_active(state: OverlayToolbarState) -> bool {
    state.tool == Some(AnnotationTool::Counter)
}

fn is_text_active(state: OverlayToolbarState) -> bool {
    state.tool == Some(AnnotationTool::Text)
}

fn is_mosaic_active(state: OverlayToolbarState) -> bool {
    state.tool == Some(AnnotationTool::Mosaic)
}

fn is_undo_disabled(state: OverlayToolbarState) -> bool {
    !state.can_undo
}

fn is_redo_disabled(state: OverlayToolbarState) -> bool {
    !state.can_redo
}

fn cmd_arrow() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Arrow))
}

fn cmd_rect() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Rectangle))
}

fn cmd_circle() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Circle))
}

fn cmd_counter() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Counter))
}

fn cmd_text() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Text))
}

fn cmd_mosaic() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Mosaic))
}

fn cmd_undo() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::Undo)
}

fn cmd_redo() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::Redo)
}

fn cmd_ocr() -> OverlayCommand {
    OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Ocr))
}

fn cmd_qr() -> OverlayCommand {
    OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::QrCode))
}

fn cmd_save() -> OverlayCommand {
    OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Save))
}

fn cmd_scroll() -> OverlayCommand {
    OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Scroll))
}

fn cmd_pin() -> OverlayCommand {
    OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Pin))
}

fn cmd_copy() -> OverlayCommand {
    OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Copy))
}

fn cmd_cancel() -> OverlayCommand {
    OverlayCommand::Lifecycle(LifecycleCommand::CloseIntent)
}

const SAVE_BUTTON_ID: &str = "overlay-save";
const SAVE_MENU_HOVER_CLOSE_GRACE: Duration = Duration::from_millis(120);
const SAVE_MENU_ROW_HEIGHT: f32 = 32.0;

#[derive(Default)]
struct SaveMenuHoverState {
    trigger_hovered: bool,
    menu_hovered: bool,
    grace_open: bool,
    close_seq: u64,
}

impl SaveMenuHoverState {
    fn is_open(&self) -> bool {
        self.trigger_hovered || self.menu_hovered || self.grace_open
    }
}

const TOOL_BUTTONS: &[ToolbarButtonSpec] = &[
    ToolbarButtonSpec {
        id: "overlay-tool-arrow",
        icon: ToolbarIcon::Arrow,
        tooltip: i18n::overlay::annotation_tool_arrow,
        command: cmd_arrow,
        active: is_arrow_active,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-tool-rect",
        icon: ToolbarIcon::Rectangle,
        tooltip: i18n::overlay::annotation_tool_rectangle,
        command: cmd_rect,
        active: is_rectangle_active,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-tool-circle",
        icon: ToolbarIcon::Circle,
        tooltip: i18n::overlay::annotation_tool_circle,
        command: cmd_circle,
        active: is_circle_active,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-tool-counter",
        icon: ToolbarIcon::Counter,
        tooltip: i18n::overlay::annotation_tool_counter,
        command: cmd_counter,
        active: is_counter_active,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-tool-text",
        icon: ToolbarIcon::Text,
        tooltip: i18n::overlay::annotation_tool_text,
        command: cmd_text,
        active: is_text_active,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-tool-mosaic",
        icon: ToolbarIcon::Mosaic,
        tooltip: i18n::overlay::annotation_tool_mosaic,
        command: cmd_mosaic,
        active: is_mosaic_active,
        disabled: never_disabled,
        visible: always_visible,
    },
];

const HISTORY_BUTTONS: &[ToolbarButtonSpec] = &[
    ToolbarButtonSpec {
        id: "overlay-undo",
        icon: ToolbarIcon::Undo,
        tooltip: i18n::overlay::annotation_undo,
        command: cmd_undo,
        active: always_inactive,
        disabled: is_undo_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-redo",
        icon: ToolbarIcon::Redo,
        tooltip: i18n::overlay::annotation_redo,
        command: cmd_redo,
        active: always_inactive,
        disabled: is_redo_disabled,
        visible: always_visible,
    },
];

const CAPTURE_BUTTONS: &[ToolbarButtonSpec] = &[
    ToolbarButtonSpec {
        id: "overlay-ocr",
        icon: ToolbarIcon::Ocr,
        tooltip: i18n::common::ocr,
        command: cmd_ocr,
        active: always_inactive,
        disabled: never_disabled,
        visible: ocr_enabled,
    },
    ToolbarButtonSpec {
        id: "overlay-qr",
        icon: ToolbarIcon::QrCode,
        tooltip: i18n::common::scan_qr,
        command: cmd_qr,
        active: always_inactive,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-scroll",
        icon: ToolbarIcon::Scroll,
        tooltip: i18n::common::scroll,
        command: cmd_scroll,
        active: always_inactive,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: SAVE_BUTTON_ID,
        icon: ToolbarIcon::Save,
        tooltip: i18n::common::save,
        command: cmd_save,
        active: always_inactive,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-pin",
        icon: ToolbarIcon::Pin,
        tooltip: i18n::common::pin,
        command: cmd_pin,
        active: always_inactive,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-copy",
        icon: ToolbarIcon::Copy,
        tooltip: i18n::common::copy,
        command: cmd_copy,
        active: always_inactive,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-cancel",
        icon: ToolbarIcon::Cancel,
        tooltip: i18n::common::cancel,
        command: cmd_cancel,
        active: always_inactive,
        disabled: never_disabled,
        visible: always_visible,
    },
];

pub(crate) fn toolbar_button_count() -> usize {
    TOOL_BUTTONS.iter().filter(|spec| (spec.visible)()).count()
        + HISTORY_BUTTONS.iter().filter(|spec| (spec.visible)()).count()
        + CAPTURE_BUTTONS.iter().filter(|spec| (spec.visible)()).count()
}

pub(crate) fn toolbar_icon(app_ctx: &App, icon_name: ToolbarIcon) -> Icon {
    let theme = app_ctx.theme();
    Icon::new(icon_name).small().text_color(theme.popover_foreground)
}

fn dispatch_save_with_custom_path(on_action: OverlayActionHandler, window: &mut Window, cx: &mut App) {
    let prompt = SharedString::from(i18n::preferences::select_save_directory());
    let receiver = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some(prompt),
    });
    let window_handle = window.window_handle();

    cx.spawn(async move |cx| {
        let next_path = match receiver.await {
            Ok(Ok(Some(paths))) => paths.into_iter().next(),
            Ok(Ok(None)) | Ok(Err(_)) | Err(_) => None,
        };

        let Some(next_path) = next_path else {
            return;
        };

        let save_path = next_path.to_string_lossy().into_owned();
        let _ = cx.update_window(window_handle, move |_, window: &mut Window, app: &mut App| {
            on_action(OverlayCommand::Capture(CaptureCommand::SaveWithPath(save_path)), window, app);
        });
    })
    .detach();
}

fn schedule_save_menu_grace_close(menu_state: Entity<SaveMenuHoverState>, window: &mut Window, cx: &mut App, seq: u64) {
    let window_handle = window.window_handle();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(SAVE_MENU_HOVER_CLOSE_GRACE).await;
        let _ = cx.update_window(window_handle, move |_, _, app| {
            menu_state.update(app, |state, cx| {
                if state.close_seq == seq && !state.trigger_hovered && !state.menu_hovered {
                    state.grace_open = false;
                    cx.notify();
                }
            });
        });
    })
    .detach();
}

fn update_save_menu_hover_state(
    menu_state: &Entity<SaveMenuHoverState>,
    window: &mut Window,
    cx: &mut App,
    update: impl FnOnce(&mut SaveMenuHoverState),
) {
    let mut close_seq = None;
    menu_state.update(cx, |state, cx| {
        update(state);
        if state.trigger_hovered || state.menu_hovered {
            state.grace_open = false;
            state.close_seq = state.close_seq.saturating_add(1);
        } else {
            state.grace_open = true;
            state.close_seq = state.close_seq.saturating_add(1);
            close_seq = Some(state.close_seq);
        }
        cx.notify();
    });

    if let Some(seq) = close_seq {
        schedule_save_menu_grace_close(menu_state.clone(), window, cx, seq);
    }
}

fn close_save_menu_immediately(menu_state: &Entity<SaveMenuHoverState>, cx: &mut App) {
    menu_state.update(cx, |state, cx| {
        state.trigger_hovered = false;
        state.menu_hovered = false;
        state.grace_open = false;
        state.close_seq = state.close_seq.saturating_add(1);
        cx.notify();
    });
}

fn toolbar_save_hover_menu_button(
    window: &mut Window,
    app_ctx: &mut App,
    spec: ToolbarButtonSpec,
    state: OverlayToolbarState,
    on_action: OverlayActionHandler,
) -> AnyElement {
    let menu_state = window.use_keyed_state("overlay-save-hover-menu", app_ctx, |_, _| SaveMenuHoverState::default());
    let is_open = menu_state.read(app_ctx).is_open();

    let command = (spec.command)();
    let primary_action = on_action.clone();
    let menu_action = on_action.clone();
    let hover_state = menu_state.clone();
    let menu_hover_state = menu_state.clone();
    let click_state = menu_state.clone();

    let button = Button::new(spec.id)
        .compact()
        .icon(toolbar_icon(app_ctx, spec.icon))
        .tooltip((spec.tooltip)())
        .disabled((spec.disabled)(state))
        .on_click(move |_, window: &mut Window, cx: &mut App| {
            close_save_menu_immediately(&click_state, cx);
            primary_action(command.clone(), window, cx);
        })
        .on_hover(move |hovered, window, cx| {
            update_save_menu_hover_state(&hover_state, window, cx, |state| {
                state.trigger_hovered = *hovered;
            });
        });
    let button = if (spec.active)(state) { button.outline() } else { button.ghost() };

    Popover::new("overlay-save-hover-popover")
        .appearance(false)
        .overlay_closable(false)
        .anchor(Corner::TopRight)
        .open(is_open)
        .trigger(button)
        .content(move |_, _window, cx| {
            let menu_action = menu_action.clone();
            let menu_hover_state_for_container = menu_hover_state.clone();
            let menu_hover_state_for_click = menu_hover_state.clone();
            let theme = cx.theme();
            let row_hover_bg = theme.accent;
            let row_hover_fg = theme.accent_foreground;

            div()
                .id("overlay-save-hover-menu")
                .rounded(theme.radius_lg)
                .border_1()
                .border_color(theme.border.alpha(0.8))
                .bg(theme.popover)
                .when(theme.shadow, |this| this.shadow_lg())
                .p_1()
                .flex()
                .flex_col()
                .on_hover(move |hovered, window, cx| {
                    update_save_menu_hover_state(&menu_hover_state_for_container, window, cx, |state| {
                        state.menu_hovered = *hovered;
                    });
                })
                .child(
                    div()
                        .id("overlay-save-hover-menu-item")
                        .h(px(SAVE_MENU_ROW_HEIGHT))
                        .rounded(theme.radius)
                        .px_2()
                        .text_sm()
                        .text_color(theme.popover_foreground)
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .hover(move |this| this.bg(row_hover_bg).text_color(row_hover_fg))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(move |_, window, cx| {
                            close_save_menu_immediately(&menu_hover_state_for_click, cx);
                            dispatch_save_with_custom_path(menu_action.clone(), window, cx);
                        })
                        .child(i18n::preferences::select_save_directory()),
                )
                .into_any_element()
        })
        .into_any_element()
}

fn toolbar_action_button(
    window: &mut Window,
    app_ctx: &mut App,
    spec: ToolbarButtonSpec,
    state: OverlayToolbarState,
    on_action: OverlayActionHandler,
) -> AnyElement {
    if spec.id == SAVE_BUTTON_ID {
        return toolbar_save_hover_menu_button(window, app_ctx, spec, state, on_action);
    }

    let button = Button::new(spec.id)
        .compact()
        .icon(toolbar_icon(app_ctx, spec.icon))
        .tooltip((spec.tooltip)());
    let button = if (spec.active)(state) { button.outline() } else { button.ghost() };
    let command = (spec.command)();
    button
        .disabled((spec.disabled)(state))
        .on_click(move |_, window: &mut Window, cx: &mut App| {
            on_action(command.clone(), window, cx);
        })
        .into_any_element()
}

pub(crate) fn toolbar_group_divider(app_ctx: &App) -> impl IntoElement {
    div().h(px(14.0)).w(px(1.0)).bg(app_ctx.theme().border.alpha(0.45))
}

pub(crate) fn toolbar_panel(app_ctx: &App, layout: OverlayPanelLayout) -> Div {
    let theme = app_ctx.theme();
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
        .border_color(theme.border.alpha(0.82))
        .bg(theme.popover.alpha(0.98))
        .overflow_hidden()
        .px_2()
        .py_1()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Middle, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());
    if theme.shadow {
        panel = panel.shadow_lg();
    }
    panel
}

pub(crate) fn overlay_toolbar(
    window: &mut Window,
    app_ctx: &mut App,
    layout: OverlayPanelLayout,
    state: OverlayToolbarState,
    on_action: OverlayActionHandler,
) -> impl IntoElement {
    let mut row = h_flex().items_center().gap_0p5();
    for spec in TOOL_BUTTONS {
        if (spec.visible)() {
            row = row.child(toolbar_action_button(window, app_ctx, *spec, state, on_action.clone()));
        }
    }
    row = row.child(toolbar_group_divider(app_ctx));
    for spec in HISTORY_BUTTONS {
        if (spec.visible)() {
            row = row.child(toolbar_action_button(window, app_ctx, *spec, state, on_action.clone()));
        }
    }
    row = row.child(toolbar_group_divider(app_ctx));
    for spec in CAPTURE_BUTTONS {
        if (spec.visible)() {
            row = row.child(toolbar_action_button(window, app_ctx, *spec, state, on_action.clone()));
        }
    }

    toolbar_panel(app_ctx, layout).child(row)
}

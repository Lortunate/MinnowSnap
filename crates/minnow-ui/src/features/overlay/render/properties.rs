use minnow_assets::asset_paths;
use gpui::{
    App, Corner, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window, div,
    px, rgba,
};
use gpui_component::{
    ActiveTheme as _, Disableable, Icon, IconNamed, Sizable,
    button::{Button, ButtonVariants},
    color_picker::{ColorPicker, ColorPickerState},
    h_flex, v_flex,
};

use minnow_core::i18n;
use crate::features::overlay::annotation::COLOR_PRESETS;
use crate::features::overlay::render::OverlayActionHandler;
use crate::features::overlay::render::layout::OverlayPanelLayout;
use crate::features::overlay::state::{
    AnnotationCommand, AnnotationKindTag, AnnotationSelectionInfo, AnnotationStyleState, AnnotationTool, MosaicMode, OverlayCommand,
};

#[derive(Clone)]
pub(crate) struct OverlayPropertyState {
    pub style: AnnotationStyleState,
    pub active_tool: Option<AnnotationTool>,
    pub selected_annotation: Option<AnnotationSelectionInfo>,
    pub text_editing: bool,
    pub recent_custom_colors: Vec<u32>,
}

#[derive(Clone, Copy)]
enum PropertyIcon {
    CustomColorAdd,
    Fill,
    StrokeDown,
    StrokeUp,
    EditText,
    MosaicPixelate,
    MosaicBlur,
}

#[derive(Clone)]
struct PropertyButtonSpec {
    id: &'static str,
    icon_name: PropertyIcon,
    tooltip: String,
    command: OverlayCommand,
    active: bool,
    disabled: bool,
}

impl IconNamed for PropertyIcon {
    fn path(self) -> SharedString {
        match self {
            Self::CustomColorAdd => asset_paths::icons::ADD,
            Self::Fill => asset_paths::icons::SQUARE_FILL,
            Self::StrokeDown => asset_paths::icons::ARROW_DROP_DOWN,
            Self::StrokeUp => asset_paths::icons::ARROW_DROP_UP,
            Self::EditText => asset_paths::icons::TEXT_FIELDS,
            Self::MosaicPixelate => asset_paths::icons::GRID_ON,
            Self::MosaicBlur => asset_paths::icons::LENS_BLUR,
        }
        .into()
    }
}

fn icon(app_ctx: &App, icon_name: PropertyIcon) -> Icon {
    Icon::new(icon_name).small().text_color(app_ctx.theme().popover_foreground)
}

fn action_button(app_ctx: &App, spec: PropertyButtonSpec, on_action: OverlayActionHandler) -> Button {
    let base = Button::new(spec.id).compact().icon(icon(app_ctx, spec.icon_name)).tooltip(spec.tooltip);
    let base = if spec.active { base.outline() } else { base.ghost() };
    let command = spec.command;
    base.disabled(spec.disabled).on_click(move |_, window: &mut Window, cx: &mut App| {
        on_action(command.clone(), window, cx);
    })
}

fn metric_label(state: &OverlayPropertyState) -> String {
    if let Some(item) = state.selected_annotation.as_ref() {
        match item.kind {
            AnnotationKindTag::Counter => format!("R:{:.0}", item.metric),
            AnnotationKindTag::Text => format!("F:{:.0}", item.metric),
            AnnotationKindTag::Mosaic => format!("I:{:.0}", item.metric),
            _ => format!("{:.0}px", item.metric),
        }
    } else {
        match state.active_tool {
            Some(AnnotationTool::Counter) => format!("R:{:.0}", state.style.counter_radius),
            Some(AnnotationTool::Text) => format!("F:{:.0}", state.style.text_size),
            Some(AnnotationTool::Mosaic) => format!("I:{:.0}", state.style.mosaic_intensity),
            _ => format!("{:.0}px", state.style.stroke_width),
        }
    }
}

fn color_swatch(app_ctx: &App, id: &'static str, color: u32, selected: bool, on_action: OverlayActionHandler) -> impl IntoElement {
    let theme = app_ctx.theme();
    let border = if selected { theme.primary } else { theme.border.alpha(0.9) };
    let bg = if selected {
        theme.primary.alpha(0.14)
    } else {
        theme.popover_foreground.alpha(0.02)
    };
    let mut swatch = div()
        .id(id)
        .w(px(26.0))
        .h(px(26.0))
        .rounded(theme.radius)
        .border_1()
        .border_color(border)
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(px(18.0))
                .h(px(18.0))
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.border.alpha(0.7))
                .bg(rgba(color)),
        )
        .on_click(move |_, window: &mut Window, cx: &mut App| {
            on_action(OverlayCommand::Annotation(AnnotationCommand::SetColor { color }), window, cx);
        });
    if selected {
        swatch = swatch.border_2().shadow_sm();
    }
    swatch
}

fn section_card(app_ctx: &App, content: impl IntoElement) -> impl IntoElement {
    let theme = app_ctx.theme();
    div()
        .w_full()
        .rounded(theme.radius)
        .border_1()
        .border_color(theme.border.alpha(0.75))
        .bg(theme.popover_foreground.alpha(0.03))
        .px_2()
        .py_1()
        .child(content)
}

fn custom_color_button(app_ctx: &App, selected_color: u32, active: bool, picker: impl IntoElement) -> impl IntoElement {
    let theme = app_ctx.theme();
    let border = if active { theme.primary } else { theme.border.alpha(0.8) };
    let bg = if active {
        theme.primary.alpha(0.14)
    } else {
        theme.popover_foreground.alpha(0.04)
    };
    div()
        .id("overlay-prop-custom-color")
        .w(px(84.0))
        .h(px(32.0))
        .rounded(theme.radius)
        .border_1()
        .border_color(border)
        .bg(bg)
        .px_2()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .w(px(16.0))
                .h(px(16.0))
                .rounded_full()
                .border_1()
                .border_color(theme.border.alpha(0.7))
                .bg(rgba(selected_color)),
        )
        .child(picker)
}

pub(crate) fn overlay_properties_panel(
    app_ctx: &App,
    layout: OverlayPanelLayout,
    state: OverlayPropertyState,
    color_picker_state: &Entity<ColorPickerState>,
    on_action: OverlayActionHandler,
) -> impl IntoElement {
    let theme = app_ctx.theme();
    let selected_color = state.style.stroke_color;
    let selected_color_key = selected_color & 0xffffff00;
    let selected_is_text = state
        .selected_annotation
        .as_ref()
        .is_some_and(|item| item.kind == AnnotationKindTag::Text);
    let selected_mosaic_mode = state.selected_annotation.as_ref().and_then(|item| item.mosaic_mode);
    let is_mosaic = selected_mosaic_mode.is_some() || state.active_tool == Some(AnnotationTool::Mosaic);
    let mosaic_mode = selected_mosaic_mode.unwrap_or(state.style.mosaic_mode);
    let custom_color_active = COLOR_PRESETS.iter().all(|color| (color & 0xffffff00) != selected_color_key);

    let mut color_row = h_flex().items_center().gap_1();
    for (idx, color) in COLOR_PRESETS.iter().copied().enumerate() {
        color_row = color_row.child(color_swatch(
            app_ctx,
            match idx {
                0 => "overlay-prop-color-0",
                1 => "overlay-prop-color-1",
                2 => "overlay-prop-color-2",
                3 => "overlay-prop-color-3",
                4 => "overlay-prop-color-4",
                _ => "overlay-prop-color-5",
            },
            color,
            (color & 0xffffff00) == selected_color_key,
            on_action.clone(),
        ));
    }

    let mut featured_colors: Vec<gpui::Hsla> = state.recent_custom_colors.iter().copied().map(|color| rgba(color).into()).collect();
    for color in COLOR_PRESETS {
        featured_colors.push(rgba(color).into());
    }
    let picker = ColorPicker::new(color_picker_state)
        .small()
        .anchor(Corner::BottomRight)
        .icon(
            Icon::new(PropertyIcon::CustomColorAdd)
                .small()
                .text_color(if custom_color_active { theme.primary } else { theme.popover_foreground }),
        )
        .featured_colors(featured_colors);

    color_row = color_row.child(custom_color_button(app_ctx, selected_color, custom_color_active, picker));

    let stroke_down = PropertyButtonSpec {
        id: "overlay-prop-stroke-down",
        icon_name: PropertyIcon::StrokeDown,
        tooltip: if is_mosaic {
            i18n::overlay::annotation_mosaic_intensity_down()
        } else {
            i18n::overlay::annotation_stroke_down()
        },
        command: if is_mosaic {
            OverlayCommand::Annotation(AnnotationCommand::AdjustMosaicIntensity { delta: -2.0 })
        } else {
            OverlayCommand::Annotation(AnnotationCommand::AdjustStroke { delta: -1.0 })
        },
        active: false,
        disabled: false,
    };
    let stroke_up = PropertyButtonSpec {
        id: "overlay-prop-stroke-up",
        icon_name: PropertyIcon::StrokeUp,
        tooltip: if is_mosaic {
            i18n::overlay::annotation_mosaic_intensity_up()
        } else {
            i18n::overlay::annotation_stroke_up()
        },
        command: if is_mosaic {
            OverlayCommand::Annotation(AnnotationCommand::AdjustMosaicIntensity { delta: 2.0 })
        } else {
            OverlayCommand::Annotation(AnnotationCommand::AdjustStroke { delta: 1.0 })
        },
        active: false,
        disabled: false,
    };
    let fill_button = PropertyButtonSpec {
        id: "overlay-prop-fill",
        icon_name: PropertyIcon::Fill,
        tooltip: i18n::overlay::annotation_toggle_fill(),
        command: OverlayCommand::Annotation(AnnotationCommand::ToggleFill),
        active: state.style.fill_enabled,
        disabled: is_mosaic,
    };
    let mut parameter_row = h_flex().items_center().gap_1();
    parameter_row = parameter_row.child(action_button(app_ctx, stroke_down, on_action.clone()));
    parameter_row = parameter_row.child(
        div()
            .w(px(56.0))
            .h(px(32.0))
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border.alpha(0.75))
            .bg(theme.popover_foreground.alpha(0.05))
            .flex()
            .items_center()
            .justify_center()
            .text_xs()
            .text_color(theme.popover_foreground)
            .child(metric_label(&state)),
    );
    parameter_row = parameter_row.child(action_button(app_ctx, stroke_up, on_action.clone()));
    parameter_row = parameter_row.child(action_button(app_ctx, fill_button, on_action.clone()));

    let mut mode_specs = Vec::new();
    if is_mosaic {
        mode_specs.push(PropertyButtonSpec {
            id: "overlay-prop-mosaic-pixelate",
            icon_name: PropertyIcon::MosaicPixelate,
            tooltip: i18n::overlay::annotation_mosaic_mode_pixelate(),
            command: OverlayCommand::Annotation(AnnotationCommand::SetMosaicMode(MosaicMode::Pixelate)),
            active: mosaic_mode == MosaicMode::Pixelate,
            disabled: false,
        });
        mode_specs.push(PropertyButtonSpec {
            id: "overlay-prop-mosaic-blur",
            icon_name: PropertyIcon::MosaicBlur,
            tooltip: i18n::overlay::annotation_mosaic_mode_blur(),
            command: OverlayCommand::Annotation(AnnotationCommand::SetMosaicMode(MosaicMode::Blur)),
            active: mosaic_mode == MosaicMode::Blur,
            disabled: false,
        });
    }

    if selected_is_text {
        mode_specs.push(PropertyButtonSpec {
            id: "overlay-prop-edit-text",
            icon_name: PropertyIcon::EditText,
            tooltip: i18n::overlay::annotation_edit_text(),
            command: OverlayCommand::Annotation(AnnotationCommand::StartTextEdit),
            active: state.text_editing,
            disabled: false,
        });
    }

    let mut panel = div()
        .absolute()
        .left(px(layout.x as f32))
        .top(px(layout.y as f32))
        .w(px(layout.width as f32))
        .h(px(layout.height as f32))
        .flex()
        .rounded(theme.radius_lg)
        .border_1()
        .border_color(theme.border.alpha(0.9))
        .bg(theme.popover)
        .overflow_hidden()
        .px_2()
        .py_2()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());
    if theme.shadow {
        panel = panel.shadow_lg();
    }
    let mut sections = v_flex()
        .w_full()
        .gap_1()
        .child(section_card(app_ctx, color_row))
        .child(section_card(app_ctx, parameter_row));

    if !mode_specs.is_empty() {
        let mut mode_row = h_flex().items_center().gap_1();
        for spec in mode_specs {
            mode_row = mode_row.child(action_button(app_ctx, spec, on_action.clone()));
        }
        sections = sections.child(section_card(app_ctx, mode_row));
    }

    panel.child(sections)
}

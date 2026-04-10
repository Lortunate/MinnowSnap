use gpui::{App, IntoElement, ParentElement, Styled, div, px, rgba};
use gpui_component::ActiveTheme as _;
use minnow_core::i18n;

use crate::features::overlay::state::{PickerFormat, PickerNeighborhood, PickerSample};
use crate::key_unicode::{KeyUnicode, key_symbol};

const PICKER_MARGIN: f64 = 16.0;
const MAGNIFIER_CELL_SIZE: f32 = 11.0;
const PICKER_DIVIDER_HEIGHT: f64 = 1.0;
const PICKER_INFO_HEIGHT: f64 = 66.0;
const PICKER_SWATCH_SIZE: f32 = 14.0;
const PICKER_VALUE_TEXT_SIZE: f32 = 13.0;
const PICKER_META_TEXT_SIZE: f32 = 11.0;

pub(crate) fn overlay_picker(
    app_ctx: &App,
    cursor: Option<(f64, f64)>,
    sample: Option<&PickerSample>,
    neighborhood: Option<&PickerNeighborhood>,
    picker_format: PickerFormat,
    viewport_w: f64,
    viewport_h: f64,
) -> impl IntoElement {
    let theme = app_ctx.theme();
    let Some((cursor_x, cursor_y)) = cursor else {
        return div();
    };
    let Some(sample) = sample else {
        return div();
    };
    let Some(neighborhood) = neighborhood else {
        return div();
    };

    let magnifier_size = neighborhood.size as f64 * f64::from(MAGNIFIER_CELL_SIZE);
    let picker_width = magnifier_size;
    let picker_height = magnifier_size + PICKER_DIVIDER_HEIGHT + PICKER_INFO_HEIGHT;
    let (x, y) = resolve_picker_origin(cursor_x, cursor_y, picker_width, picker_height, viewport_w, viewport_h);

    let mut panel = div()
        .absolute()
        .left(px(x as f32))
        .top(px(y as f32))
        .w(px(picker_width as f32))
        .rounded(theme.radius_lg)
        .bg(theme.popover)
        .border_1()
        .border_color(theme.border)
        .overflow_hidden();
    if theme.shadow {
        panel = panel.shadow_lg();
    }
    panel
        .child(div().w_full().h(px(magnifier_size as f32)).child(picker_magnifier(neighborhood)))
        .child(div().w_full().h(px(PICKER_DIVIDER_HEIGHT as f32)).bg(theme.border))
        .child(picker_info_section(theme, sample, neighborhood, picker_format))
}

fn picker_magnifier(neighborhood: &PickerNeighborhood) -> impl IntoElement {
    let grid_size = neighborhood.size;
    let center = grid_size / 2;

    let center_idx = center * grid_size + center;
    let [center_r, center_g, center_b] = neighborhood.pixels.get(center_idx).copied().unwrap_or([0, 0, 0]);
    let contrast_border = picker_border_color(center_r, center_g, center_b);

    let mut grid = div().flex().flex_col();
    for row in 0..grid_size {
        let mut row_el = div().flex().h(px(MAGNIFIER_CELL_SIZE));
        for col in 0..grid_size {
            let idx = row * grid_size + col;
            let [r, g, b] = neighborhood.pixels.get(idx).copied().unwrap_or([0, 0, 0]);
            let color = rgba(u32::from(r) << 24 | u32::from(g) << 16 | u32::from(b) << 8 | 0xff);
            let mut cell = div().w(px(MAGNIFIER_CELL_SIZE)).h(px(MAGNIFIER_CELL_SIZE)).bg(color);
            if row == center && col == center {
                cell = cell.border_1().border_color(contrast_border);
            }
            row_el = row_el.child(cell);
        }
        grid = grid.child(row_el);
    }
    grid
}

fn resolve_picker_origin(cursor_x: f64, cursor_y: f64, picker_width: f64, picker_height: f64, viewport_w: f64, viewport_h: f64) -> (f64, f64) {
    let mut x = cursor_x + PICKER_MARGIN;
    let mut y = cursor_y + PICKER_MARGIN;
    if x + picker_width > viewport_w - PICKER_MARGIN {
        x = cursor_x - picker_width - PICKER_MARGIN;
    }
    if y + picker_height > viewport_h - PICKER_MARGIN {
        y = cursor_y - picker_height - PICKER_MARGIN;
    }

    // Keyboard pixel movement can place the picker on fractional coordinates, which
    // causes antialiased seams between magnifier cells. Snap the panel to whole pixels.
    (
        x.clamp(PICKER_MARGIN, (viewport_w - picker_width - PICKER_MARGIN).max(PICKER_MARGIN))
            .round(),
        y.clamp(PICKER_MARGIN, (viewport_h - picker_height - PICKER_MARGIN).max(PICKER_MARGIN))
            .round(),
    )
}

fn picker_info_section(
    theme: &gpui_component::Theme,
    sample: &PickerSample,
    neighborhood: &PickerNeighborhood,
    picker_format: PickerFormat,
) -> gpui::Div {
    let color_text = sample.formatted(picker_format);
    let format_label = picker_format.label();
    let color_value = rgba(u32::from(sample.r) << 24 | u32::from(sample.g) << 16 | u32::from(sample.b) << 8 | 0xff);
    let primary_label = i18n::overlay::picker_value_and_format(&color_text, format_label);
    let coordinates_label = i18n::overlay::picker_coordinates(neighborhood.center_x, neighborhood.center_y);
    let shortcut_hint = i18n::overlay::picker_shortcuts("C", key_symbol(KeyUnicode::Shift));

    div().h(px(PICKER_INFO_HEIGHT as f32)).px_1().py_1().child(
        div()
            .w_full()
            .h_full()
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border.alpha(0.9))
            .bg(theme.popover.alpha(0.96))
            .px_2()
            .py_1()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .text_center()
            .gap_0p5()
            .child(
                div()
                    .w_full()
                    .h(px(PICKER_SWATCH_SIZE + 2.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_0p5()
                    .overflow_hidden()
                    .child(
                        div()
                            .w(px(PICKER_SWATCH_SIZE))
                            .h(px(PICKER_SWATCH_SIZE))
                            .rounded_sm()
                            .bg(color_value)
                            .border_1()
                            .border_color(theme.border.alpha(0.8)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(PICKER_VALUE_TEXT_SIZE))
                            .text_color(theme.popover_foreground)
                            .line_clamp(1)
                            .text_ellipsis()
                            .child(primary_label),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .text_size(px(PICKER_META_TEXT_SIZE))
                    .text_color(theme.muted_foreground.alpha(0.95))
                    .line_clamp(1)
                    .text_ellipsis()
                    .child(coordinates_label),
            )
            .child(
                div()
                    .w_full()
                    .text_size(px(PICKER_META_TEXT_SIZE))
                    .text_color(theme.muted_foreground.alpha(0.8))
                    .line_clamp(1)
                    .text_ellipsis()
                    .child(shortcut_hint),
            ),
    )
}

fn picker_border_color(r: u8, g: u8, b: u8) -> gpui::Hsla {
    if prefers_black_border(r, g, b) {
        rgba(0x000000ff).into()
    } else {
        rgba(0xffffffff).into()
    }
}

fn prefers_black_border(r: u8, g: u8, b: u8) -> bool {
    let pixel_luma = relative_luminance(r, g, b);
    let contrast_with_black = contrast_ratio(pixel_luma, 0.0);
    let contrast_with_white = contrast_ratio(pixel_luma, 1.0);
    contrast_with_black >= contrast_with_white
}

fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    fn srgb_to_linear(channel: u8) -> f64 {
        let srgb = f64::from(channel) / 255.0;
        if srgb <= 0.04045 {
            srgb / 12.92
        } else {
            ((srgb + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * srgb_to_linear(r) + 0.7152 * srgb_to_linear(g) + 0.0722 * srgb_to_linear(b)
}

fn contrast_ratio(luma_a: f64, luma_b: f64) -> f64 {
    let lighter = luma_a.max(luma_b);
    let darker = luma_a.min(luma_b);
    (lighter + 0.05) / (darker + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_origin_snaps_fractional_position_to_whole_pixels() {
        let (x, y) = resolve_picker_origin(10.5, 20.5, 143.0, 194.0, 800.0, 600.0);

        assert_eq!((x, y), (27.0, 37.0));
    }

    #[test]
    fn picker_origin_flips_when_bottom_right_space_is_insufficient() {
        let (x, y) = resolve_picker_origin(790.4, 590.6, 143.0, 194.0, 800.0, 600.0);

        assert_eq!((x, y), (631.0, 381.0));
    }

    #[test]
    fn picker_origin_clamps_to_margin_when_viewport_is_smaller_than_picker() {
        let (x, y) = resolve_picker_origin(4.2, 3.8, 143.0, 194.0, 120.0, 160.0);

        assert_eq!((x, y), (16.0, 16.0));
    }

    #[test]
    fn dark_pixels_prefer_white_border() {
        assert!(!prefers_black_border(8, 16, 24));
    }

    #[test]
    fn bright_pixels_prefer_black_border() {
        assert!(prefers_black_border(240, 240, 240));
    }

    #[test]
    fn saturated_red_prefers_higher_contrast_black_border() {
        assert!(prefers_black_border(255, 0, 0));
    }
}

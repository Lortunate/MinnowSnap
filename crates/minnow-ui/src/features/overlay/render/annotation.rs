use gpui::prelude::FluentBuilder as _;
use gpui::{App, IntoElement, ParentElement, Styled, div, img, px, rgba};
use gpui_component::ActiveTheme as _;

use minnow_core::geometry::RectF;
use crate::features::overlay::state::AnnotationLayerState;

fn color(value: u32) -> gpui::Hsla {
    rgba(value).into()
}

fn relative(selection: RectF, x: f64, y: f64) -> (f64, f64) {
    (x - selection.x, y - selection.y)
}

pub(crate) fn overlay_annotations_layer(cx: &App, selection: RectF, state: &AnnotationLayerState) -> impl IntoElement {
    let mut layer = div()
        .absolute()
        .left(px(selection.x as f32))
        .top(px(selection.y as f32))
        .w(px(selection.width.max(0.0) as f32))
        .h(px(selection.height.max(0.0) as f32))
        .overflow_hidden();

    if let Some(image) = state.image.clone() {
        layer = layer.child(img(image).size_full());
    }

    for outline in &state.outlines {
        let (x, y) = relative(selection, outline.bounds.x, outline.bounds.y);
        layer = layer.child(
            div()
                .absolute()
                .left(px(x as f32))
                .top(px(y as f32))
                .w(px(outline.bounds.width.max(2.0) as f32))
                .h(px(outline.bounds.height.max(2.0) as f32))
                .when(outline.selected, |this| {
                    this.border_1()
                        .border_color(color(0xffffffff).alpha(if outline.transient { 0.75 } else { 0.9 }))
                })
                .when(outline.transient, |this| this.border_1().border_color(color(0xffffffaa))),
        );
    }

    let theme = cx.theme();
    layer.border_1().border_color(theme.selection.alpha(0.25))
}

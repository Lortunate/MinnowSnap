//! GPU presentation of the CPU editor. Document mutation and rasterization live in minnow-editor.
use gpui::RenderImage;
use image::RgbaImage;
use std::sync::{Arc, Weak};

#[cfg(test)]
pub(crate) use minnow_editor::AnnotationItem;
pub(crate) use minnow_editor::{
    AnnotationEngine, AnnotationKind, AnnotationKindTag, AnnotationRasterDiagnostics, AnnotationSelectionInfo, AnnotationStyleState, AnnotationTool,
    COLOR_PRESETS, MosaicMode,
};

#[derive(Clone, Debug, Default)]
pub(crate) struct AnnotationTextureCache {
    source: Option<Weak<RgbaImage>>,
    texture: Option<Arc<RenderImage>>,
}

impl AnnotationTextureCache {
    pub(crate) fn texture(&mut self, source: Option<Arc<RgbaImage>>) -> Option<Arc<RenderImage>> {
        let Some(source) = source else {
            self.source = None;
            self.texture = None;
            return None;
        };
        if self
            .source
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|previous| Arc::ptr_eq(&previous, &source))
        {
            return self.texture.clone();
        }
        let texture = crate::presentation::render_image::from_rgba_copy(&source);
        self.source = Some(Arc::downgrade(&source));
        self.texture = Some(texture.clone());
        Some(texture)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AnnotationLayerState {
    pub image: Option<Arc<RenderImage>>,
    pub image_bounds: Option<minnow_imaging::RectF>,
    pub raster_error: Option<minnow_imaging::GeometryError>,
    pub preview_reduced: bool,
    pub outlines: Vec<minnow_editor::AnnotationOutline>,
}

#[derive(Clone, Debug)]
pub(crate) struct AnnotationUiState {
    pub layer: AnnotationLayerState,
    pub selected: Option<AnnotationSelectionInfo>,
    pub tool: Option<AnnotationTool>,
    pub style: AnnotationStyleState,
    pub can_undo: bool,
    pub can_redo: bool,
    pub text_editing: bool,
}

impl AnnotationUiState {
    pub(crate) fn from_editor(state: minnow_editor::AnnotationUiState, cache: &mut AnnotationTextureCache) -> Self {
        Self {
            layer: AnnotationLayerState {
                preview_reduced: state
                    .layer
                    .image
                    .as_ref()
                    .is_some_and(|image| crate::presentation::render_image::preview_is_reduced(image.width(), image.height())),
                image: cache.texture(state.layer.image),
                image_bounds: state.layer.image_bounds,
                raster_error: state.layer.raster_error,
                outlines: state.layer.outlines,
            },
            selected: state.selected,
            tool: state.tool,
            style: state.style,
            can_undo: state.can_undo,
            can_redo: state.can_redo,
            text_editing: state.text_editing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_cache_keeps_identity_until_cpu_raster_changes() {
        let mut cache = AnnotationTextureCache::default();
        let pixels = Arc::new(RgbaImage::new(2, 2));
        let first = cache.texture(Some(pixels.clone())).unwrap();
        let second = cache.texture(Some(pixels.clone())).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        let replacement = cache.texture(Some(Arc::new((*pixels).clone()))).unwrap();
        assert!(!Arc::ptr_eq(&first, &replacement));
        assert!(cache.texture(None).is_none());
        let source_lifetime = Arc::downgrade(&pixels);
        let after_clear = cache.texture(Some(pixels)).unwrap();
        assert!(
            source_lifetime.upgrade().is_none(),
            "the UI texture cache must not retain CPU raster pixels"
        );
        assert!(!Arc::ptr_eq(&first, &after_clear));
    }
}

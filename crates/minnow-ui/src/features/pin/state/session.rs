use super::super::request::PinRequest;
use super::super::selection_text::format_selected_blocks;
use super::PinLease;
use crate::flows::output::{OutputDelivery, OutputRequest, OutputState};
use gpui::{App, AppContext, Entity, Pixels, Point, Size, px, size};
use minnow_application::ports::OcrBlock;
use minnow_imaging::ImageRef;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::features::pin) struct PinWindowGeometry {
    origin: Option<(f32, f32)>,
    size: (f32, f32),
    min_size: f32,
}

impl PinWindowGeometry {
    pub(in crate::features::pin) fn origin(self) -> Option<(f32, f32)> {
        self.origin
    }

    pub(in crate::features::pin) fn min_size(self) -> f32 {
        self.min_size
    }

    pub(in crate::features::pin) fn window_size(self) -> Size<Pixels> {
        size(px(self.size.0), px(self.size.1))
    }
}

#[derive(Clone, Debug)]
pub(in crate::features::pin) struct PinSession {
    image: ImageRef,
    render_image: Arc<gpui::RenderImage>,
    base_size: (f32, f32),
    zoom: f32,
    opacity: f32,
    auto_ocr: bool,
    ocr: PinOcrState,
    output: OutputState,
    quota: PinLease,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::features::pin) struct PinTextSelection {
    pub block_index: usize,
    pub anchor: usize,
    pub head: usize,
}

impl PinTextSelection {
    pub fn range(&self) -> std::ops::Range<usize> {
        if self.anchor <= self.head {
            self.anchor..self.head
        } else {
            self.head..self.anchor
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(in crate::features::pin) struct PinOcrState {
    pub processing: bool,
    pub generation: u64,
    /// Changes only when a successful attempt replaces the selectable text.
    pub result_revision: u64,
    pub blocks: Arc<[OcrBlock]>,
    pub selected_indices: BTreeSet<usize>,
    pub hovered_index: Option<usize>,
    pub active_text: Option<PinTextSelection>,
    pub selection_rect: Option<(Point<Pixels>, Point<Pixels>)>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
pub(in crate::features::pin) struct PinFrame {
    pub image: Arc<gpui::RenderImage>,
    pub opacity: f32,
    pub preview_reduced: bool,
    pub base_size: (f32, f32),
    pub ocr: PinOcrState,
    pub output_pending: bool,
    pub output_request_error: Option<minnow_application::OperationError>,
    pub output_preflight_error: Option<minnow_application::OperationError>,
    pub output_can_retry: bool,
    _quota: PinLease,
}

impl PinSession {
    const MIN_SIZE: f32 = 24.0;
    const MIN_ZOOM: f32 = 0.2;
    const MAX_ZOOM: f32 = 8.0;
    const ZOOM_STEP: f32 = 0.1;
    const MIN_OPACITY: f32 = 0.2;
    const MAX_OPACITY: f32 = 1.0;
    const OPACITY_STEP: f32 = 0.05;

    pub(in crate::features::pin) fn new(cx: &mut App, request: PinRequest, lease: PinLease) -> Entity<Self> {
        cx.new(|_| Self::from_request(request, lease))
    }

    pub(in crate::features::pin) fn initial_geometry(request: &PinRequest) -> PinWindowGeometry {
        let base_size = request.base_size();
        let zoom = Self::initial_zoom(base_size);

        PinWindowGeometry {
            origin: request.origin(),
            size: (base_size.0 * zoom, base_size.1 * zoom),
            min_size: Self::MIN_SIZE,
        }
    }

    pub(in crate::features::pin) fn from_request(request: PinRequest, lease: PinLease) -> Self {
        let base_size = request.base_size();
        let image = request.image();
        let preview_size = lease.preview_size();
        // The preview allocation must match the admitted display dimensions.
        // A source may have trailing bytes or spare capacity beyond its pixels.
        let preview = if image.dimensions() == preview_size {
            image::RgbaImage::from_fn(preview_size.0, preview_size.1, |x, y| *image.pixels().get_pixel(x, y))
        } else {
            image::imageops::thumbnail(image.pixels(), preview_size.0, preview_size.1)
        };
        let render_image = crate::presentation::render_image::from_rgba(preview);
        Self {
            image,
            render_image,
            base_size,
            zoom: Self::initial_zoom(base_size),
            opacity: Self::MAX_OPACITY,
            auto_ocr: request.auto_ocr(),
            ocr: PinOcrState::default(),
            output: OutputState::default(),
            quota: lease,
        }
    }

    fn initial_zoom(base_size: (f32, f32)) -> f32 {
        Self::min_zoom_for(base_size).clamp(1.0, Self::MAX_ZOOM)
    }

    fn min_zoom_for(base_size: (f32, f32)) -> f32 {
        let (base_width, base_height) = base_size;
        if base_width <= 0.0 || base_height <= 0.0 {
            return Self::MIN_ZOOM;
        }

        (Self::MIN_SIZE / base_width).max(Self::MIN_SIZE / base_height).max(Self::MIN_ZOOM)
    }

    fn zoom_bounds(&self) -> (f32, f32) {
        (Self::min_zoom_for(self.base_size).min(Self::MAX_ZOOM), Self::MAX_ZOOM)
    }

    pub(in crate::features::pin) fn frame(&self) -> PinFrame {
        PinFrame {
            image: self.render_image.clone(),
            opacity: self.opacity,
            preview_reduced: crate::presentation::render_image::preview_is_reduced(self.image.width(), self.image.height()),
            base_size: self.base_size,
            ocr: self.ocr.clone(),
            output_pending: self.output.is_pending(),
            output_request_error: self.output.request_error().cloned(),
            output_preflight_error: self.output.preflight_error().cloned(),
            output_can_retry: self.output.can_retry(),
            _quota: self.quota.clone(),
        }
    }

    pub(in crate::features::pin) fn begin_output(&mut self, request: &OutputRequest) -> Option<u64> {
        self.output.begin(request)
    }

    pub(in crate::features::pin) fn finish_output(&mut self, attempt: u64, delivery: &OutputDelivery) -> bool {
        delivery.finish(&mut self.output, attempt)
    }

    pub(in crate::features::pin) fn retry_output(&self) -> Option<OutputRequest> {
        self.output.retry()
    }

    pub(in crate::features::pin) fn show_preflight_error(&mut self, error: minnow_application::OperationError) {
        self.output.show_error(error);
    }

    pub(in crate::features::pin) fn window_size(&self) -> Size<Pixels> {
        size(px(self.base_size.0 * self.zoom), px(self.base_size.1 * self.zoom))
    }

    pub(in crate::features::pin) fn apply_zoom_step(&mut self, step: f32) -> Option<Size<Pixels>> {
        let (min_zoom, max_zoom) = self.zoom_bounds();
        let next_zoom = (self.zoom + step * Self::ZOOM_STEP).clamp(min_zoom, max_zoom);
        if (next_zoom - self.zoom).abs() <= f32::EPSILON {
            return None;
        }

        self.zoom = next_zoom;
        Some(self.window_size())
    }

    pub(in crate::features::pin) fn apply_opacity_step(&mut self, step: f32) -> bool {
        let next_opacity = (self.opacity + step * Self::OPACITY_STEP).clamp(Self::MIN_OPACITY, Self::MAX_OPACITY);
        if (next_opacity - self.opacity).abs() <= f32::EPSILON {
            return false;
        }

        self.opacity = next_opacity;
        true
    }

    pub(in crate::features::pin) fn begin_ocr(&mut self) -> bool {
        if self.ocr.processing {
            return false;
        }
        self.ocr.processing = true;
        self.ocr.generation = self.ocr.generation.saturating_add(1);
        self.ocr.last_error = None;
        true
    }

    pub(in crate::features::pin) fn begin_ocr_source(&mut self) -> Option<(u64, ImageRef)> {
        self.begin_ocr().then(|| (self.ocr.generation, self.image.clone()))
    }

    pub(in crate::features::pin) fn record_ocr_unavailable(&mut self, reason: String) {
        if !self.ocr.processing {
            self.ocr.last_error = Some(reason);
        }
    }

    pub(in crate::features::pin) fn image(&self) -> ImageRef {
        self.image.clone()
    }

    pub(in crate::features::pin) fn ocr_result_revision(&self) -> u64 {
        self.ocr.result_revision
    }

    pub(in crate::features::pin) fn finish_ocr(&mut self, generation: u64, result: Result<Vec<OcrBlock>, String>) {
        if generation != self.ocr.generation || !self.ocr.processing {
            return;
        }
        self.ocr.processing = false;
        match result {
            Ok(blocks) => {
                self.ocr.blocks = blocks.into();
                self.ocr.result_revision = self.ocr.result_revision.saturating_add(1);
                self.ocr.last_error = None;
                self.ocr.hovered_index = None;
                self.ocr.active_text = None;
                self.ocr.selected_indices.clear();
                self.ocr.selection_rect = None;
            }
            Err(err) => {
                self.ocr.last_error = Some(err);
            }
        }
    }

    pub(in crate::features::pin) fn has_ocr_selection(&self) -> bool {
        self.ocr.active_text.as_ref().is_some_and(|selection| !selection.range().is_empty()) || !self.ocr.selected_indices.is_empty()
    }

    pub(in crate::features::pin) fn clear_ocr_selection(&mut self) -> bool {
        let had_selection = self.has_ocr_selection() || self.ocr.selection_rect.is_some();
        self.ocr.selected_indices.clear();
        self.ocr.active_text = None;
        self.ocr.selection_rect = None;
        had_selection
    }

    pub(in crate::features::pin) fn clear_active_text_selection(&mut self) -> bool {
        self.ocr.active_text.take().is_some()
    }

    pub(in crate::features::pin) fn set_hovered_block(&mut self, hovered_index: Option<usize>) -> bool {
        if self.ocr.hovered_index == hovered_index {
            return false;
        }
        self.ocr.hovered_index = hovered_index;
        true
    }

    pub(in crate::features::pin) fn set_selected_indices(&mut self, selected_indices: BTreeSet<usize>) -> bool {
        if self.ocr.selected_indices == selected_indices {
            return false;
        }
        self.ocr.selected_indices = selected_indices;
        true
    }

    pub(in crate::features::pin) fn set_single_selected_index(&mut self, selected_index: usize) -> bool {
        let mut next = BTreeSet::new();
        next.insert(selected_index);
        self.ocr.active_text = None;
        self.set_selected_indices(next)
    }

    pub(in crate::features::pin) fn is_block_selected(&self, block_index: usize) -> bool {
        self.ocr.selected_indices.contains(&block_index)
    }

    pub(in crate::features::pin) fn active_text_block_index(&self) -> Option<usize> {
        self.ocr.active_text.as_ref().map(|selection| selection.block_index)
    }

    pub(in crate::features::pin) fn start_selection_rect(&mut self, start: Point<Pixels>) -> bool {
        self.ocr.active_text = None;
        self.ocr.selection_rect = Some((start, start));
        true
    }

    pub(in crate::features::pin) fn update_selection_rect(&mut self, current: Point<Pixels>) -> bool {
        if let Some((start, existing_current)) = self.ocr.selection_rect {
            if existing_current == current {
                return false;
            }
            self.ocr.selection_rect = Some((start, current));
            return true;
        }
        false
    }

    pub(in crate::features::pin) fn clear_selection_rect(&mut self) -> bool {
        self.ocr.selection_rect.take().is_some()
    }

    pub(in crate::features::pin) fn start_text_selection(&mut self, block_index: usize, anchor: usize) -> bool {
        let next = PinTextSelection {
            block_index,
            anchor,
            head: anchor,
        };
        if self.ocr.active_text.as_ref() == Some(&next) {
            return false;
        }
        self.ocr.active_text = Some(next);
        self.ocr.selected_indices.clear();
        true
    }

    pub(in crate::features::pin) fn update_text_selection_head(&mut self, head: usize) -> bool {
        let Some(active_text) = self.ocr.active_text.as_mut() else {
            return false;
        };
        if active_text.head == head {
            return false;
        }
        active_text.head = head;
        true
    }

    pub(in crate::features::pin) fn selected_or_active_text(&self) -> Option<String> {
        if let Some(selection) = self.ocr.active_text.as_ref() {
            let block = self.ocr.blocks.get(selection.block_index)?;
            let range = selection.range();
            if !range.is_empty() {
                let text = block.text.chars().skip(range.start).take(range.end - range.start).collect::<String>();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }

        if self.ocr.selected_indices.is_empty() {
            return None;
        }

        let selected = self.ocr.selected_indices.iter().copied().collect::<Vec<_>>();
        format_selected_blocks(&self.ocr.blocks, &selected)
    }

    pub(in crate::features::pin) fn take_auto_ocr_request(&mut self) -> bool {
        if self.auto_ocr {
            self.auto_ocr = false;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flows::output::CaptureActionEffect;
    use minnow_imaging::{ImageAsset, RectF};

    fn test_image(width: u32, height: u32) -> ImageRef {
        ImageAsset::new(image::RgbaImage::from_pixel(width, height, image::Rgba([255, 0, 0, 255])))
    }

    fn test_session(request: PinRequest) -> PinSession {
        let lease = super::super::PinBudget::default().reserve(&request.image()).unwrap();
        PinSession::from_request(request, lease)
    }

    #[test]
    fn pin_initial_geometry_clamps_tiny_images_up_to_minimum_size() {
        let request = PinRequest::new(test_image(8, 10), None, false);
        let geometry = PinSession::initial_geometry(&request);

        assert_eq!(geometry.window_size(), size(px(24.0), px(30.0)));
        assert_eq!(geometry.min_size(), 24.0);
    }

    #[test]
    fn pin_initial_geometry_uses_source_bounds_before_image_dimensions() {
        let image = test_image(400, 300);
        let request = PinRequest::new(image, Some(RectF::new(32.0, 48.0, 120.0, 90.0)), false);
        let geometry = PinSession::initial_geometry(&request);

        assert_eq!(geometry.origin(), Some((32.0, 48.0)));
        assert_eq!(geometry.window_size(), size(px(120.0), px(90.0)));
    }

    #[test]
    fn pin_zoom_steps_clamp_between_min_and_max_bounds() {
        let mut session = test_session(PinRequest::new(test_image(320, 200), None, false));
        session.base_size = (480.0, 320.0);

        for _ in 0..200 {
            let _ = session.apply_zoom_step(1.0);
        }
        assert_eq!(session.zoom, PinSession::MAX_ZOOM);

        for _ in 0..400 {
            let _ = session.apply_zoom_step(-1.0);
        }
        assert_eq!(session.zoom, PinSession::min_zoom_for(session.base_size));
    }

    #[test]
    fn pin_opacity_steps_clamp_between_min_and_max_bounds() {
        let mut session = test_session(PinRequest::new(test_image(320, 200), None, false));
        session.base_size = (480.0, 320.0);

        for _ in 0..200 {
            let _ = session.apply_opacity_step(-1.0);
        }
        assert_eq!(session.opacity, PinSession::MIN_OPACITY);

        for _ in 0..200 {
            let _ = session.apply_opacity_step(1.0);
        }
        assert_eq!(session.opacity, PinSession::MAX_OPACITY);
    }

    #[test]
    fn pin_window_size_tracks_zoomed_dimensions() {
        let mut session = test_session(PinRequest::new(test_image(320, 200), None, false));

        let resized = session.apply_zoom_step(1.0).expect("zoom step should resize");

        assert_eq!(resized, size(px(352.0), px(220.0)));
        assert_eq!(session.window_size(), resized);
    }

    #[test]
    fn pin_request_auto_ocr_propagates_into_session_frame() {
        let request = PinRequest::new(test_image(80, 40), None, true);
        let mut session = test_session(request);

        assert!(session.take_auto_ocr_request());
        assert!(!session.take_auto_ocr_request());
    }

    #[test]
    fn pin_ocr_keeps_the_shared_capture_pixels() {
        let image = test_image(12, 6);
        let request = PinRequest::new(image.clone(), None, false);
        let mut session = test_session(request);
        let (_, source) = session.begin_ocr_source().expect("OCR should start");
        assert!(Arc::ptr_eq(&source, &image));
    }

    #[test]
    fn quota_is_held_until_the_last_session_and_render_frame_release_their_images() {
        let image = test_image(2, 2);
        let budget = super::super::PinBudget::with_limits(1, 1024);
        let lease = budget.reserve(&image).unwrap();
        let session = PinSession::from_request(PinRequest::new(image.clone(), None, false), lease);
        let cloned = session.clone();
        let frame = session.frame();
        drop(session);
        assert_eq!(budget.reserve(&image).unwrap_err().kind, minnow_application::ErrorKind::Busy);
        drop(cloned);
        assert_eq!(budget.reserve(&image).unwrap_err().kind, minnow_application::ErrorKind::Busy);
        drop(frame);
        assert!(budget.reserve(&image).is_ok());
    }

    #[test]
    fn output_failure_is_visible_in_the_local_frame_and_retries_the_frozen_text() {
        let mut session = test_session(PinRequest::new(test_image(12, 6), None, false));
        let request = OutputRequest::Text {
            text: "selected text".into(),
            message: "copied".into(),
        };
        let attempt = session.begin_output(&request).unwrap();
        assert!(session.frame().output_pending);
        assert!(
            session.begin_output(&request).is_none(),
            "repeated clicks cannot create another pending output"
        );
        let error = minnow_application::OperationError::new(minnow_application::ErrorKind::Io, "clipboard unavailable");
        session.output.finish(attempt, &CaptureActionEffect::Warning(error.clone()));
        let frame = session.frame();
        assert!(!frame.output_pending);
        assert_eq!(
            frame.output_request_error,
            Some(error),
            "local feedback does not require system notification delivery"
        );
        assert!(frame.output_preflight_error.is_none());
        assert!(frame.output_can_retry);
        assert!(matches!(
            session.retry_output(),
            Some(OutputRequest::Text { text, .. }) if text == "selected text"
        ));
        let retry = session.begin_output(&request).unwrap();
        assert!(retry > attempt);
        assert!(!session.output.finish(
            attempt,
            &crate::flows::output::interpret(Ok(minnow_application::ImageCommandResult::Copied))
        ));
        assert!(session.frame().output_pending, "an older completion cannot settle the retry");
    }

    #[test]
    fn preflight_failure_stays_visible_during_output_and_is_separate_from_its_retry() {
        let preflight = minnow_application::OperationError::new(minnow_application::ErrorKind::Conflict, "image publication failed");
        let request_error = minnow_application::OperationError::new(minnow_application::ErrorKind::Io, "clipboard unavailable");
        for fails in [false, true] {
            let mut session = test_session(PinRequest::new(test_image(12, 6), None, false));
            let request = OutputRequest::Text {
                text: "frozen text".into(),
                message: "copied".into(),
            };
            let attempt = session.begin_output(&request).unwrap();
            session.show_preflight_error(preflight.clone());
            let pending = session.frame();
            assert!(pending.output_pending);
            assert_eq!(pending.output_preflight_error, Some(preflight.clone()));
            assert!(pending.output_request_error.is_none());
            assert!(!pending.output_can_retry);

            let effect = if fails {
                CaptureActionEffect::Warning(request_error.clone())
            } else {
                crate::flows::output::interpret(Ok(minnow_application::ImageCommandResult::Copied))
            };
            assert!(session.output.finish(attempt, &effect));
            let settled = session.frame();
            assert!(!settled.output_pending);
            assert_eq!(settled.output_preflight_error, Some(preflight.clone()));
            assert_eq!(settled.output_request_error, fails.then(|| request_error.clone()));
            assert_eq!(settled.output_can_retry, fails);
            if fails {
                assert!(matches!(
                    session.retry_output(),
                    Some(OutputRequest::Text { text, .. }) if text == "frozen text"
                ));
                let later_preflight = minnow_application::OperationError::new(minnow_application::ErrorKind::Conflict, "newer publication failed");
                session.show_preflight_error(later_preflight.clone());
                let idle = session.frame();
                assert!(idle.output_request_error.is_none());
                assert_eq!(idle.output_preflight_error, Some(later_preflight));
                assert!(!idle.output_can_retry);
                assert!(
                    session.retry_output().is_none(),
                    "a new idle preflight error clears the previous failed request"
                );
            } else {
                assert!(session.retry_output().is_none());
            }
        }
    }

    #[test]
    fn failed_ocr_retry_keeps_the_previous_text_and_ignores_obsolete_attempts() {
        let request = PinRequest::new(test_image(80, 40), None, false);
        let mut session = test_session(request);
        let previous = OcrBlock::new(
            "retained text",
            minnow_application::ports::NormalizedOcrGeometry::new(0.5, 0.5, 0.4, 0.1, 0.0).unwrap(),
        );
        let (first, _) = session.begin_ocr_source().unwrap();
        assert!(session.begin_ocr_source().is_none(), "a second attempt must not overlap");
        session.finish_ocr(first, Ok(vec![previous.clone()]));
        assert!(Arc::ptr_eq(&session.frame().ocr.blocks, &session.frame().ocr.blocks));
        session.set_single_selected_index(0);

        let (retry, _) = session.begin_ocr_source().unwrap();
        session.finish_ocr(first, Ok(Vec::new()));
        assert!(session.ocr.processing, "an old attempt must not finish the retry");
        session.finish_ocr(retry, Err("model unavailable".into()));
        assert!(!session.ocr.processing);
        assert_eq!(session.ocr.blocks.as_ref(), &[previous]);
        assert_eq!(session.ocr.last_error.as_deref(), Some("model unavailable"));
        assert_eq!(session.selected_or_active_text().as_deref(), Some("retained text"));
    }

    #[test]
    fn selected_or_active_text_prefers_active_text_selection() {
        let mut session = test_session(PinRequest::new(test_image(320, 200), None, false));
        session.ocr = PinOcrState {
            processing: false,
            generation: 0,
            result_revision: 0,
            blocks: vec![
                OcrBlock::new(
                    "hello world",
                    minnow_application::ports::NormalizedOcrGeometry::new(0.5, 0.5, 0.4, 0.1, 0.0).unwrap(),
                ),
                OcrBlock::new(
                    "second line",
                    minnow_application::ports::NormalizedOcrGeometry::new(0.5, 0.6, 0.4, 0.1, 0.0).unwrap(),
                ),
            ]
            .into(),
            selected_indices: [0usize, 1usize].into_iter().collect(),
            hovered_index: None,
            active_text: Some(PinTextSelection {
                block_index: 0,
                anchor: 0,
                head: 5,
            }),
            selection_rect: None,
            last_error: None,
        };

        assert_eq!(session.selected_or_active_text(), Some("hello".to_string()));

        session.ocr.active_text = None;
        assert_eq!(session.selected_or_active_text(), Some("hello world second line".to_string()));
    }
}

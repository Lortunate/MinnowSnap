use image::RgbaImage;
use minnow_imaging::{CaptureGeometry, CapturedFrame, GeometryError, ImageAsset, ImageRef, ViewRect};

use crate::stitcher::{MAX_PREVIEW_PIXELS, allocate_image, preview_dimensions};
use crate::{CaptureError, ScrollStitcher, StitchConfig, StitchFrameResult};

/// Resource policy for a single scrolling capture.
#[derive(Clone, Copy, Debug)]
pub struct ScrollCaptureConfig {
    pub stitch: StitchConfig,
    /// Bounds the logical stitched output and canvas dimensions.
    pub max_output_pixels: u64,
    /// Bounds native width * height before cropping or grayscale analysis.
    /// Backing storage beyond these logical pixels is charged to working bytes.
    /// The backend owns the allocation needed to produce the source itself.
    pub max_source_pixels: u64,
    /// Bounds source allocations, owned input, analysis, scratch buffers, previews,
    /// and both old/new canvases during growth. Allocator overhead and copies
    /// retained by external consumers are outside this local budget.
    pub max_working_bytes: u64,
}

impl Default for ScrollCaptureConfig {
    fn default() -> Self {
        Self {
            stitch: StitchConfig::default(),
            max_output_pixels: 64 * 1024 * 1024,
            max_source_pixels: 32 * 1024 * 1024,
            max_working_bytes: 512 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapturePhase {
    Capturing,
    Finalizing,
    Ready,
    Cancelled,
    Failed,
}

/// The authoritative terminal result of one capture run.
///
/// Failed and cancelled runs retain any useful image already collected. Their
/// partial data must not be interpreted as successful completion.
#[derive(Clone, Debug)]
pub enum CaptureOutcome {
    Ready(ImageRef),
    Cancelled { partial: Option<ImageRef> },
    Failed { error: CaptureError, partial: Option<ImageRef> },
}

impl CaptureOutcome {
    pub fn phase(&self) -> CapturePhase {
        match self {
            Self::Ready(_) => CapturePhase::Ready,
            Self::Cancelled { .. } => CapturePhase::Cancelled,
            Self::Failed { .. } => CapturePhase::Failed,
        }
    }

    /// Returns a shared image without consuming it.
    pub fn image(&self) -> Option<ImageRef> {
        match self {
            Self::Ready(image) => Some(image.clone()),
            Self::Cancelled { partial } | Self::Failed { partial, .. } => partial.clone(),
        }
    }

    pub fn error(&self) -> Option<&CaptureError> {
        match self {
            Self::Failed { error, .. } => Some(error),
            _ => None,
        }
    }
}

/// Platform-independent state of one scrolling capture.
///
/// A session is never restarted. Finishing releases the stitching working set
/// and retains one immutable final image for repeated exports.
pub struct ScrollCapture {
    selection: ViewRect,
    geometry: Option<CaptureGeometry>,
    stitcher: Option<ScrollStitcher>,
    config: ScrollCaptureConfig,
    phase: CapturePhase,
    outcome: Option<CaptureOutcome>,
    unresolved: Option<crate::StitchRejection>,
}

impl ScrollCapture {
    pub fn new(selection: ViewRect) -> Result<Self, CaptureError> {
        Self::with_config(selection, ScrollCaptureConfig::default())
    }

    pub fn with_config(selection: ViewRect, config: ScrollCaptureConfig) -> Result<Self, CaptureError> {
        config.stitch.validate()?;
        let r = selection.0;
        if ![r.x, r.y, r.width, r.height].into_iter().all(f64::is_finite) {
            return Err(GeometryError::InvalidTransform.into());
        }
        if r.width <= 0.0 || r.height <= 0.0 {
            return Err(GeometryError::Empty.into());
        }
        if config.max_output_pixels == 0 {
            return Err(CaptureError::CapacityExceeded { max_pixels: 0 });
        }
        if config.max_source_pixels == 0 {
            return Err(CaptureError::SourceTooLarge { max_pixels: 0 });
        }
        if config.max_working_bytes == 0 {
            return Err(CaptureError::WorkingSetExceeded {
                max_bytes: 0,
                required_bytes: 1,
            });
        }
        Ok(Self {
            selection,
            geometry: None,
            unresolved: None,
            stitcher: Some(ScrollStitcher::with_limits(
                config.stitch,
                config.max_output_pixels,
                config.max_working_bytes,
            )),
            config,
            phase: CapturePhase::Capturing,
            outcome: None,
        })
    }

    pub fn phase(&self) -> CapturePhase {
        self.phase
    }

    pub fn geometry(&self) -> Option<&CaptureGeometry> {
        self.geometry.as_ref()
    }

    pub fn height(&self) -> u32 {
        if let Some(stitcher) = &self.stitcher {
            return stitcher.current_image().map_or(0, |(_, height)| height);
        }
        self.result().map_or(0, |image| image.height())
    }

    pub fn push_frame(&mut self, frame: &CapturedFrame) -> Result<StitchFrameResult, CaptureError> {
        if self.phase != CapturePhase::Capturing {
            return Err(CaptureError::Closed);
        }
        match self.process_frame(frame) {
            Ok(result) => Ok(result),
            Err(error) => {
                self.fail(error.clone());
                Err(error)
            }
        }
    }

    fn process_frame(&mut self, frame: &CapturedFrame) -> Result<StitchFrameResult, CaptureError> {
        if self.geometry.as_ref().is_some_and(|geometry| geometry != frame.geometry()) {
            return Err(CaptureError::TargetChanged);
        }

        let source_pixels = u64::from(frame.image().width()) * u64::from(frame.image().height());
        if source_pixels > self.config.max_source_pixels {
            return Err(CaptureError::SourceTooLarge {
                max_pixels: self.config.max_source_pixels,
            });
        }
        let selection = frame.geometry().resolve_selection(self.selection)?;
        let rect = selection.pixel_rect();
        let preview_pixels = u64::from(rect.width)
            .saturating_mul(u64::from(self.height()) + u64::from(rect.height))
            .min(MAX_PREVIEW_PIXELS);
        // Reserve cache/projection replacement buffers without counting any
        // number of clones an external preview consumer may retain.
        let retained_bytes = (frame.image().pixels().as_raw().capacity() as u64).saturating_add(preview_pixels.saturating_mul(12));
        self.stitcher.as_ref().expect("capturing state owns a stitcher").admit_frame(
            rect.width,
            rect.height,
            u64::from(rect.width) * u64::from(rect.height) * 4,
            retained_bytes,
        )?;

        // Captured desktop content is opaque. Some native backends supply zero
        // alpha even though their RGB channels contain valid screen pixels.
        // Copy only the resolved logical pixels, even for a full-frame crop.
        // ImageBuffer permits trailing storage and spare capacity; cloning it
        // would allocate that storage despite the compact-input admission.
        let mut pixels = allocate_image(rect.width, rect.height)?;
        let row_bytes = rect.width as usize * 4;
        let source = frame.image().pixels();
        for (y, row) in pixels.as_mut().chunks_exact_mut(row_bytes).enumerate() {
            let start = ((rect.y as usize + y) * source.width() as usize + rect.x as usize) * 4;
            row.copy_from_slice(&source.as_raw()[start..start + row_bytes]);
        }
        for pixel in pixels.pixels_mut() {
            pixel[3] = 255;
        }
        let stitcher = self.stitcher.as_mut().expect("capturing state owns a stitcher");
        let result = stitcher.try_process_frame_detailed(pixels, retained_bytes)?;
        self.unresolved = result.warning;
        self.geometry.get_or_insert_with(|| frame.geometry().clone());
        Ok(result)
    }

    /// Produces a preview on the worker that owns the stitching state.
    pub fn preview(&mut self, target_width: u32) -> Option<ImageRef> {
        if target_width == 0 {
            return None;
        }
        if let Some(stitcher) = &mut self.stitcher {
            return stitcher.make_thumbnail(target_width).map(ImageAsset::new);
        }
        self.result().map(|image| thumbnail(&image, target_width))
    }

    /// Finalizes once and returns the same shared asset on subsequent calls.
    pub fn finish(&mut self) -> Result<ImageRef, CaptureError> {
        if let Some(outcome) = &self.outcome {
            return match outcome {
                CaptureOutcome::Ready(image) => Ok(image.clone()),
                CaptureOutcome::Cancelled { .. } => Err(CaptureError::Cancelled),
                CaptureOutcome::Failed { error, .. } => Err(error.clone()),
            };
        }

        if let Some(reason) = self.unresolved {
            let error = CaptureError::Incomplete { reason };
            self.fail(error.clone());
            return Err(error);
        }
        self.phase = CapturePhase::Finalizing;
        match self.release_working_set() {
            Some(image) => {
                self.set_outcome(CaptureOutcome::Ready(image.clone()));
                Ok(image)
            }
            None => {
                self.set_outcome(CaptureOutcome::Failed {
                    error: CaptureError::Empty,
                    partial: None,
                });
                Err(CaptureError::Empty)
            }
        }
    }

    pub fn cancel(&mut self) -> CaptureOutcome {
        if let Some(outcome) = &self.outcome {
            return outcome.clone();
        }
        let partial = self.release_working_set();
        self.set_outcome(CaptureOutcome::Cancelled { partial });
        self.outcome.clone().expect("terminal outcome was installed")
    }

    pub fn fail(&mut self, error: CaptureError) -> CaptureOutcome {
        if let Some(outcome) = &self.outcome {
            return outcome.clone();
        }
        let partial = self.release_working_set();
        self.set_outcome(CaptureOutcome::Failed { error, partial });
        self.outcome.clone().expect("terminal outcome was installed")
    }

    /// Reads retained data; exports must not consume the session's image.
    pub fn result(&self) -> Option<ImageRef> {
        self.outcome.as_ref().and_then(CaptureOutcome::image)
    }

    pub fn outcome(&self) -> Option<CaptureOutcome> {
        self.outcome.clone()
    }

    fn release_working_set(&mut self) -> Option<ImageRef> {
        self.stitcher.take().and_then(ScrollStitcher::into_final_image).map(ImageAsset::new)
    }

    fn set_outcome(&mut self, outcome: CaptureOutcome) {
        self.phase = outcome.phase();
        self.outcome = Some(outcome);
    }
}

pub(crate) fn thumbnail(image: &ImageRef, width: u32) -> ImageRef {
    let (width, height) = preview_dimensions(image.width(), image.height(), width);
    if image.dimensions() == (width, height) {
        return image.clone();
    }
    let pixels: RgbaImage = image::imageops::thumbnail(image.pixels(), width, height);
    ImageAsset::new(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use minnow_imaging::{DisplayId, RectF, ScreenPoint};
    use std::sync::Arc;
    const MAX_PREVIEW_EDGE: u32 = 4096;

    pub(super) fn frame(width: u32, height: u32, scale: f64) -> CapturedFrame {
        CapturedFrame::new(
            ImageAsset::new(RgbaImage::from_pixel(width, height, Rgba([30, 60, 90, 0]))),
            CaptureGeometry {
                display_id: DisplayId("display-a".into()),
                screen_origin: ScreenPoint { x: -500.0, y: 40.0 },
                scale_factor: scale,
                pixel_size: (width, height),
            },
        )
        .unwrap()
    }

    #[test]
    fn finished_image_survives_repeated_reads_and_export_attempts() {
        let mut session = ScrollCapture::new(ViewRect(RectF::new(2.0, 3.0, 20.0, 15.0))).unwrap();
        session.push_frame(&frame(100, 100, 1.5)).unwrap();
        let first = session.finish().unwrap();
        assert_eq!(first.dimensions(), (30, 23));
        assert!(first.pixels().pixels().all(|pixel| pixel[3] == 255));
        let retained = session.result().unwrap();
        let retry = session.finish().unwrap();
        assert!(Arc::ptr_eq(&first, &retained));
        assert!(Arc::ptr_eq(&first, &retry));
        assert!(session.push_frame(&frame(100, 100, 1.5)).is_err());
        assert!(Arc::ptr_eq(&first, &session.result().unwrap()));
    }

    #[test]
    fn changed_target_fails_with_previous_image_retained() {
        let selection = ViewRect(RectF::new(0.0, 0.0, 40.0, 30.0));
        let mut session = ScrollCapture::new(selection).unwrap();
        session.push_frame(&frame(80, 60, 2.0)).unwrap();
        let initial = frame(80, 60, 2.0);
        let mut geometry = initial.geometry().clone();
        geometry.display_id = DisplayId("display-b".into());
        let changed = CapturedFrame::new(initial.image().clone(), geometry).unwrap();
        assert_eq!(session.push_frame(&changed).unwrap_err(), CaptureError::TargetChanged);
        assert_eq!(session.phase(), CapturePhase::Failed);
        assert_eq!(session.result().unwrap().dimensions(), (80, 60));
        assert!(matches!(
            session.outcome(),
            Some(CaptureOutcome::Failed {
                error: CaptureError::TargetChanged,
                ..
            })
        ));
    }

    #[test]
    fn reaching_output_limit_does_not_reject_stationary_frames() {
        let selection = ViewRect(RectF::new(0.0, 0.0, 40.0, 30.0));
        let mut session = ScrollCapture::with_config(
            selection,
            ScrollCaptureConfig {
                max_output_pixels: 40 * 30,
                ..ScrollCaptureConfig::default()
            },
        )
        .unwrap();
        let input = frame(40, 30, 1.0);
        session.push_frame(&input).unwrap();
        session.push_frame(&input).unwrap();
        assert_eq!(session.phase(), CapturePhase::Capturing);
        assert_eq!(session.finish().unwrap().dimensions(), (40, 30));
    }

    #[test]
    fn source_and_working_set_admission_fail_without_losing_existing_pixels() {
        let selection = ViewRect(RectF::new(0.0, 0.0, 40.0, 30.0));
        let input = frame(40, 30, 1.0);
        let mut oversized = ScrollCapture::with_config(
            selection,
            ScrollCaptureConfig {
                max_source_pixels: 1199,
                ..ScrollCaptureConfig::default()
            },
        )
        .unwrap();
        assert_eq!(
            oversized.push_frame(&input).unwrap_err(),
            CaptureError::SourceTooLarge { max_pixels: 1199 }
        );
        assert!(oversized.result().is_none());

        let mut limited = ScrollCapture::with_config(
            selection,
            ScrollCaptureConfig {
                max_working_bytes: 48_000,
                ..ScrollCaptureConfig::default()
            },
        )
        .unwrap();
        limited.push_frame(&input).unwrap();
        assert!(matches!(limited.push_frame(&input), Err(CaptureError::WorkingSetExceeded { .. })));
        assert_eq!(limited.phase(), CapturePhase::Failed);
        assert_eq!(limited.result().unwrap().dimensions(), (40, 30));
    }

    #[test]
    fn invalid_stitch_config_is_rejected_before_a_session_starts() {
        let result = ScrollCapture::with_config(
            ViewRect(RectF::new(0.0, 0.0, 1.0, 1.0)),
            ScrollCaptureConfig {
                stitch: StitchConfig {
                    min_matching_rows: f32::NAN,
                    ..StitchConfig::default()
                },
                ..ScrollCaptureConfig::default()
            },
        );
        assert!(matches!(result, Err(CaptureError::InvalidStitchConfig { field: "min_matching_rows" })));
    }

    #[test]
    fn oversized_backing_storage_is_counted_but_only_logical_pixels_are_copied() {
        const SOURCE_BYTES: usize = 1024 * 1024;
        const WORKING_BYTES: u64 = 1_052_672;
        for raw_len in [4, SOURCE_BYTES] {
            let mut raw = Vec::with_capacity(SOURCE_BYTES);
            raw.resize(raw_len, 0);
            raw[..4].copy_from_slice(&[30, 60, 90, 0]);
            let input = CapturedFrame::new(
                ImageAsset::new(RgbaImage::from_raw(1, 1, raw).unwrap()),
                frame(1, 1, 1.0).geometry().clone(),
            )
            .unwrap();
            let mut session = ScrollCapture::with_config(
                ViewRect(RectF::new(0.0, 0.0, 1.0, 1.0)),
                ScrollCaptureConfig {
                    max_working_bytes: WORKING_BYTES,
                    ..ScrollCaptureConfig::default()
                },
            )
            .unwrap();
            session.push_frame(&input).unwrap();
            let result = session.finish().unwrap();
            assert_eq!(result.pixels().as_raw(), &[30, 60, 90, 255]);
            let retained_bytes = input.image().pixels().as_raw().capacity() + result.pixels().as_raw().capacity();
            assert!(retained_bytes as u64 <= WORKING_BYTES);
            assert_eq!(input.image().pixels().get_pixel(0, 0)[3], 0);
        }
    }

    #[test]
    fn cancellation_and_empty_finish_are_distinct_terminal_results() {
        let selection = ViewRect(RectF::new(0.0, 0.0, 10.0, 10.0));
        let mut cancelled = ScrollCapture::new(selection).unwrap();
        cancelled.push_frame(&frame(10, 10, 1.0)).unwrap();
        assert!(matches!(cancelled.cancel(), CaptureOutcome::Cancelled { partial: Some(_) }));
        assert_eq!(cancelled.finish().unwrap_err(), CaptureError::Cancelled);

        let mut empty = ScrollCapture::new(selection).unwrap();
        assert_eq!(empty.finish().unwrap_err(), CaptureError::Empty);
        assert_eq!(empty.phase(), CapturePhase::Failed);
    }

    #[test]
    fn previews_never_upscale_and_tall_frames_have_bounded_dimensions() {
        let mut small = ScrollCapture::new(ViewRect(RectF::new(0.0, 0.0, 20.0, 15.0))).unwrap();
        small.push_frame(&frame(40, 30, 2.0)).unwrap();
        assert_eq!(small.preview(500).unwrap().dimensions(), (40, 30));
        assert!(small.preview(0).is_none());

        let mut tall = ScrollCapture::new(ViewRect(RectF::new(0.0, 0.0, 5.0, 5000.0))).unwrap();
        tall.push_frame(&frame(5, 5000, 1.0)).unwrap();
        let active_preview = tall.preview(500).unwrap();
        assert!(active_preview.width() <= 5);
        assert!(active_preview.height() <= MAX_PREVIEW_EDGE);
        tall.finish().unwrap();
        let terminal_preview = tall.preview(500).unwrap();
        assert!(terminal_preview.width() <= 5);
        assert!(terminal_preview.height() <= MAX_PREVIEW_EDGE);
        assert!(tall.preview(0).is_none());
        let (width, height) = preview_dimensions(4096, 4096, 4096);
        assert!(u64::from(width) * u64::from(height) <= MAX_PREVIEW_PIXELS);
    }
}

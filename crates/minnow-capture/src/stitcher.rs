use crate::matching::{FrameAnalysis, match_frames};
use crate::matching::{MatchEvidence, StitchConfig, StitchRejection};
use image::RgbaImage;

use crate::{CaptureError, reserve};

pub(crate) const MAX_PREVIEW_PIXELS: u64 = 1024 * 1024;
const MAX_PREVIEW_EDGE: u32 = 4096;
const DEFAULT_MAX_OUTPUT_PIXELS: u64 = 64 * 1024 * 1024;
const DEFAULT_MAX_WORKING_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StitchFrameStatus {
    Appended,
    Stationary,
    LowConfidence,
    Reverse,
}

#[derive(Debug, Clone)]
pub struct StitchFrameResult {
    pub status: StitchFrameStatus,
    pub height: i32,
    pub warning: Option<StitchRejection>,
    pub evidence: Option<MatchEvidence>,
}

#[derive(Debug, Clone, Copy)]
struct StitchAppendPlan {
    trim_amount: u32,
    append_start_y: u32,
    append_end_y: u32,
}

#[derive(Default)]
struct ThumbnailCache {
    target_width: u32,
    source_width: u32,
    source_height: u32,
    dirty_from: u32,
    image: Option<RgbaImage>,
}

impl ThumbnailCache {
    fn reset(&mut self) {
        self.target_width = 0;
        self.source_width = 0;
        self.source_height = 0;
        self.dirty_from = 0;
        self.image = None;
    }

    fn mark_dirty_from(&mut self, source_row: u32) {
        if self.image.is_none() {
            return;
        }
        self.dirty_from = self.dirty_from.min(source_row);
    }
}

pub struct ScrollStitcher {
    canvas: Option<RgbaImage>,
    valid_height: u32,
    // Matching stays anchored to the frame represented by the canvas tail.
    // Rejected observations must not advance this anchor.
    accepted_analysis: Option<FrameAnalysis>,
    config: StitchConfig,
    thumbnail: ThumbnailCache,
    max_output_pixels: u64,
    max_working_bytes: u64,
}

impl Default for ScrollStitcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollStitcher {
    pub fn new() -> Self {
        Self::with_config(StitchConfig::default())
    }

    /// Creates a stitcher whose configuration is checked on frame admission.
    /// Invalid values return `CaptureError::InvalidStitchConfig` from processing.
    pub fn with_config(config: StitchConfig) -> Self {
        Self::with_limits(config, DEFAULT_MAX_OUTPUT_PIXELS, DEFAULT_MAX_WORKING_BYTES)
    }

    pub(crate) fn with_limits(config: StitchConfig, max_output_pixels: u64, max_working_bytes: u64) -> Self {
        Self {
            canvas: None,
            valid_height: 0,
            accepted_analysis: None,
            config,
            thumbnail: ThumbnailCache::default(),
            max_output_pixels,
            max_working_bytes,
        }
    }

    pub fn current_image(&self) -> Option<(&RgbaImage, u32)> {
        self.canvas.as_ref().map(|img| (img, self.valid_height))
    }

    pub fn get_final_image(&self) -> Option<RgbaImage> {
        let (canvas, h) = self.current_image()?;
        if h == 0 || canvas.width() == 0 {
            return None;
        }
        let mut final_img = RgbaImage::new(canvas.width(), h);
        Self::copy_region(canvas, 0, &mut final_img, 0, h);
        Some(final_img)
    }

    /// Transfers the canvas without allocating a second full output image.
    pub fn into_final_image(mut self) -> Option<RgbaImage> {
        let canvas = self.canvas.take()?;
        let width = canvas.width();
        if width == 0 || self.valid_height == 0 {
            return None;
        }
        let mut pixels = canvas.into_raw();
        pixels.truncate(width as usize * self.valid_height as usize * 4);
        RgbaImage::from_raw(width, self.valid_height, pixels)
    }

    /// Checks the peak before cropping or analyzing the incoming frame.
    ///
    /// `retained_bytes` includes the caller's source frame and preview slots.
    /// `input_bytes` is the actual allocation of the owned RGBA observation.
    /// A growing canvas must fit beside the old canvas until copying completes.
    pub(crate) fn admit_frame(&self, width: u32, height: u32, input_bytes: u64, retained_bytes: u64) -> Result<u64, CaptureError> {
        self.config.validate()?;
        let pixels = u64::from(width) * u64::from(height);
        if pixels == 0 || pixels > self.max_output_pixels || width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err(CaptureError::CapacityExceeded {
                max_pixels: self.max_output_pixels,
            });
        }
        let required_bytes = [
            retained_bytes,
            input_bytes, // actual owned RGBA allocation, including spare capacity
            FrameAnalysis::required_bytes(width, height),
            self.accepted_analysis.as_ref().map_or(0, FrameAnalysis::allocated_bytes),
            self.canvas.as_ref().map_or(0, |canvas| canvas.as_raw().capacity() as u64),
            self.thumbnail.image.as_ref().map_or(0, |image| image.as_raw().capacity() as u64),
        ]
        .into_iter()
        .fold(0u64, u64::saturating_add);
        if required_bytes > self.max_working_bytes {
            return Err(CaptureError::WorkingSetExceeded {
                max_bytes: self.max_working_bytes,
                required_bytes,
            });
        }
        Ok(self.max_working_bytes - required_bytes)
    }

    pub fn make_thumbnail(&mut self, target_width: u32) -> Option<RgbaImage> {
        let canvas = self.canvas.as_ref()?;
        let valid_h = self.valid_height;
        let source_width = canvas.width();
        if target_width == 0 || valid_h == 0 || source_width == 0 {
            return None;
        }

        let (target_width, target_height) = preview_dimensions(source_width, valid_h, target_width);
        let scale = target_width as f32 / source_width as f32;

        let requires_full_render = self.thumbnail.image.is_none()
            || self.thumbnail.target_width != target_width
            || self.thumbnail.source_width != source_width
            || valid_h < self.thumbnail.source_height
            || u64::from(valid_h) * u64::from(target_width) / u64::from(source_width) != u64::from(target_height);

        if requires_full_render {
            let cropped = image::imageops::crop_imm(canvas, 0, 0, source_width, valid_h);
            self.thumbnail.image = Some(image::imageops::thumbnail(&*cropped, target_width, target_height));
            self.thumbnail.target_width = target_width;
            self.thumbnail.source_width = source_width;
            self.thumbnail.source_height = valid_h;
            self.thumbnail.dirty_from = valid_h;
            return self.thumbnail.image.clone();
        }

        let dirty_from = self.thumbnail.dirty_from.min(valid_h);
        if dirty_from >= valid_h && self.thumbnail.source_height == valid_h {
            return self.thumbnail.image.clone();
        }

        let previous_thumbnail = self.thumbnail.image.as_ref()?;
        let mut next_thumbnail = if previous_thumbnail.height() == target_height {
            previous_thumbnail.clone()
        } else {
            let mut resized = RgbaImage::new(target_width, target_height);
            let preserved_rows = ((dirty_from as f32) * scale).floor() as u32;
            let copy_rows = preserved_rows.min(previous_thumbnail.height()).min(target_height);
            if copy_rows > 0 {
                Self::copy_region(previous_thumbnail, 0, &mut resized, 0, copy_rows);
            }
            resized
        };

        let source_start = if valid_h > self.thumbnail.source_height {
            dirty_from.saturating_sub(1)
        } else {
            dirty_from
        };
        let target_start = ((source_start as f32) * scale).floor() as u32;
        let source_h = valid_h.saturating_sub(source_start);
        let target_h = target_height.saturating_sub(target_start);

        if source_h > 0 && target_h > 0 {
            let strip = image::imageops::crop_imm(canvas, 0, source_start, source_width, source_h);
            let resized_strip = image::imageops::thumbnail(&*strip, target_width, target_h);
            Self::copy_region(&resized_strip, 0, &mut next_thumbnail, target_start, target_h);
        }

        self.thumbnail.image = Some(next_thumbnail);
        self.thumbnail.target_width = target_width;
        self.thumbnail.source_width = source_width;
        self.thumbnail.source_height = valid_h;
        self.thumbnail.dirty_from = valid_h;
        self.thumbnail.image.clone()
    }

    /// Processes an owned observation, retaining the last committed canvas on
    /// resource errors. Unreliable overlap is reported in the successful result.
    pub fn process_frame_detailed(&mut self, new_image: RgbaImage) -> Result<StitchFrameResult, CaptureError> {
        self.try_process_frame_detailed(new_image, 0)
    }

    pub(crate) fn try_process_frame_detailed(&mut self, new_image: RgbaImage, retained_bytes: u64) -> Result<StitchFrameResult, CaptureError> {
        let allocation_bytes = self.admit_frame(
            new_image.width(),
            new_image.height(),
            new_image.as_raw().capacity() as u64,
            retained_bytes,
        )?;
        if self.accepted_analysis.is_none() {
            self.initialize_canvas(new_image)?;
            return Ok(StitchFrameResult {
                status: StitchFrameStatus::Appended,
                height: self.valid_height as i32,
                warning: None,
                evidence: None,
            });
        }
        if self.accepted_analysis.as_ref().is_some_and(|anchor| anchor.is_identical(&new_image)) {
            return Ok(StitchFrameResult {
                status: StitchFrameStatus::Stationary,
                height: self.valid_height as i32,
                warning: None,
                evidence: Some(crate::matching::stationary_evidence(new_image.height())),
            });
        }
        let next = FrameAnalysis::from_image(&new_image)?;
        let decision = match_frames(self.accepted_analysis.as_ref().expect("initialized"), &next, self.config);
        let mut result = StitchFrameResult {
            status: StitchFrameStatus::LowConfidence,
            height: self.valid_height as i32,
            warning: decision.reason,
            evidence: decision.evidence,
        };
        if result.warning.is_some() {
            return Ok(result);
        }
        let evidence = result.evidence.as_mut().expect("accepted match has evidence");
        if evidence.displacement == 0 {
            result.status = StitchFrameStatus::Stationary;
        } else if evidence.displacement < 0 {
            result.status = StitchFrameStatus::Reverse;
            result.warning = Some(StitchRejection::Reverse);
        } else {
            // One coordinate pair drives both seam verification and canvas commit.
            // The next frame's header is omitted; its footer replaces the old one.
            let seam = evidence.next_seam.expect("forward match has seam");
            let previous_seam = seam + evidence.displacement as u32;
            let plan = StitchAppendPlan {
                trim_amount: new_image.height() - previous_seam,
                append_start_y: seam,
                append_end_y: new_image.height(),
            };
            evidence.output_seam = Some(self.valid_height - plan.trim_amount);
            if self.execute_stitch(&new_image, plan, allocation_bytes)? {
                self.accepted_analysis = Some(next);
                result.status = StitchFrameStatus::Appended;
                result.height = self.valid_height as i32;
            } else {
                result.warning = Some(StitchRejection::InsufficientOverlap);
            }
        }
        Ok(result)
    }

    fn initialize_canvas(&mut self, first_image: RgbaImage) -> Result<(), CaptureError> {
        let h = first_image.height();
        let analysis = FrameAnalysis::from_image(&first_image)?;
        // The admitted input becomes the first canvas; no triple-height copy.
        self.canvas = Some(first_image);
        self.valid_height = h;
        self.accepted_analysis = Some(analysis);
        self.thumbnail.reset();
        Ok(())
    }

    fn execute_stitch(&mut self, new_image: &RgbaImage, plan: StitchAppendPlan, allocation_bytes: u64) -> Result<bool, CaptureError> {
        let Some(canvas) = self.canvas.as_mut() else {
            return Ok(false);
        };

        let previous_valid_height = self.valid_height;
        // Replace the complete tail from the verified seam, including the old
        // footer. The new observation supplies exactly one current footer.
        let content_end = self.valid_height;
        if plan.trim_amount > content_end {
            return Ok(false);
        }

        let keep_h = content_end - plan.trim_amount;
        let append_h = plan.append_end_y.saturating_sub(plan.append_start_y);
        let new_total_h = keep_h.checked_add(append_h).ok_or(CaptureError::CapacityExceeded {
            max_pixels: self.max_output_pixels,
        })?;
        let width = canvas.width();
        if u64::from(width) * u64::from(new_total_h) > self.max_output_pixels {
            return Err(CaptureError::CapacityExceeded {
                max_pixels: self.max_output_pixels,
            });
        }

        if new_total_h > canvas.height() {
            let available_height = (allocation_bytes / 4 / u64::from(width))
                .min(self.max_output_pixels / u64::from(width))
                .min(u64::from(u32::MAX)) as u32;
            if available_height < new_total_h {
                return Err(CaptureError::WorkingSetExceeded {
                    max_bytes: self.max_working_bytes,
                    required_bytes: self
                        .max_working_bytes
                        .saturating_sub(allocation_bytes)
                        .saturating_add(u64::from(width) * u64::from(new_total_h) * 4),
                });
            }
            // Prefer modest headroom, but always fall back to what fits beside
            // the old canvas. Matching and seam decisions are unchanged.
            let new_cap = canvas.height().saturating_add(canvas.height() / 2).max(new_total_h).min(available_height);
            let mut new_canvas = allocate_image(width, new_cap)?;
            Self::copy_region(canvas, 0, &mut new_canvas, 0, keep_h);
            self.canvas = Some(new_canvas);
        }

        let Some(canvas) = self.canvas.as_mut() else {
            return Ok(false);
        };

        if append_h > 0 {
            Self::copy_region(new_image, plan.append_start_y, canvas, keep_h, append_h);
        }

        self.valid_height = new_total_h;
        self.thumbnail.mark_dirty_from(keep_h.min(previous_valid_height));
        Ok(true)
    }

    fn copy_region(src: &RgbaImage, src_y: u32, dest: &mut RgbaImage, dest_y: u32, height: u32) {
        if height == 0 {
            return;
        }
        let width_bytes = src.width() as usize * 4;
        let copy_bytes = height as usize * width_bytes;
        let src_offset = src_y as usize * width_bytes;
        let dest_offset = dest_y as usize * width_bytes;

        let src_raw = src.as_raw();
        let dest_raw: &mut [u8] = dest.as_mut();

        if src_offset + copy_bytes <= src_raw.len() && dest_offset + copy_bytes <= dest_raw.len() {
            dest_raw[dest_offset..dest_offset + copy_bytes].copy_from_slice(&src_raw[src_offset..src_offset + copy_bytes]);
        }
    }
}

pub(crate) fn allocate_image(width: u32, height: u32) -> Result<RgbaImage, CaptureError> {
    let len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(CaptureError::AllocationFailed)?;
    let mut pixels = Vec::new();
    reserve(&mut pixels, len)?;
    pixels.resize(len, 0);
    RgbaImage::from_raw(width, height, pixels).ok_or(CaptureError::AllocationFailed)
}

pub(crate) fn preview_dimensions(width: u32, height: u32, target_width: u32) -> (u32, u32) {
    let pixels = u64::from(width) * u64::from(height);
    let scale = 1.0f64
        .min(f64::from(target_width) / f64::from(width))
        .min(f64::from(MAX_PREVIEW_EDGE) / f64::from(width))
        .min(f64::from(MAX_PREVIEW_EDGE) / f64::from(height))
        .min((MAX_PREVIEW_PIXELS as f64 / pixels as f64).sqrt());
    let preview_width = (f64::from(width) * scale).floor().max(1.0) as u32;
    let preview_height = ((u64::from(height) * u64::from(preview_width)) / u64::from(width)).clamp(1, u64::from(MAX_PREVIEW_EDGE)) as u32;
    (preview_width, preview_height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage, imageops};

    fn source(width: u32, height: u32) -> RgbaImage {
        let mut img = RgbaImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let r = (((x * 31) ^ (y * 17)) & 0xff) as u8;
                let g = (((x * 13) ^ (y * 57)) & 0xff) as u8;
                let b = (((x * 97) ^ (y * 29)) & 0xff) as u8;
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }
        img
    }

    fn crop_frame(source: &RgbaImage, y: u32, h: u32) -> RgbaImage {
        imageops::crop_imm(source, 0, y, source.width(), h).to_image()
    }

    // Spatially varied content with deterministic pixels and sticky controls.
    fn scroll_source() -> RgbaImage {
        RgbaImage::from_fn(240, 600, |x, y| {
            let mut hash = x.wrapping_mul(0x9e37_79b9) ^ y.wrapping_mul(0x85eb_ca6b);
            hash = (hash ^ (hash >> 16)).wrapping_mul(0x7feb_352d);
            hash ^= hash >> 15;
            let value = if x < 96 { 100 + (hash % 20) as u8 } else { hash as u8 };
            Rgba([value, value, value, 255])
        })
    }

    fn sticky_frame(source: &RgbaImage, offset: u32, content_height: u32) -> RgbaImage {
        const STICKY_HEIGHT: u32 = 12;
        RgbaImage::from_fn(source.width(), content_height + STICKY_HEIGHT * 2, |x, y| {
            if y < STICKY_HEIGHT {
                Rgba([20, 160, 50 + (x % 12) as u8, 255])
            } else if y >= STICKY_HEIGHT + content_height {
                Rgba([200, 50, 180 + (x % 12) as u8, 255])
            } else {
                *source.get_pixel(x, offset + y - STICKY_HEIGHT)
            }
        })
    }

    #[test]
    fn slow_scroll_accumulates_against_committed_frame_and_keeps_one_footer() {
        for repeated_bottom_row in [false, true] {
            let mut stitcher = ScrollStitcher::new();
            let mut src = scroll_source();
            if repeated_bottom_row {
                // At offset 4 the last content row also looks sticky. At offset
                // 8 it moves again, so the detected footer extent changes.
                for x in 0..src.width() {
                    let pixel = *src.get_pixel(x, 179);
                    src.put_pixel(x, 183, pixel);
                }
            }
            stitcher.process_frame_detailed(sticky_frame(&src, 0, 180)).unwrap();

            for offset in (2..=16).step_by(2) {
                let result = stitcher.process_frame_detailed(sticky_frame(&src, offset, 180)).unwrap();
                let committed_offset = offset;
                let expected_status = StitchFrameStatus::Appended;
                assert_eq!(result.status, expected_status, "offset {offset}, repeated row {repeated_bottom_row}");
                assert_eq!(
                    stitcher.get_final_image().unwrap(),
                    sticky_frame(&src, 0, 180 + committed_offset),
                    "offset {offset}, repeated row {repeated_bottom_row}"
                );
            }
        }
    }

    #[test]
    fn stitcher_marks_stationary_for_same_frame() {
        let mut stitcher = ScrollStitcher::new();
        let src = source(200, 420);
        let first = crop_frame(&src, 10, 160);
        assert_eq!(
            stitcher.process_frame_detailed(first.clone()).unwrap().status,
            StitchFrameStatus::Appended
        );
        assert_eq!(stitcher.process_frame_detailed(first).unwrap().status, StitchFrameStatus::Stationary);
    }

    #[test]
    fn reverse_then_return_only_appends_content_beyond_committed_frame() {
        let mut stitcher = ScrollStitcher::new();
        let src = scroll_source();
        stitcher.process_frame_detailed(sticky_frame(&src, 40, 180)).unwrap();

        let mut committed_offset = 40;
        for (offset, status) in [
            (56, StitchFrameStatus::Appended),
            (48, StitchFrameStatus::Reverse),
            (40, StitchFrameStatus::Reverse),
            (52, StitchFrameStatus::Reverse),
            (56, StitchFrameStatus::Stationary),
            (60, StitchFrameStatus::Appended),
            (72, StitchFrameStatus::Appended),
        ] {
            let result = stitcher.process_frame_detailed(sticky_frame(&src, offset, 180)).unwrap();
            assert_eq!(result.status, status, "offset {offset}");
            if status == StitchFrameStatus::Appended {
                committed_offset = offset;
            }
            assert_eq!(
                stitcher.get_final_image().unwrap(),
                sticky_frame(&src, 40, 180 + committed_offset - 40),
                "offset {offset}"
            );
        }
    }

    #[test]
    fn rejected_observation_preserves_anchor_and_next_frames_are_pixel_complete() {
        let src = scroll_source();
        for bad_frame in [RgbaImage::from_pixel(240, 204, Rgba([255, 255, 255, 255])), sticky_frame(&src, 18, 181)] {
            let mut stitcher = ScrollStitcher::new();
            stitcher.process_frame_detailed(sticky_frame(&src, 0, 180)).unwrap();
            assert_eq!(
                stitcher.process_frame_detailed(sticky_frame(&src, 12, 180)).unwrap().status,
                StitchFrameStatus::Appended
            );
            let before_rejection = stitcher.get_final_image().unwrap();
            assert_eq!(
                stitcher.process_frame_detailed(bad_frame).unwrap().status,
                StitchFrameStatus::LowConfidence
            );
            assert_eq!(stitcher.get_final_image().unwrap(), before_rejection);

            for offset in [24, 36] {
                assert_eq!(
                    stitcher.process_frame_detailed(sticky_frame(&src, offset, 180)).unwrap().status,
                    StitchFrameStatus::Appended
                );
                assert_eq!(stitcher.get_final_image().unwrap(), sticky_frame(&src, 0, 180 + offset));
            }
        }
    }

    #[test]
    fn public_processing_returns_resource_error_and_retains_committed_pixels() {
        let src = scroll_source();
        let first = sticky_frame(&src, 0, 180);
        let mut stitcher = ScrollStitcher::with_limits(StitchConfig::default(), u64::from(first.width()) * u64::from(first.height()), 4_000_000);
        stitcher.process_frame_detailed(first.clone()).unwrap();
        assert!(matches!(
            stitcher.process_frame_detailed(sticky_frame(&src, 12, 180)),
            Err(CaptureError::CapacityExceeded { .. })
        ));
        assert_eq!(stitcher.get_final_image().unwrap(), first);
    }

    #[test]
    fn direct_input_accounts_for_spare_storage_and_analysis_uses_logical_pixels() {
        const INPUT_BYTES: usize = 1024 * 1024;
        for raw_len in [4, INPUT_BYTES] {
            let input = || {
                let mut raw = Vec::with_capacity(INPUT_BYTES);
                raw.resize(raw_len, 0);
                raw[..4].copy_from_slice(&[30, 60, 90, 255]);
                RgbaImage::from_raw(1, 1, raw).unwrap()
            };
            let mut limited = ScrollStitcher::with_limits(StitchConfig::default(), 1, 4096);
            assert!(matches!(
                limited.process_frame_detailed(input()),
                Err(CaptureError::WorkingSetExceeded { .. })
            ));
            assert!(limited.current_image().is_none());

            let mut fitting = ScrollStitcher::with_limits(StitchConfig::default(), 1, 2 * INPUT_BYTES as u64);
            fitting.process_frame_detailed(input()).unwrap();
            let analysis = fitting.accepted_analysis.as_ref().unwrap();
            assert_eq!(analysis.pixels.len(), 4);
            assert_eq!(fitting.get_final_image().unwrap().as_raw(), &[30, 60, 90, 255]);
        }
    }

    #[test]
    fn canvas_growth_accounts_for_old_and_new_storage_and_preserves_data_on_rejection() {
        let first = source(40, 30);
        let original_allocation = first.as_raw().as_ptr();
        let mut limited = ScrollStitcher::with_limits(StitchConfig::default(), 2400, 34_000);
        limited.try_process_frame_detailed(first, 0).unwrap();
        let (canvas, height) = limited.current_image().unwrap();
        assert_eq!(height, 30);
        assert_eq!(canvas.height(), 30);
        assert_eq!(canvas.as_raw().as_ptr(), original_allocation);
        let plan = StitchAppendPlan {
            trim_amount: 0,
            append_start_y: 0,
            append_end_y: 30,
        };
        let next = source(40, 30);
        let available = limited.admit_frame(40, 30, 40 * 30 * 4, 0).unwrap();
        assert!(matches!(
            limited.execute_stitch(&next, plan, available),
            Err(CaptureError::WorkingSetExceeded { .. })
        ));
        assert_eq!(limited.current_image().unwrap().1, 30);
        assert_eq!(limited.get_final_image().unwrap(), next);

        let mut fitting = ScrollStitcher::with_limits(StitchConfig::default(), 2400, 50_000);
        fitting.try_process_frame_detailed(next.clone(), 0).unwrap();
        let available = fitting.admit_frame(40, 30, 40 * 30 * 4, 0).unwrap();
        assert!(fitting.execute_stitch(&next, plan, available).unwrap());
        let (canvas, height) = fitting.current_image().unwrap();
        assert_eq!(height, 60);
        assert_eq!(canvas.dimensions(), (40, 60));
        let final_allocation = canvas.as_raw().as_ptr();
        assert!(matches!(
            fitting.execute_stitch(&next, plan, u64::MAX),
            Err(CaptureError::CapacityExceeded { max_pixels: 2400 })
        ));
        let output = fitting.into_final_image().unwrap();
        assert_eq!(output.dimensions(), (40, 60));
        assert_eq!(output.as_raw().as_ptr(), final_allocation);
    }
}

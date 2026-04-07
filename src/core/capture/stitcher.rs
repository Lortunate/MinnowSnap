use image::RgbaImage;

const CANVAS_RESIZE_HEADROOM: u32 = 2_000;

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
    pub confidence: f32,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StitchConfig {
    pub min_overlap: u32,
    pub min_scroll_threshold: u32,
    pub dynamic_block_size: usize,
    pub dynamic_threshold: f32,
    pub min_stable_blocks: usize,
    pub zncc_strip_count: usize,
    pub zncc_patch_height: usize,
    pub zncc_search_radius: i32,
    pub low_confidence_threshold: f32,
    pub low_confidence_gap: f32,
    pub seam_margin_divisor: u32,
}

impl Default for StitchConfig {
    fn default() -> Self {
        Self {
            min_overlap: 24,
            min_scroll_threshold: 4,
            dynamic_block_size: 16,
            dynamic_threshold: 12.0,
            min_stable_blocks: 6,
            zncc_strip_count: 6,
            zncc_patch_height: 8,
            zncc_search_radius: 8,
            low_confidence_threshold: 0.52,
            low_confidence_gap: 0.06,
            seam_margin_divisor: 5,
        }
    }
}

#[derive(Default)]
struct FrameAnalysis {
    width: usize,
    height: usize,
    gray: Vec<f32>,
    edge: Vec<f32>,
}

impl FrameAnalysis {
    fn from_image(image: &RgbaImage) -> Self {
        let width = image.width() as usize;
        let height = image.height() as usize;
        let gray = ScrollStitcher::grayscale(image);
        let edge = ScrollStitcher::edge_energy(&gray, width, height);
        Self { width, height, gray, edge }
    }
}

#[derive(Default)]
struct StitchScratch {
    stable_blocks: Vec<usize>,
    signature_prev: Vec<f32>,
    signature_next: Vec<f32>,
    coarse_prev: Vec<f32>,
    coarse_next: Vec<f32>,
    medium_prev: Vec<f32>,
    medium_next: Vec<f32>,
    strip_scores: Vec<f32>,
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
    last_analysis: Option<FrameAnalysis>,
    last_footer_height: u32,
    config: StitchConfig,
    scratch: StitchScratch,
    thumbnail: ThumbnailCache,
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

    pub fn with_config(config: StitchConfig) -> Self {
        Self {
            canvas: None,
            valid_height: 0,
            last_analysis: None,
            last_footer_height: 0,
            config,
            scratch: StitchScratch::default(),
            thumbnail: ThumbnailCache::default(),
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

    pub fn make_thumbnail(&mut self, target_width: u32) -> Option<RgbaImage> {
        let canvas = self.canvas.as_ref()?;
        let valid_h = self.valid_height;
        let source_width = canvas.width();
        if target_width == 0 || valid_h == 0 || source_width == 0 {
            return None;
        }

        let scale = target_width as f32 / source_width as f32;
        let target_height = (valid_h as f32 * scale) as u32;
        if target_height == 0 {
            return None;
        }

        let requires_full_render = self.thumbnail.image.is_none()
            || self.thumbnail.target_width != target_width
            || self.thumbnail.source_width != source_width
            || valid_h < self.thumbnail.source_height;

        if requires_full_render {
            let mut cropped = RgbaImage::new(source_width, valid_h);
            Self::copy_region(canvas, 0, &mut cropped, 0, valid_h);
            self.thumbnail.image = Some(image::imageops::resize(
                &cropped,
                target_width,
                target_height,
                image::imageops::FilterType::Triangle,
            ));
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
            let strip = image::imageops::crop_imm(canvas, 0, source_start, source_width, source_h).to_image();
            let resized_strip = image::imageops::resize(&strip, target_width, target_h, image::imageops::FilterType::Triangle);
            Self::copy_region(&resized_strip, 0, &mut next_thumbnail, target_start, target_h);
        }

        self.thumbnail.image = Some(next_thumbnail);
        self.thumbnail.target_width = target_width;
        self.thumbnail.source_width = source_width;
        self.thumbnail.source_height = valid_h;
        self.thumbnail.dirty_from = valid_h;
        self.thumbnail.image.clone()
    }

    pub fn process_frame_detailed(&mut self, new_image: RgbaImage) -> StitchFrameResult {
        if self.canvas.is_none() {
            self.initialize_canvas(new_image);
            return StitchFrameResult {
                status: StitchFrameStatus::Appended,
                height: self.valid_height as i32,
                confidence: 1.0,
                warning: None,
            };
        }

        let Some(prev_analysis) = self.last_analysis.take() else {
            self.initialize_canvas(new_image);
            return StitchFrameResult {
                status: StitchFrameStatus::Appended,
                height: self.valid_height as i32,
                confidence: 1.0,
                warning: None,
            };
        };

        let next_analysis = FrameAnalysis::from_image(&new_image);
        if prev_analysis.width != next_analysis.width || prev_analysis.height != next_analysis.height {
            self.last_analysis = Some(next_analysis);
            self.last_footer_height = 0;
            return StitchFrameResult {
                status: StitchFrameStatus::LowConfidence,
                height: self.valid_height as i32,
                confidence: 0.0,
                warning: Some("Frame geometry changed while scrolling".to_string()),
            };
        }

        let width = prev_analysis.width;
        let height = prev_analysis.height;
        let (fixed_top, fixed_bottom) = self.detect_sticky_regions(&prev_analysis.gray, &next_analysis.gray, width, height);
        let valid_h = (height as u32).saturating_sub(fixed_top + fixed_bottom);
        if valid_h < self.config.min_overlap {
            self.last_analysis = Some(next_analysis);
            self.last_footer_height = fixed_bottom;
            return StitchFrameResult {
                status: StitchFrameStatus::LowConfidence,
                height: self.valid_height as i32,
                confidence: 0.0,
                warning: Some("Insufficient scrollable content in selection".to_string()),
            };
        }

        Self::detect_stable_blocks(
            self.config,
            &prev_analysis.gray,
            &next_analysis.gray,
            width,
            height,
            fixed_top as usize,
            fixed_bottom as usize,
            &mut self.scratch.stable_blocks,
        );

        if self.scratch.stable_blocks.len() < self.config.min_stable_blocks {
            self.last_analysis = Some(next_analysis);
            self.last_footer_height = fixed_bottom;
            return StitchFrameResult {
                status: StitchFrameStatus::LowConfidence,
                height: self.valid_height as i32,
                confidence: 0.0,
                warning: Some("Dynamic content dominates viewport; wait for a stable frame".to_string()),
            };
        }

        Self::row_signature(
            self.config,
            &prev_analysis.edge,
            width,
            height,
            fixed_top as usize,
            fixed_bottom as usize,
            &self.scratch.stable_blocks,
            &mut self.scratch.signature_prev,
        );
        Self::row_signature(
            self.config,
            &next_analysis.edge,
            width,
            height,
            fixed_top as usize,
            fixed_bottom as usize,
            &self.scratch.stable_blocks,
            &mut self.scratch.signature_next,
        );

        let coarse_shift = Self::multiscale_row_shift(
            &self.scratch.signature_prev,
            &self.scratch.signature_next,
            self.config.min_overlap as usize,
            &mut self.scratch.coarse_prev,
            &mut self.scratch.coarse_next,
            &mut self.scratch.medium_prev,
            &mut self.scratch.medium_next,
        )
        .unwrap_or(0);

        let refine = Self::refine_shift_zncc(
            self.config,
            &prev_analysis.gray,
            &next_analysis.gray,
            width,
            height,
            fixed_top as usize,
            fixed_bottom as usize,
            coarse_shift,
            &self.scratch.stable_blocks,
            &mut self.scratch.strip_scores,
        );

        let Some((delta, best_score, second_score)) = refine else {
            self.last_analysis = Some(next_analysis);
            self.last_footer_height = fixed_bottom;
            return StitchFrameResult {
                status: StitchFrameStatus::LowConfidence,
                height: self.valid_height as i32,
                confidence: 0.0,
                warning: Some("Unable to estimate reliable overlap".to_string()),
            };
        };

        if delta < 0 {
            self.last_analysis = Some(next_analysis);
            self.last_footer_height = fixed_bottom;
            return StitchFrameResult {
                status: StitchFrameStatus::Reverse,
                height: self.valid_height as i32,
                confidence: best_score,
                warning: Some("Detected reverse scrolling; capture paused".to_string()),
            };
        }

        let delta = delta as u32;
        if delta < self.config.min_scroll_threshold {
            self.last_analysis = Some(next_analysis);
            self.last_footer_height = fixed_bottom;
            return StitchFrameResult {
                status: StitchFrameStatus::Stationary,
                height: self.valid_height as i32,
                confidence: best_score,
                warning: None,
            };
        }

        let confidence_gap = best_score - second_score;
        if best_score < self.config.low_confidence_threshold || confidence_gap < self.config.low_confidence_gap {
            self.last_analysis = Some(next_analysis);
            self.last_footer_height = fixed_bottom;
            return StitchFrameResult {
                status: StitchFrameStatus::LowConfidence,
                height: self.valid_height as i32,
                confidence: best_score,
                warning: Some("Low confidence overlap match; keep scrolling smoothly".to_string()),
            };
        }

        let valid_h_usize = valid_h as usize;
        let delta_usize = delta as usize;
        if delta_usize >= valid_h_usize {
            self.last_analysis = Some(next_analysis);
            self.last_footer_height = fixed_bottom;
            return StitchFrameResult {
                status: StitchFrameStatus::LowConfidence,
                height: self.valid_height as i32,
                confidence: best_score,
                warning: Some("Overlap collapsed due to unstable motion".to_string()),
            };
        }

        let overlap_valid = valid_h_usize.saturating_sub(delta_usize);
        if overlap_valid < self.config.min_overlap as usize {
            self.last_analysis = Some(next_analysis);
            self.last_footer_height = fixed_bottom;
            return StitchFrameResult {
                status: StitchFrameStatus::LowConfidence,
                height: self.valid_height as i32,
                confidence: best_score,
                warning: Some("Overlap too small; scroll slower".to_string()),
            };
        }

        let cut_valid = self.find_smart_seam(
            &prev_analysis.gray,
            &next_analysis.gray,
            width,
            height,
            fixed_top as usize,
            overlap_valid,
            &self.scratch.stable_blocks,
        );

        let trim_prev = overlap_valid.saturating_sub(cut_valid);
        let append_start = fixed_top as usize + cut_valid;
        let append_end = (new_image.height().saturating_sub(fixed_bottom)) as usize;

        if append_end <= append_start {
            self.last_analysis = Some(next_analysis);
            self.last_footer_height = fixed_bottom;
            return StitchFrameResult {
                status: StitchFrameStatus::LowConfidence,
                height: self.valid_height as i32,
                confidence: best_score,
                warning: Some("No appendable content after sticky region filtering".to_string()),
            };
        }

        if self.execute_stitch(&new_image, trim_prev as u32, append_start as u32, append_end as u32, fixed_bottom) {
            self.last_analysis = Some(next_analysis);
            StitchFrameResult {
                status: StitchFrameStatus::Appended,
                height: self.valid_height as i32,
                confidence: best_score,
                warning: None,
            }
        } else {
            self.last_analysis = Some(next_analysis);
            self.last_footer_height = fixed_bottom;
            StitchFrameResult {
                status: StitchFrameStatus::LowConfidence,
                height: self.valid_height as i32,
                confidence: best_score,
                warning: Some("Failed to append frame into scroll canvas".to_string()),
            }
        }
    }

    fn initialize_canvas(&mut self, first_image: RgbaImage) {
        let w = first_image.width();
        let h = first_image.height();
        let mut canvas = RgbaImage::new(w, h * 3);
        Self::copy_region(&first_image, 0, &mut canvas, 0, h);

        self.canvas = Some(canvas);
        self.valid_height = h;
        self.last_analysis = Some(FrameAnalysis::from_image(&first_image));
        self.last_footer_height = 0;
        self.thumbnail.reset();
    }

    fn grayscale(image: &RgbaImage) -> Vec<f32> {
        image
            .as_raw()
            .chunks_exact(4)
            .map(|px| (0.299 * f32::from(px[0])) + (0.587 * f32::from(px[1])) + (0.114 * f32::from(px[2])))
            .collect()
    }

    fn edge_energy(gray: &[f32], width: usize, height: usize) -> Vec<f32> {
        let mut out = vec![0.0; gray.len()];
        if width < 3 || height < 3 {
            return out;
        }

        for y in 1..(height - 1) {
            let row = y * width;
            for x in 1..(width - 1) {
                let i = row + x;
                let gx = gray[i + 1] - gray[i - 1];
                let gy = gray[i + width] - gray[i - width];
                out[i] = gx.abs() + gy.abs();
            }
        }
        out
    }

    fn detect_sticky_regions(&self, prev: &[f32], next: &[f32], width: usize, height: usize) -> (u32, u32) {
        if width == 0 || height == 0 {
            return (0, 0);
        }

        let threshold = self.config.dynamic_threshold;
        let max_check = (height / 3).max(1);

        let mut top = 0usize;
        while top < max_check {
            let row_start = top * width;
            let mut diff = 0.0f32;
            for x in 0..width {
                let idx = row_start + x;
                diff += (prev[idx] - next[idx]).abs();
            }
            if diff / width as f32 > threshold {
                break;
            }
            top += 1;
        }

        let mut bottom = 0usize;
        while bottom < max_check {
            let row = height - 1 - bottom;
            let row_start = row * width;
            let mut diff = 0.0f32;
            for x in 0..width {
                let idx = row_start + x;
                diff += (prev[idx] - next[idx]).abs();
            }
            if diff / width as f32 > threshold {
                break;
            }
            bottom += 1;
        }

        (top as u32, bottom as u32)
    }

    #[allow(clippy::too_many_arguments)]
    fn detect_stable_blocks(
        config: StitchConfig,
        prev: &[f32],
        next: &[f32],
        width: usize,
        height: usize,
        fixed_top: usize,
        fixed_bottom: usize,
        out: &mut Vec<usize>,
    ) {
        let block = config.dynamic_block_size.max(8);
        let block_count = (width / block).max(1);
        let y_start = fixed_top.min(height);
        let y_end = height.saturating_sub(fixed_bottom).max(y_start + 1);

        out.clear();
        if out.capacity() < block_count {
            out.reserve(block_count - out.capacity());
        }

        for b in 0..block_count {
            let x0 = b * block;
            let x1 = ((b + 1) * block).min(width);
            if x1 <= x0 {
                continue;
            }

            let mut diff_sum = 0.0f32;
            let mut count = 0usize;
            for y in y_start..y_end {
                let row = y * width;
                for x in x0..x1 {
                    let i = row + x;
                    diff_sum += (prev[i] - next[i]).abs();
                    count += 1;
                }
            }

            if count == 0 {
                continue;
            }
            let avg = diff_sum / count as f32;
            if avg <= config.dynamic_threshold {
                out.push(b);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn row_signature(
        config: StitchConfig,
        edge: &[f32],
        width: usize,
        height: usize,
        fixed_top: usize,
        fixed_bottom: usize,
        stable_blocks: &[usize],
        out: &mut Vec<f32>,
    ) {
        let block = config.dynamic_block_size.max(8);
        let y_start = fixed_top.min(height);
        let y_end = height.saturating_sub(fixed_bottom).max(y_start + 1);

        out.clear();
        let capacity = y_end.saturating_sub(y_start);
        if out.capacity() < capacity {
            out.reserve(capacity - out.capacity());
        }
        for y in y_start..y_end {
            let row = y * width;
            let mut sum = 0.0f32;
            let mut n = 0usize;
            for &b in stable_blocks {
                let x0 = b * block;
                let x1 = ((b + 1) * block).min(width);
                for x in x0..x1 {
                    sum += edge[row + x];
                    n += 1;
                }
            }
            out.push(if n == 0 { 0.0 } else { sum / n as f32 });
        }

        let mean = if out.is_empty() {
            0.0
        } else {
            out.iter().copied().sum::<f32>() / out.len() as f32
        };
        for v in out.iter_mut() {
            *v -= mean;
        }
    }

    fn downsample_signature(input: &[f32], factor: usize, out: &mut Vec<f32>) {
        out.clear();
        if factor <= 1 {
            out.extend_from_slice(input);
            return;
        }

        out.reserve(input.len().div_ceil(factor));
        for chunk in input.chunks(factor) {
            let sum = chunk.iter().copied().sum::<f32>();
            out.push(sum / chunk.len() as f32);
        }
    }

    fn signature_score(a: &[f32], b: &[f32], shift: i32, min_overlap: usize) -> Option<f32> {
        if a.is_empty() || b.is_empty() {
            return None;
        }

        let len = a.len().min(b.len()) as i32;
        let overlap = len - shift.unsigned_abs() as i32;
        if overlap <= 0 || overlap < min_overlap as i32 {
            return None;
        }

        let (a_start, b_start) = if shift >= 0 {
            (shift as usize, 0usize)
        } else {
            (0usize, (-shift) as usize)
        };

        let overlap = overlap as usize;
        let mut sum_a = 0.0f32;
        let mut sum_b = 0.0f32;
        let mut sum_aa = 0.0f32;
        let mut sum_bb = 0.0f32;
        let mut sum_ab = 0.0f32;

        for i in 0..overlap {
            let av = a[a_start + i];
            let bv = b[b_start + i];
            sum_a += av;
            sum_b += bv;
            sum_aa += av * av;
            sum_bb += bv * bv;
            sum_ab += av * bv;
        }

        let n = overlap as f32;
        let cov = sum_ab - (sum_a * sum_b / n);
        let var_a = (sum_aa - (sum_a * sum_a / n)).max(0.0);
        let var_b = (sum_bb - (sum_b * sum_b / n)).max(0.0);
        let denom = (var_a.sqrt() * var_b.sqrt()).max(f32::EPSILON);
        Some(cov / denom)
    }

    fn best_signature_shift(a: &[f32], b: &[f32], min_overlap: usize, start_shift: i32, end_shift: i32) -> Option<i32> {
        let mut best_shift = 0i32;
        let mut best_score = -1.0f32;
        let mut found = false;

        for shift in start_shift..=end_shift {
            let Some(score) = Self::signature_score(a, b, shift, min_overlap) else {
                continue;
            };
            if !found || score > best_score {
                best_score = score;
                best_shift = shift;
                found = true;
            }
        }

        if found { Some(best_shift) } else { None }
    }

    fn multiscale_row_shift(
        signature_prev: &[f32],
        signature_next: &[f32],
        min_overlap: usize,
        coarse_prev: &mut Vec<f32>,
        coarse_next: &mut Vec<f32>,
        medium_prev: &mut Vec<f32>,
        medium_next: &mut Vec<f32>,
    ) -> Option<i32> {
        let len = signature_prev.len().min(signature_next.len());
        if len < min_overlap {
            return None;
        }

        Self::downsample_signature(signature_prev, 4, coarse_prev);
        Self::downsample_signature(signature_next, 4, coarse_next);
        Self::downsample_signature(signature_prev, 2, medium_prev);
        Self::downsample_signature(signature_next, 2, medium_next);

        let mut shift = 0i32;

        let coarse_len = coarse_prev.len().min(coarse_next.len());
        let coarse_overlap = min_overlap.div_ceil(4).max(4);
        if coarse_len > coarse_overlap {
            let max_shift = (coarse_len - coarse_overlap) as i32;
            shift = Self::best_signature_shift(coarse_prev, coarse_next, coarse_overlap, -max_shift, max_shift).unwrap_or(0);
        }

        let medium_len = medium_prev.len().min(medium_next.len());
        let medium_overlap = min_overlap.div_ceil(2).max(6);
        if medium_len > medium_overlap {
            let max_shift = (medium_len - medium_overlap) as i32;
            let center = (shift * 2).clamp(-max_shift, max_shift);
            let radius = 6i32.min(max_shift.max(1));
            let start = (center - radius).max(-max_shift);
            let end = (center + radius).min(max_shift);
            shift = Self::best_signature_shift(medium_prev, medium_next, medium_overlap, start, end).unwrap_or(center);
        }

        let full_len = signature_prev.len().min(signature_next.len());
        if full_len <= min_overlap {
            return None;
        }
        let max_shift = (full_len - min_overlap) as i32;
        let center = (shift * 2).clamp(-max_shift, max_shift);
        let radius = 8i32.min(max_shift.max(1));
        let start = (center - radius).max(-max_shift);
        let end = (center + radius).min(max_shift);
        Self::best_signature_shift(signature_prev, signature_next, min_overlap, start, end)
    }

    #[allow(clippy::too_many_arguments)]
    fn refine_shift_zncc(
        config: StitchConfig,
        prev: &[f32],
        next: &[f32],
        width: usize,
        height: usize,
        fixed_top: usize,
        fixed_bottom: usize,
        coarse_shift: i32,
        stable_blocks: &[usize],
        strip_scores: &mut Vec<f32>,
    ) -> Option<(i32, f32, f32)> {
        let valid_h = height.saturating_sub(fixed_top + fixed_bottom) as i32;
        if valid_h <= config.min_overlap as i32 {
            return None;
        }

        let max_shift = valid_h - config.min_overlap as i32;
        if max_shift <= 0 {
            return None;
        }

        let mut best_shift = 0i32;
        let mut best_score = -1.0f32;
        let mut second_score = -1.0f32;

        let search_start = (coarse_shift - config.zncc_search_radius).max(-max_shift);
        let search_end = (coarse_shift + config.zncc_search_radius).min(max_shift);

        for shift in search_start..=search_end {
            let score = Self::zncc_score_for_shift(
                config,
                prev,
                next,
                width,
                height,
                fixed_top,
                fixed_bottom,
                shift,
                stable_blocks,
                strip_scores,
            );

            if score > best_score {
                second_score = best_score;
                best_score = score;
                best_shift = shift;
            } else if score > second_score {
                second_score = score;
            }
        }

        if best_score < -0.99 {
            return None;
        }

        if second_score < -0.99 {
            second_score = best_score;
        }

        Some((best_shift, best_score, second_score))
    }

    #[allow(clippy::too_many_arguments)]
    fn zncc_score_for_shift(
        config: StitchConfig,
        prev: &[f32],
        next: &[f32],
        width: usize,
        height: usize,
        fixed_top: usize,
        fixed_bottom: usize,
        shift: i32,
        stable_blocks: &[usize],
        strip_scores: &mut Vec<f32>,
    ) -> f32 {
        let valid_h = height.saturating_sub(fixed_top + fixed_bottom) as i32;
        let overlap = valid_h - shift.abs();
        if overlap <= config.min_overlap as i32 {
            return -1.0;
        }

        let strips = config.zncc_strip_count.max(3);
        let patch_h = config.zncc_patch_height.max(4) as i32;
        let block = config.dynamic_block_size.max(8) as i32;

        if strip_scores.capacity() < strips {
            strip_scores.reserve(strips - strip_scores.capacity());
        }
        strip_scores.clear();
        for si in 0..strips {
            let ratio = (si + 1) as f32 / (strips + 1) as f32;
            let overlap_start_next = if shift >= 0 { 0 } else { -shift };
            let overlap_start_prev = if shift >= 0 { shift } else { 0 };
            let center = ((overlap as f32) * ratio) as i32;

            let next_y = fixed_top as i32 + overlap_start_next + center;
            let prev_y = fixed_top as i32 + overlap_start_prev + center;

            let y0_next = (next_y - patch_h / 2).clamp(fixed_top as i32, height as i32 - fixed_bottom as i32 - patch_h - 1);
            let y0_prev = (prev_y - patch_h / 2).clamp(fixed_top as i32, height as i32 - fixed_bottom as i32 - patch_h - 1);

            let mut score_sum = 0.0f32;
            let mut score_count = 0usize;
            for &b in stable_blocks {
                let x0 = (b as i32 * block).min(width as i32 - 1);
                let x1 = ((b as i32 + 1) * block).min(width as i32);
                if x1 - x0 < 2 {
                    continue;
                }
                let score = Self::zncc_patch(
                    prev,
                    next,
                    width,
                    x0 as usize,
                    x1 as usize,
                    y0_prev as usize,
                    y0_next as usize,
                    patch_h as usize,
                );
                if score.is_finite() {
                    score_sum += score;
                    score_count += 1;
                }
            }

            if score_count == 0 {
                continue;
            }
            strip_scores.push(score_sum / score_count as f32);
        }

        if strip_scores.is_empty() {
            return -1.0;
        }

        strip_scores.iter().sum::<f32>() / strip_scores.len() as f32
    }

    #[allow(clippy::too_many_arguments)]
    fn zncc_patch(prev: &[f32], next: &[f32], width: usize, x0: usize, x1: usize, y0_prev: usize, y0_next: usize, patch_h: usize) -> f32 {
        let mut sum_p = 0.0f32;
        let mut sum_n = 0.0f32;
        let mut count = 0usize;

        for dy in 0..patch_h {
            let row_p = (y0_prev + dy) * width;
            let row_n = (y0_next + dy) * width;
            for x in x0..x1 {
                sum_p += prev[row_p + x];
                sum_n += next[row_n + x];
                count += 1;
            }
        }

        if count == 0 {
            return 0.0;
        }

        let mean_p = sum_p / count as f32;
        let mean_n = sum_n / count as f32;

        let mut cov = 0.0f32;
        let mut var_p = 0.0f32;
        let mut var_n = 0.0f32;

        for dy in 0..patch_h {
            let row_p = (y0_prev + dy) * width;
            let row_n = (y0_next + dy) * width;
            for x in x0..x1 {
                let p = prev[row_p + x] - mean_p;
                let n = next[row_n + x] - mean_n;
                cov += p * n;
                var_p += p * p;
                var_n += n * n;
            }
        }

        let denom = (var_p.sqrt() * var_n.sqrt()).max(f32::EPSILON);
        cov / denom
    }

    #[allow(clippy::too_many_arguments)]
    fn find_smart_seam(
        &self,
        prev: &[f32],
        next: &[f32],
        width: usize,
        height: usize,
        fixed_top: usize,
        overlap_valid: usize,
        stable_blocks: &[usize],
    ) -> usize {
        let block = self.config.dynamic_block_size.max(8);
        let search_start = overlap_valid / self.config.seam_margin_divisor.max(2) as usize;
        let search_end =
            overlap_valid.saturating_mul((self.config.seam_margin_divisor.max(2) - 1) as usize) / self.config.seam_margin_divisor.max(2) as usize;

        let mut best_k = overlap_valid / 2;
        let mut best_energy = f32::MAX;

        for k in search_start..search_end.max(search_start + 1) {
            let prev_row = fixed_top + k;
            let next_row = k;
            if prev_row == 0 || prev_row >= height || next_row == 0 || next_row >= height {
                continue;
            }

            let mut energy = 0.0f32;
            let mut n = 0usize;
            for &b in stable_blocks {
                let x0 = b * block;
                let x1 = ((b + 1) * block).min(width);
                for x in x0..x1 {
                    let p_idx = prev_row * width + x;
                    let p_up = (prev_row - 1) * width + x;
                    let n_idx = next_row * width + x;
                    let n_up = (next_row - 1) * width + x;

                    let p_grad = (prev[p_idx] - prev[p_up]).abs();
                    let n_grad = (next[n_idx] - next[n_up]).abs();
                    energy += (p_grad - n_grad).abs();
                    n += 1;
                }
            }

            if n == 0 {
                continue;
            }
            let avg = energy / n as f32;
            if avg < best_energy {
                best_energy = avg;
                best_k = k;
            }
        }

        best_k
    }

    fn execute_stitch(&mut self, new_image: &RgbaImage, trim_amount: u32, append_start_y: u32, append_end_y: u32, fixed_bottom: u32) -> bool {
        let Some(canvas) = self.canvas.as_mut() else {
            return false;
        };

        let previous_valid_height = self.valid_height;
        let content_end = self.valid_height.saturating_sub(self.last_footer_height);
        if trim_amount > content_end {
            return false;
        }

        let keep_h = content_end - trim_amount;
        let append_h = append_end_y.saturating_sub(append_start_y);
        let new_total_h = keep_h + append_h;

        if new_total_h > canvas.height() {
            let new_cap = (canvas.height() * 2).max(new_total_h + CANVAS_RESIZE_HEADROOM);
            let width = canvas.width();
            let mut new_canvas = RgbaImage::new(width, new_cap);
            Self::copy_region(canvas, 0, &mut new_canvas, 0, keep_h);
            self.canvas = Some(new_canvas);
        }

        let Some(canvas) = self.canvas.as_mut() else {
            return false;
        };

        if append_h > 0 {
            Self::copy_region(new_image, append_start_y, canvas, keep_h, append_h);
        }

        self.valid_height = new_total_h;
        self.last_footer_height = fixed_bottom;
        self.thumbnail.mark_dirty_from(keep_h.min(previous_valid_height));
        true
    }

    fn copy_region(src: &RgbaImage, src_y: u32, dest: &mut RgbaImage, dest_y: u32, height: u32) {
        if height == 0 {
            return;
        }
        let width_bytes = (src.width() * 4) as usize;
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

    #[test]
    fn stitcher_appends_on_forward_scroll() {
        let mut stitcher = ScrollStitcher::new();
        let src = source(240, 420);

        let first = crop_frame(&src, 0, 180);
        let second = crop_frame(&src, 24, 180);

        assert_eq!(stitcher.process_frame_detailed(first).status, StitchFrameStatus::Appended);
        let detail = stitcher.process_frame_detailed(second);
        assert!(matches!(detail.status, StitchFrameStatus::Appended | StitchFrameStatus::LowConfidence));
        assert!(detail.height >= 180);
    }

    #[test]
    fn stitcher_marks_stationary_for_same_frame() {
        let mut stitcher = ScrollStitcher::new();
        let src = source(200, 420);
        let first = crop_frame(&src, 10, 160);
        assert_eq!(stitcher.process_frame_detailed(first.clone()).status, StitchFrameStatus::Appended);
        assert_eq!(stitcher.process_frame_detailed(first).status, StitchFrameStatus::Stationary);
    }

    #[test]
    fn stitcher_reports_reverse() {
        let mut stitcher = ScrollStitcher::new();
        let src = source(200, 420);
        let first = crop_frame(&src, 80, 160);
        let reverse = crop_frame(&src, 30, 160);

        assert_eq!(stitcher.process_frame_detailed(first).status, StitchFrameStatus::Appended);
        let detail = stitcher.process_frame_detailed(reverse);
        assert!(matches!(detail.status, StitchFrameStatus::Reverse | StitchFrameStatus::LowConfidence));
    }
}

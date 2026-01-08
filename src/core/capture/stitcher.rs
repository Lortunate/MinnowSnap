use image::RgbaImage;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum StitchResult {
    Success,
    Stationary,
    Failure,
}

pub struct StitchConfig {
    pub min_overlap: u32,
    pub pixel_tolerance: u8,
    pub min_scroll_threshold: u32,
}

impl Default for StitchConfig {
    fn default() -> Self {
        Self {
            min_overlap: 50,
            pixel_tolerance: 10,
            min_scroll_threshold: 15,
        }
    }
}

pub struct ScrollStitcher {
    canvas: Option<RgbaImage>,
    valid_height: u32,
    last_frame: Option<RgbaImage>,
    last_footer_height: u32,
    config: StitchConfig,
}

impl Default for ScrollStitcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollStitcher {
    pub fn new() -> Self {
        Self {
            canvas: None,
            valid_height: 0,
            last_frame: None,
            last_footer_height: 0,
            config: StitchConfig::default(),
        }
    }

    pub fn with_config(config: StitchConfig) -> Self {
        Self {
            canvas: None,
            valid_height: 0,
            last_frame: None,
            last_footer_height: 0,
            config,
        }
    }

    #[must_use]
    pub fn current_image(&self) -> Option<(&RgbaImage, u32)> {
        self.canvas.as_ref().map(|img| (img, self.valid_height))
    }

    #[must_use]
    pub fn get_final_image(&self) -> Option<RgbaImage> {
        let (canvas, h) = self.current_image()?;
        let w = canvas.width();
        let mut final_img = RgbaImage::new(w, h);
        Self::copy_region(canvas, 0, &mut final_img, 0, h);
        Some(final_img)
    }

    #[must_use]
    pub fn make_thumbnail(&self, target_width: u32) -> Option<RgbaImage> {
        let (canvas, valid_h) = self.current_image()?;
        let w = canvas.width();

        if valid_h == 0 || w == 0 {
            return None;
        }

        let scale = target_width as f32 / w as f32;
        let target_height = (valid_h as f32 * scale) as u32;

        if target_height == 0 {
            return None;
        }

        let view = image::imageops::crop_imm(canvas, 0, 0, w, valid_h).to_image();
        Some(image::imageops::resize(
            &view,
            target_width,
            target_height,
            image::imageops::FilterType::Triangle,
        ))
    }

    pub fn process_frame(&mut self, new_image: RgbaImage) -> StitchResult {
        if self.canvas.is_none() {
            let w = new_image.width();
            let h = new_image.height();
            let capacity = h * 2;
            let mut canvas = RgbaImage::new(w, capacity);

            Self::copy_region(&new_image, 0, &mut canvas, 0, h);

            self.canvas = Some(canvas);
            self.valid_height = h;
            self.last_frame = Some(new_image);
            self.last_footer_height = 0;
            return StitchResult::Success;
        }

        let last_frame = self.last_frame.as_ref().unwrap();

        if last_frame.width() != new_image.width() {
            return StitchResult::Failure;
        }

        let (fixed_top, fixed_bottom) = self.detect_fixed_regions(last_frame, &new_image);

        let prev_h = last_frame.height();
        let next_h = new_image.height();

        if prev_h <= fixed_top + fixed_bottom || next_h <= fixed_top + fixed_bottom {
            return StitchResult::Failure;
        }

        let valid_prev_h = prev_h - fixed_top - fixed_bottom;
        let valid_next_h = next_h - fixed_top - fixed_bottom;

        let prev_sig = self.compute_row_signatures(last_frame, fixed_top, valid_prev_h);
        let next_sig = self.compute_row_signatures(&new_image, fixed_top, valid_next_h);

        let best_overlap = self.find_best_overlap(&prev_sig, &next_sig);

        if best_overlap == 0 {
            return StitchResult::Failure;
        }

        let check_offset = best_overlap / 2;
        let verify_y_prev = prev_h - fixed_bottom - best_overlap + check_offset;
        let verify_y_next = fixed_top + check_offset;
        let stride = (last_frame.width() * 4) as usize;

        if !self.rows_match_sampled(last_frame.as_raw(), new_image.as_raw(), stride, verify_y_prev, verify_y_next) {
            return StitchResult::Failure;
        }

        let scroll_delta = valid_prev_h.saturating_sub(best_overlap);

        if scroll_delta < self.config.min_scroll_threshold {
            self.last_frame = Some(new_image);
            return StitchResult::Stationary;
        }

        let prev_seam_start_y = prev_h - fixed_bottom - best_overlap;
        let next_seam_start_y = fixed_top;

        let best_seam_k = self.find_best_seam(last_frame, &new_image, prev_seam_start_y, next_seam_start_y, best_overlap);

        let trim_amount = best_overlap.saturating_sub(best_seam_k);
        let next_content_start_y = next_seam_start_y + best_seam_k;

        if self.execute_stitch(new_image, trim_amount, next_content_start_y, fixed_bottom) {
            StitchResult::Success
        } else {
            StitchResult::Failure
        }
    }

    fn execute_stitch(&mut self, new_image: RgbaImage, trim_amount: u32, new_content_start_y: u32, fixed_bottom: u32) -> bool {
        let canvas = self.canvas.as_mut().unwrap();
        let width = canvas.width();

        let content_end_y = self.valid_height.saturating_sub(self.last_footer_height);

        if trim_amount > content_end_y {
            return false;
        }

        let keep_h = content_end_y - trim_amount;
        let new_content_h = new_image.height().saturating_sub(new_content_start_y);
        let new_total_h = keep_h + new_content_h;

        if new_total_h > canvas.height() {
            let new_cap = (canvas.height() * 2).max(new_total_h);
            let mut new_canvas = RgbaImage::new(width, new_cap);

            Self::copy_region(canvas, 0, &mut new_canvas, 0, keep_h);

            self.canvas = Some(new_canvas);
        }

        let canvas = self.canvas.as_mut().unwrap();

        Self::copy_region(&new_image, new_content_start_y, canvas, keep_h, new_content_h);

        self.valid_height = new_total_h;
        self.last_frame = Some(new_image);
        self.last_footer_height = fixed_bottom;

        true
    }

    fn copy_region(src: &RgbaImage, src_y: u32, dest: &mut RgbaImage, dest_y: u32, height: u32) {
        if height == 0 {
            return;
        }

        let width_bytes = (src.width() * 4) as usize;
        let copy_bytes = (height as usize) * width_bytes;

        let src_offset = (src_y as usize) * width_bytes;
        let dest_offset = (dest_y as usize) * width_bytes;

        let src_raw = src.as_raw();
        let dest_raw = dest.as_mut();

        if src_offset + copy_bytes <= src_raw.len() && dest_offset + copy_bytes <= dest_raw.len() {
            dest_raw[dest_offset..dest_offset + copy_bytes].copy_from_slice(&src_raw[src_offset..src_offset + copy_bytes]);
        }
    }

    fn detect_fixed_regions(&self, prev: &RgbaImage, next: &RgbaImage) -> (u32, u32) {
        let w = prev.width();
        let h = prev.height();
        let max_check = h / 3;

        let raw_prev = prev.as_raw();
        let raw_next = next.as_raw();
        let stride = (w * 4) as usize;

        let mut fixed_top = 0;
        for y in 0..max_check {
            if !self.rows_match_sampled(raw_prev, raw_next, stride, y, y) {
                break;
            }
            fixed_top += 1;
        }

        let mut fixed_bottom = 0;
        for y in 0..max_check {
            let y_idx = h - 1 - y;
            if !self.rows_match_sampled(raw_prev, raw_next, stride, y_idx, y_idx) {
                break;
            }
            fixed_bottom += 1;
        }

        (fixed_top, fixed_bottom)
    }

    #[inline]
    fn rows_match_sampled(&self, raw1: &[u8], raw2: &[u8], stride: usize, y1: u32, y2: u32) -> bool {
        let start1 = (y1 as usize) * stride;
        let start2 = (y2 as usize) * stride;

        if start1 + stride > raw1.len() || start2 + stride > raw2.len() {
            return false;
        }

        let r1 = &raw1[start1..start1 + stride];
        let r2 = &raw2[start2..start2 + stride];

        let mut total_diff: u64 = 0;
        let mut count = 0;

        // Sample every 4th pixel (16 bytes)
        for (c1, c2) in r1.chunks_exact(16).zip(r2.chunks_exact(16)) {
            total_diff += (c1[0].abs_diff(c2[0]) as u64) + (c1[1].abs_diff(c2[1]) as u64) + (c1[2].abs_diff(c2[2]) as u64);
            count += 1;
        }

        if count == 0 {
            return true;
        }
        (total_diff / count) <= (self.config.pixel_tolerance as u64)
    }

    fn compute_row_signatures(&self, img: &RgbaImage, start_y: u32, height: u32) -> Vec<u64> {
        let w = img.width();
        let stride = (w * 4) as usize;
        let raw = img.as_raw();
        let mut sigs = Vec::with_capacity(height as usize);

        for i in 0..height {
            let y = start_y + i;
            let row_start = (y as usize) * stride;

            if row_start + stride > raw.len() {
                sigs.push(0);
                continue;
            }

            let row = &raw[row_start..row_start + stride];
            let mut sum = 0u64;

            // Sample every 4th pixel for performance (consistent with detection)
            // This speeds up signature calculation by 4x without losing much coarse signal
            for pixel in row.chunks_exact(16) {
                sum += (pixel[0] as u64) + (pixel[1] as u64) + (pixel[2] as u64);
            }
            sigs.push(sum);
        }
        sigs
    }

    fn find_best_overlap(&self, prev_sig: &[u64], next_sig: &[u64]) -> u32 {
        let len_prev = prev_sig.len();
        let len_next = next_sig.len();
        let max_overlap = len_prev.min(len_next);
        let min_overlap = self.config.min_overlap as usize;

        if max_overlap < min_overlap {
            return 0;
        }

        let mut best_overlap = 0;
        let mut min_avg_diff = u64::MAX;

        // Slide check: BOTTOM of prev matches TOP of next
        for overlap in min_overlap..=max_overlap {
            let s1_part = &prev_sig[len_prev - overlap..];
            let s2_part = &next_sig[..overlap];

            let diff_sum: u64 = s1_part.iter().zip(s2_part.iter()).map(|(a, b)| a.abs_diff(*b)).sum();

            let avg_diff = diff_sum / (overlap as u64);

            if avg_diff < min_avg_diff {
                min_avg_diff = avg_diff;
                best_overlap = overlap;
            }
        }

        best_overlap as u32
    }

    fn find_best_seam(&self, prev: &RgbaImage, next: &RgbaImage, prev_y_start: u32, next_y_start: u32, height: u32) -> u32 {
        let w = prev.width();
        let stride = (w * 4) as usize;
        let raw_prev = prev.as_raw();
        let raw_next = next.as_raw();

        let mut best_k = 0;
        let mut min_row_diff = u64::MAX;

        for k in 0..height {
            let idx_prev = ((prev_y_start + k) as usize) * stride;
            let idx_next = ((next_y_start + k) as usize) * stride;

            if idx_prev + stride > raw_prev.len() || idx_next + stride > raw_next.len() {
                continue;
            }

            let r1 = &raw_prev[idx_prev..idx_prev + stride];
            let r2 = &raw_next[idx_next..idx_next + stride];

            let mut diff: u64 = 0;
            // Full pixel difference for precision in seam finding
            for (p1, p2) in r1.chunks_exact(4).zip(r2.chunks_exact(4)) {
                diff += (p1[0].abs_diff(p2[0]) as u64) + (p1[1].abs_diff(p2[1]) as u64) + (p1[2].abs_diff(p2[2]) as u64);
            }

            if diff < min_row_diff {
                min_row_diff = diff;
                best_k = k;
            }
        }

        best_k
    }
}

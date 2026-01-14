use image::RgbaImage;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum StitchResult {
    Success,
    Stationary,
    Failure,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StitchConfig {
    pub min_overlap: u32,
    pub min_scroll_threshold: u32,
    pub overlap_avg_threshold: u64,
    pub motion_scan_step: usize,
    pub motion_threshold_divisor: u64,
    pub fixed_diff_percent: u64,
    pub verify_pixel_diff: u32,
    pub verify_step_divisor: usize,
    pub seam_margin_divisor: u32,
}

impl Default for StitchConfig {
    fn default() -> Self {
        Self {
            min_overlap: 20,
            min_scroll_threshold: 5,
            overlap_avg_threshold: 500,
            motion_scan_step: 8,
            motion_threshold_divisor: 2,
            fixed_diff_percent: 5,
            verify_pixel_diff: 80,
            verify_step_divisor: 20,
            seam_margin_divisor: 4,
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
        Self::with_config(StitchConfig::default())
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

        let mut cropped = RgbaImage::new(w, valid_h);
        Self::copy_region(canvas, 0, &mut cropped, 0, valid_h);

        Some(image::imageops::resize(
            &cropped,
            target_width,
            target_height,
            image::imageops::FilterType::Triangle,
        ))
    }

    pub fn process_frame(&mut self, new_image: RgbaImage) -> StitchResult {
        if self.canvas.is_none() {
            self.initialize_canvas(new_image);
            return StitchResult::Success;
        }

        let last_frame = self.last_frame.as_ref().unwrap();
        if last_frame.width() != new_image.width() {
            return StitchResult::Failure;
        }

        let h_prev = last_frame.height();
        let h_next = new_image.height();

        let active_mask = self.compute_motion_mask(last_frame, &new_image);
        if active_mask.is_empty() {
            self.last_frame = Some(new_image);
            return StitchResult::Stationary;
        }

        let sig_prev = self.compute_masked_signatures(last_frame, &active_mask);
        let sig_next = self.compute_masked_signatures(&new_image, &active_mask);

        let (fixed_top, fixed_bottom) = self.detect_fixed_regions_sig(&sig_prev, &sig_next);

        let valid_prev = h_prev.saturating_sub(fixed_top + fixed_bottom);
        let valid_next = h_next.saturating_sub(fixed_top + fixed_bottom);

        if valid_prev < self.config.min_overlap || valid_next < self.config.min_overlap {
            return StitchResult::Failure;
        }

        let (best_overlap, is_reverse) = self.find_optimal_overlap(&sig_prev, &sig_next, fixed_top, fixed_bottom, valid_prev, valid_next);

        if best_overlap == 0 {
            return StitchResult::Failure;
        }

        if !self.verify_pixels(last_frame, &new_image, best_overlap, is_reverse, fixed_top, fixed_bottom, &active_mask) {
            return StitchResult::Failure;
        }

        let scroll_delta = valid_prev.saturating_sub(best_overlap);
        if scroll_delta < self.config.min_scroll_threshold {
            self.last_frame = Some(new_image);
            return StitchResult::Stationary;
        }

        if is_reverse {
            self.valid_height = self.valid_height.saturating_sub(scroll_delta);
            self.last_frame = Some(new_image);
            self.last_footer_height = fixed_bottom;
            return StitchResult::Success;
        }

        let cut_y = self.find_smart_seam(last_frame, &new_image, best_overlap, fixed_top, &active_mask);

        let trim_amount = best_overlap.saturating_sub(cut_y);
        let next_start = fixed_top + cut_y;

        if self.execute_stitch(new_image, trim_amount, next_start, fixed_bottom) {
            StitchResult::Success
        } else {
            StitchResult::Failure
        }
    }

    fn initialize_canvas(&mut self, first_image: RgbaImage) {
        let w = first_image.width();
        let h = first_image.height();
        let mut canvas = RgbaImage::new(w, h * 3);
        Self::copy_region(&first_image, 0, &mut canvas, 0, h);

        self.canvas = Some(canvas);
        self.valid_height = h;
        self.last_frame = Some(first_image);
        self.last_footer_height = 0;
    }

    fn compute_motion_mask(&self, prev: &RgbaImage, next: &RgbaImage) -> Vec<usize> {
        let w = prev.width();
        let h = prev.height();
        let raw_prev = prev.as_raw();
        let raw_next = next.as_raw();
        let stride = (w * 4) as usize;
        let step = self.config.motion_scan_step;

        (0..w)
            .step_by(step)
            .map(|x| x as usize)
            .filter(|&x| {
                let mut diff_sum: u64 = 0;
                for y in (0..h).step_by(step) {
                    let idx = (y as usize) * stride + (x * 4);
                    diff_sum += Self::pixel_diff(raw_prev, idx, raw_next, idx) as u64;
                }
                diff_sum > (h as u64 / self.config.motion_threshold_divisor)
            })
            .collect()
    }

    fn compute_masked_signatures(&self, img: &RgbaImage, cols: &[usize]) -> Vec<u64> {
        let w = img.width();
        let h = img.height();
        let raw = img.as_raw();
        let stride = (w * 4) as usize;

        (0..h)
            .map(|y| {
                let row_start = (y as usize) * stride;
                cols.iter().fold(0u64, |sum, &x| {
                    let idx = row_start + (x * 4);
                    sum + Self::pixel_sum(raw, idx)
                })
            })
            .collect()
    }

    fn measure_fixed_len(&self, s1: &[u64], s2: &[u64], indices: impl Iterator<Item = usize>) -> u32 {
        let mut len = 0;
        for i in indices {
            let diff = s1[i].abs_diff(s2[i]);
            let max_val = s1[i].max(s2[i]);
            if max_val > 0 && (diff * 100 / max_val) > self.config.fixed_diff_percent {
                break;
            }
            len += 1;
        }
        len
    }

    fn detect_fixed_regions_sig(&self, s_prev: &[u64], s_next: &[u64]) -> (u32, u32) {
        let h = s_prev.len();
        let max_check = h / 3;

        let top = self.measure_fixed_len(s_prev, s_next, 0..max_check);
        let bottom = self.measure_fixed_len(s_prev, s_next, (0..max_check).map(|i| h - 1 - i));

        (top, bottom)
    }

    fn scan_overlaps<F>(&self, range: impl Iterator<Item = u32>, calc_offsets: F, s1: &[u64], s2: &[u64]) -> (u32, u64)
    where
        F: Fn(u32) -> (usize, usize),
    {
        let mut best_ov = 0;
        let mut best_score = u64::MAX;

        for overlap in range {
            let (st1, st2) = calc_offsets(overlap);
            let score = Self::score_overlap(s1, s2, st1, st2, overlap as usize);
            let avg = score / (overlap as u64);

            if avg < self.config.overlap_avg_threshold {
                return (overlap, avg);
            }
            if avg < best_score {
                best_score = avg;
                best_ov = overlap;
            }
        }
        (best_ov, best_score)
    }

    fn find_optimal_overlap(&self, s_prev: &[u64], s_next: &[u64], f_top: u32, f_btm: u32, v_prev: u32, v_next: u32) -> (u32, bool) {
        let max_overlap = v_prev.min(v_next);
        if max_overlap < self.config.min_overlap {
            return (0, false);
        }

        let range = (self.config.min_overlap..=max_overlap).rev();

        let (f_ov, f_score) = self.scan_overlaps(
            range.clone(),
            |ov| {
                let prev_start = (s_prev.len() as u32 - f_btm - ov) as usize;
                let next_start = f_top as usize;
                (prev_start, next_start)
            },
            s_prev,
            s_next,
        );

        let (r_ov, r_score) = self.scan_overlaps(
            range,
            |ov| {
                let prev_start = f_top as usize;
                let next_start = (s_next.len() as u32 - f_btm - ov) as usize;
                (prev_start, next_start)
            },
            s_prev,
            s_next,
        );

        if r_score < f_score / 2 { (r_ov, true) } else { (f_ov, false) }
    }

    #[inline]
    fn score_overlap(s1: &[u64], s2: &[u64], start1: usize, start2: usize, len: usize) -> u64 {
        (0..len).map(|i| s1[start1 + i].abs_diff(s2[start2 + i])).sum()
    }

    fn verify_pixels(&self, prev: &RgbaImage, next: &RgbaImage, overlap: u32, reverse: bool, f_top: u32, f_btm: u32, cols: &[usize]) -> bool {
        let raw_prev = prev.as_raw();
        let raw_next = next.as_raw();
        let stride = (prev.width() * 4) as usize;

        let (y1_base, y2_base) = if reverse {
            (f_top, next.height() - f_btm - overlap)
        } else {
            (prev.height() - f_btm - overlap, f_top)
        };

        [0, overlap / 2, overlap - 1].iter().all(|&r| {
            let y1 = y1_base + r;
            let y2 = y2_base + r;
            let step = (cols.len() / self.config.verify_step_divisor).max(1);

            let (hits, checks) = cols.iter().step_by(step).fold((0, 0), |(h, c), &x| {
                let idx1 = (y1 as usize) * stride + (x * 4);
                let idx2 = (y2 as usize) * stride + (x * 4);
                let d = Self::pixel_diff(raw_prev, idx1, raw_next, idx2);
                (if d < self.config.verify_pixel_diff { h + 1 } else { h }, c + 1)
            });

            checks == 0 || hits >= (checks / 2)
        })
    }

    fn find_smart_seam(&self, prev: &RgbaImage, next: &RgbaImage, overlap: u32, f_top: u32, cols: &[usize]) -> u32 {
        let w = prev.width();
        let stride = (w * 4) as usize;
        let raw = next.as_raw();
        let start_y = f_top;

        let search_start = if overlap > self.config.min_overlap {
            overlap / self.config.seam_margin_divisor
        } else {
            0
        };
        let search_end = if overlap > self.config.min_overlap {
            overlap * (self.config.seam_margin_divisor - 1) / self.config.seam_margin_divisor
        } else {
            overlap
        };
        let step = (cols.len() / self.config.verify_step_divisor).max(1);

        (search_start..search_end)
            .min_by_key(|&k| {
                let y = start_y + k;
                let idx = (y as usize) * stride;
                cols.iter()
                    .step_by(step)
                    .map(|&x| {
                        let p = idx + (x * 4);
                        if p >= stride {
                            Self::pixel_diff(raw, p, raw, p - stride) as u64
                        } else {
                            0
                        }
                    })
                    .sum::<u64>()
            })
            .unwrap_or(overlap / 2)
    }

    fn execute_stitch(&mut self, new_image: RgbaImage, trim_amount: u32, new_content_start_y: u32, fixed_bottom: u32) -> bool {
        let canvas = self.canvas.as_mut().unwrap();
        let content_end_y = self.valid_height.saturating_sub(self.last_footer_height);

        if trim_amount > content_end_y {
            return false;
        }

        let keep_h = content_end_y - trim_amount;
        let new_content_h = new_image.height().saturating_sub(new_content_start_y);
        let new_total_h = keep_h + new_content_h;

        if new_total_h > canvas.height() {
            let new_cap = (canvas.height() * 2).max(new_total_h + 2000);
            let width = canvas.width();
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

    #[inline(always)]
    fn pixel_diff(raw1: &[u8], idx1: usize, raw2: &[u8], idx2: usize) -> u32 {
        (raw1[idx1].abs_diff(raw2[idx2]) as u32) + (raw1[idx1 + 1].abs_diff(raw2[idx2 + 1]) as u32) + (raw1[idx1 + 2].abs_diff(raw2[idx2 + 2]) as u32)
    }

    #[inline(always)]
    fn pixel_sum(raw: &[u8], idx: usize) -> u64 {
        (raw[idx] as u64) + (raw[idx + 1] as u64) + (raw[idx + 2] as u64)
    }
}

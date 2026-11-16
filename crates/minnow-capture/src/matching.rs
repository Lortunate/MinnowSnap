//! Pure overlap decisions. This module never mutates a canvas or advances an anchor.
use image::RgbaImage;
use std::hash::{DefaultHasher, Hasher};

use crate::{CaptureError, reserve};

const BINS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StitchConfig {
    pub min_overlap: u32,
    pub min_informative_rows: u32,
    pub pixel_tolerance: u8,
    pub min_matching_rows: f32,
    pub uniqueness_margin: f32,
}

impl Default for StitchConfig {
    fn default() -> Self {
        Self {
            min_overlap: 24,
            min_informative_rows: 12,
            pixel_tolerance: 8,
            min_matching_rows: 0.90,
            uniqueness_margin: 0.08,
        }
    }
}

impl StitchConfig {
    pub(crate) fn validate(self) -> Result<(), CaptureError> {
        for (valid, field) in [
            ((1..=i32::MAX as u32).contains(&self.min_overlap), "min_overlap"),
            ((1..=i32::MAX as u32).contains(&self.min_informative_rows), "min_informative_rows"),
            (self.pixel_tolerance <= 32, "pixel_tolerance"),
            ((0.8..=1.0).contains(&self.min_matching_rows), "min_matching_rows"),
            ((0.01..=1.0).contains(&self.uniqueness_margin), "uniqueness_margin"),
        ] {
            if !valid {
                return Err(CaptureError::InvalidStitchConfig { field });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StitchRejection {
    #[error("frame dimensions changed")]
    DimensionsChanged,
    #[error("not enough overlapping content")]
    InsufficientOverlap,
    #[error("not enough distinctive content")]
    InsufficientDetail,
    #[error("several scrolling distances match the content")]
    Ambiguous,
    #[error("content changed or scrolling skipped the overlap")]
    ContentChanged,
    #[error("scrolling moved behind the last accepted frame")]
    Reverse,
}

/// Scores are fractions of informative rows agreeing across the whole overlap.
/// The competitor is searched across both directions, including zero motion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MatchEvidence {
    pub displacement: i32,
    pub overlap: u32,
    pub matching_rows: u32,
    pub informative_rows: u32,
    pub score: f32,
    pub competitor_score: f32,
    pub competitor_displacement: Option<i32>,
    pub fixed_top: u32,
    pub fixed_bottom: u32,
    pub next_seam: Option<u32>,
    pub output_seam: Option<u32>,
}

pub(crate) struct Decision {
    pub reason: Option<StitchRejection>,
    pub evidence: Option<MatchEvidence>,
}

struct Row {
    fingerprint: u64,
    means: [f32; BINS * 3],
    informative: bool,
}

pub(crate) struct FrameAnalysis {
    width: usize,
    height: usize,
    pub(crate) pixels: Vec<u8>,
    rows: Vec<Row>,
}

impl FrameAnalysis {
    pub(crate) fn required_bytes(width: u32, height: u32) -> u64 {
        u64::from(width) * u64::from(height) * 4 + u64::from(height) * std::mem::size_of::<Row>() as u64
    }

    pub(crate) fn allocated_bytes(&self) -> u64 {
        self.pixels.capacity() as u64 + (self.rows.capacity() * std::mem::size_of::<Row>()) as u64
    }

    pub(crate) fn from_image(image: &RgbaImage) -> Result<Self, CaptureError> {
        let width = image.width() as usize;
        let height = image.height() as usize;
        let mut pixels = Vec::new();
        reserve(&mut pixels, width * height * 4)?;
        pixels.extend_from_slice(&image.as_raw()[..width * height * 4]);
        let mut rows = Vec::new();
        reserve(&mut rows, height)?;
        for row in pixels.chunks_exact(width * 4) {
            let mut means = [0.0; BINS * 3];
            let mut hasher = DefaultHasher::new();
            hasher.write(row);
            let fingerprint = hasher.finish();
            let mut low = [255u8; 3];
            let mut high = [0u8; 3];
            for bin in 0..BINS {
                let start = (bin * width).div_ceil(BINS);
                let end = ((bin + 1) * width).div_ceil(BINS);
                let mut sums = [0u64; 3];
                for pixel in row[start * 4..end * 4].as_chunks::<4>().0 {
                    for c in 0..3 {
                        sums[c] += u64::from(pixel[c]);
                        low[c] = low[c].min(pixel[c]);
                        high[c] = high[c].max(pixel[c]);
                    }
                }
                for c in 0..3 {
                    means[bin * 3 + c] = sums[c] as f32 / (end - start).max(1) as f32;
                }
            }
            rows.push(Row {
                fingerprint,
                means,
                informative: (0..3).any(|c| high[c] - low[c] > 16),
            });
        }
        Ok(Self { width, height, pixels, rows })
    }

    pub(crate) fn is_identical(&self, image: &RgbaImage) -> bool {
        self.width == image.width() as usize && self.height == image.height() as usize && self.pixels == *image.as_raw()
    }

    fn row(&self, y: usize) -> &[u8] {
        &self.pixels[y * self.width * 4..(y + 1) * self.width * 4]
    }
}

fn row_agrees(a: &Row, b: &Row, tolerance: u8) -> bool {
    a.fingerprint == b.fingerprint || a.means.iter().zip(&b.means).all(|(a, b)| (a - b).abs() <= f32::from(tolerance) / 2.0)
}

fn pixels_agree(a: &[u8], b: &[u8], tolerance: u8) -> bool {
    // A small changed area is tolerated, but it cannot supply motion evidence.
    let bad = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0.iter())
        .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > tolerance))
        .count();
    bad * 100 <= a.len() / 4
}

pub(crate) fn match_frames(prev: &FrameAnalysis, next: &FrameAnalysis, config: StitchConfig) -> Decision {
    let reject = |reason| Decision {
        reason: Some(reason),
        evidence: None,
    };
    if (prev.width, prev.height) != (next.width, next.height) {
        return reject(StitchRejection::DimensionsChanged);
    }
    let h = prev.height;
    if prev.pixels == next.pixels {
        return Decision {
            reason: None,
            evidence: Some(stationary_evidence(h as u32)),
        };
    }
    // Exclude only contiguous, identical screen-anchored edge rows. Motion is
    // evaluated inside this region, never by how much an entire column changed.
    let mut top = 0;
    while top < h / 3 && prev.row(top) == next.row(top) {
        top += 1;
    }
    let mut bottom = 0;
    while bottom < h / 3 && prev.row(h - 1 - bottom) == next.row(h - 1 - bottom) {
        bottom += 1;
    }
    let content = h - top - bottom;
    let required_overlap = (config.min_overlap as usize).max(content / 4);
    if content <= required_overlap {
        return reject(StitchRejection::InsufficientOverlap);
    }
    let limit = (content - required_overlap) as i32;
    let mut best: Option<MatchEvidence> = None;
    let mut second: Option<MatchEvidence> = None;
    // Near-zero first finds the useful bound early for slow scrolling. Every
    // feasible displacement is still considered, in both directions.
    for delta in std::iter::once(0).chain((1..=limit).flat_map(|d| [d, -d])) {
        let overlap = content - delta.unsigned_abs() as usize;
        let p0 = top + delta.max(0) as usize;
        let n0 = top + (-delta).max(0) as usize;
        let mut informative = 0;
        let mut matching = 0;
        let mut pruned = false;
        let mut first = overlap;
        let mut last = 0;
        for y in 0..overlap {
            let a = &prev.rows[p0 + y];
            let b = &next.rows[n0 + y];
            if !a.informative && !b.informative {
                continue;
            }
            informative += 1;
            if row_agrees(a, b, config.pixel_tolerance) {
                matching += 1;
                first = first.min(y);
                last = y;
            }
            // Even if every remaining row supplies a match, this candidate
            // cannot enter the top two. Preserve the exact competitor score.
            let remaining = (overlap - y - 1) as u32;
            let upper_bound = (matching + remaining) as f32 / (informative + remaining) as f32;
            if second.is_some_and(|candidate| upper_bound < candidate.score) {
                pruned = true;
                break;
            }
        }
        if pruned || informative < config.min_informative_rows || matching < config.min_informative_rows || last.saturating_sub(first) < overlap / 3 {
            continue;
        }
        let evidence = MatchEvidence {
            displacement: delta,
            overlap: overlap as u32,
            matching_rows: matching,
            informative_rows: informative,
            score: matching as f32 / informative as f32,
            competitor_score: 0.0,
            competitor_displacement: None,
            fixed_top: top as u32,
            fixed_bottom: bottom as u32,
            next_seam: None,
            output_seam: None,
        };
        if best.is_none_or(|b| evidence.score > b.score) {
            second = best;
            best = Some(evidence);
        } else if second.is_none_or(|b| evidence.score > b.score) {
            second = Some(evidence);
        }
    }
    let Some(mut best) = best else {
        return reject(StitchRejection::InsufficientDetail);
    };
    best.competitor_score = second.map_or(0.0, |b| b.score);
    best.competitor_displacement = second.map(|b| b.displacement);
    let mut reason = if best.score < config.min_matching_rows {
        Some(StitchRejection::ContentChanged)
    } else if best.score - best.competitor_score < config.uniqueness_margin {
        Some(StitchRejection::Ambiguous)
    } else {
        None
    };
    if reason.is_none() {
        let p0 = top + best.displacement.max(0) as usize;
        let n0 = top + (-best.displacement).max(0) as usize;
        let overlap = best.overlap as usize;
        // Verify original RGB, including blank rows, across the entire overlap.
        // Coarse descriptors alone cannot authorize image mutation.
        let mut good = 0;
        let mut seam = None::<usize>;
        for y in 0..overlap {
            if pixels_agree(prev.row(p0 + y), next.row(n0 + y), config.pixel_tolerance) {
                good += 1;
                if seam.is_none_or(|current| y.abs_diff(overlap / 2) < current.abs_diff(overlap / 2)) {
                    seam = Some(y);
                }
            }
        }
        if (good as f32) < overlap as f32 * config.min_matching_rows {
            reason = Some(StitchRejection::ContentChanged);
        } else if best.displacement > 0 {
            // Find a verified pair close to the overlap centre. No unrelated
            // coordinate system is introduced between scoring and committing.
            best.next_seam = seam.map(|y| (n0 + y) as u32);
            if seam.is_none() {
                reason = Some(StitchRejection::ContentChanged);
            }
        }
    }
    Decision {
        reason,
        evidence: Some(best),
    }
}

pub(crate) fn stationary_evidence(height: u32) -> MatchEvidence {
    MatchEvidence {
        displacement: 0,
        overlap: height,
        matching_rows: 0,
        informative_rows: 0,
        score: 1.0,
        competitor_score: 0.0,
        competitor_displacement: None,
        fixed_top: 0,
        fixed_bottom: 0,
        next_seam: None,
        output_seam: None,
    }
}

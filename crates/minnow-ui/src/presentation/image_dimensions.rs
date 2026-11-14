//! Admission policy for display textures. Original image pixels remain untouched.
pub(crate) const MAX_TEXTURE_EDGE: u32 = 8192;
pub(crate) const MAX_TEXTURE_PIXELS: u64 = 16 * 1024 * 1024;

pub(crate) fn display_dimensions(width: u32, height: u32) -> Option<(u32, u32)> {
    if width == 0 || height == 0 {
        return None;
    }
    let pixels = u64::from(width) * u64::from(height);
    let scale = (f64::from(MAX_TEXTURE_EDGE) / f64::from(width))
        .min(f64::from(MAX_TEXTURE_EDGE) / f64::from(height))
        .min((MAX_TEXTURE_PIXELS as f64 / pixels as f64).sqrt())
        .min(1.0);
    Some((
        (f64::from(width) * scale).floor().max(1.0) as u32,
        (f64::from(height) * scale).floor().max(1.0) as u32,
    ))
}

pub(crate) fn preview_is_reduced(width: u32, height: u32) -> bool {
    display_dimensions(width, height).is_some_and(|dimensions| dimensions != (width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_admission_keeps_ordinary_images_and_bounds_extreme_aspect_ratios() {
        assert_eq!(display_dimensions(3840, 2160), Some((3840, 2160)));
        for (width, height) in [(32768, 32768), (1, u32::MAX), (u32::MAX, 1), (u32::MAX, u32::MAX)] {
            let (display_w, display_h) = display_dimensions(width, height).unwrap();
            assert!(display_w > 0 && display_h > 0);
            assert!(display_w <= MAX_TEXTURE_EDGE && display_h <= MAX_TEXTURE_EDGE);
            assert!(u64::from(display_w) * u64::from(display_h) <= MAX_TEXTURE_PIXELS);
        }
        assert_eq!(display_dimensions(0, 100), None);
        assert!(preview_is_reduced(32768, 32768));
        assert!(!preview_is_reduced(3840, 2160));
    }
}

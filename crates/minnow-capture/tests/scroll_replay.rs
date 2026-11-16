use image::{Rgba, RgbaImage, imageops};
use minnow_capture::{ScrollStitcher, StitchFrameStatus, StitchRejection};

fn assert_canvas(stitcher: &ScrollStitcher, expected: &RgbaImage) {
    let (canvas, height) = stitcher.current_image().unwrap();
    assert_eq!((canvas.width(), height), expected.dimensions());
    // Compare only committed rows; the allocation can include growth headroom.
    let committed = &canvas.as_raw()[..expected.as_raw().len()];
    assert!(committed == expected.as_raw(), "committed pixels differ from the document");
}

#[test]
fn article_replay_preserves_every_pixel_including_small_final_scrolls() {
    let document = image::load_from_memory(include_bytes!("fixtures/article.png")).unwrap().into_rgba8();
    let height = 1200;
    for step in [1, 2, 4, 24, 96, 240] {
        let mut stitcher = ScrollStitcher::new();
        for observation in 0..=8 {
            let offset = step * observation;
            let frame = imageops::crop_imm(&document, 0, offset, document.width(), height).to_image();
            let result = stitcher.process_frame_detailed(frame).unwrap();
            assert_eq!(
                result.status,
                StitchFrameStatus::Appended,
                "step={step}, observation={observation}: {result:?}"
            );
            let expected = imageops::crop_imm(&document, 0, 0, document.width(), height + offset).to_image();
            assert_canvas(&stitcher, &expected);
        }
    }
}

fn texture(x: u32, y: u32) -> u8 {
    let mut value = x.wrapping_mul(0x9e37_79b9) ^ y.wrapping_mul(0x85eb_ca6b);
    value = (value ^ (value >> 16)).wrapping_mul(0x7feb_352d);
    (value ^ (value >> 15)) as u8
}

fn periodic_document(period: u32, unique_region: bool) -> RgbaImage {
    RgbaImage::from_fn(240, 1200, |x, y| {
        let value = if unique_region && x >= 96 {
            texture(x, y)
        } else {
            100 + texture(x, y % period) % 20
        };
        Rgba([value, value, value, 255])
    })
}

fn with_fixed_edges(document: &RgbaImage, offset: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(document.width(), height + 24, |x, y| {
        if y < 12 {
            Rgba([20, 160, 50 + (x % 12) as u8, 255])
        } else if y >= height + 12 {
            Rgba([200, 50, 180 + (x % 12) as u8, 255])
        } else {
            *document.get_pixel(x, offset + y - 12)
        }
    })
}

#[test]
fn repeated_sidebar_does_not_override_unique_content_or_duplicate_fixed_edges() {
    for period in [16, 24, 32, 40, 48, 64, 96] {
        let document = periodic_document(period, true);
        for height in [120, 180, 240, 300] {
            for offset in [4, 8, 12, 20, 28, 36, 52, 68, 84, 100] {
                if offset >= height - 24 {
                    continue;
                }
                let mut stitcher = ScrollStitcher::new();
                stitcher.process_frame_detailed(with_fixed_edges(&document, 0, height)).unwrap();
                let result = stitcher.process_frame_detailed(with_fixed_edges(&document, offset, height)).unwrap();
                assert_eq!(
                    result.status,
                    StitchFrameStatus::Appended,
                    "period={period}, height={height}, offset={offset}: {result:?}"
                );
                assert_canvas(&stitcher, &with_fixed_edges(&document, 0, height + offset));
            }
        }
    }
}

#[test]
fn ambiguous_repeat_preserves_the_anchor_and_recovery_uses_that_anchor() {
    let document = periodic_document(32, false);
    let anchor = with_fixed_edges(&document, 0, 240);
    let mut stitcher = ScrollStitcher::new();
    stitcher.process_frame_detailed(anchor.clone()).unwrap();
    let rejected = stitcher.process_frame_detailed(with_fixed_edges(&document, 8, 240)).unwrap();
    assert_eq!(rejected.warning, Some(StitchRejection::Ambiguous));
    assert_canvas(&stitcher, &anchor);
    let recovery = stitcher.process_frame_detailed(anchor.clone()).unwrap();
    assert_eq!(recovery.status, StitchFrameStatus::Stationary);
    assert!(recovery.warning.is_none());
    assert_canvas(&stitcher, &anchor);
}

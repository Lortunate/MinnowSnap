//! ScreenCaptureKit acquisition scoped to one scrolling session. The filter
//! excludes the application identity, including its capture controls. Native
//! compositor exclusion must be verified on the supported macOS versions.
use super::{backend_error, monitor_geometry, select_monitor};
use block2::RcBlock;
use image::RgbaImage;
use minnow_capture::{CaptureError, CaptureSource, CaptureTarget};
use minnow_imaging::{CaptureGeometry, CapturedFrame, ImageAsset};
use objc2::{
    AnyThread,
    rc::{Retained, autoreleasepool},
};
use objc2_core_graphics::{CGDataProvider, CGImage, CGImageAlphaInfo, CGImageByteOrderInfo};
use objc2_foundation::{NSArray, NSError};
use objc2_screen_capture_kit::{SCContentFilter, SCScreenshotManager, SCShareableContent, SCStreamConfiguration};
use std::{sync::mpsc, time::Duration};
use xcap::Monitor;

const CALLBACK_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_SOURCE_PIXELS: u64 = 32 * 1024 * 1024;

// The filter is immutable after initialization. ScreenCaptureKit accepts it
// from arbitrary queues; this wrapper transfers its retained ownership exactly
// once from Apple's completion queue to the capture worker. It is never shared
// for mutation, and no AppKit UI object crosses this boundary.
struct PreparedFilter(Retained<SCContentFilter>);
unsafe impl Send for PreparedFilter {}

pub(super) struct ScreenSource {
    monitor: Monitor,
    geometry: CaptureGeometry,
    filter: PreparedFilter,
    config: Retained<SCStreamConfiguration>,
}

impl ScreenSource {
    pub(super) fn open(target: &CaptureTarget) -> Result<Self, CaptureError> {
        if !objc2::available!(macos = 14.0) {
            return Err(backend_error("scroll capture requires macOS 14 or later"));
        }
        autoreleasepool(|_| {
            let monitor = select_monitor(target)?;
            let geometry = monitor_geometry(&monitor)?;
            let (width, height) = geometry.pixel_size;
            if u64::from(width) * u64::from(height) > MAX_SOURCE_PIXELS {
                return Err(CaptureError::SourceTooLarge {
                    max_pixels: MAX_SOURCE_PIXELS,
                });
            }
            let display_id = monitor.id().map_err(backend_error)?;
            let (sender, receiver) = mpsc::sync_channel(1);
            let callback = RcBlock::new(move |content: *mut SCShareableContent, error: *mut NSError| {
                let result = unsafe {
                    if !error.is_null() {
                        Err(native_error(error))
                    } else if let Some(content) = content.as_ref() {
                        let display = content.displays().into_iter().find(|display| display.displayID() == display_id);
                        let applications: Vec<_> = content
                            .applications()
                            .into_iter()
                            .filter(|application| application.processID() == std::process::id() as i32)
                            .collect();
                        match display {
                            Some(display) if !applications.is_empty() => {
                                let filter = SCContentFilter::initWithDisplay_excludingApplications_exceptingWindows(
                                    SCContentFilter::alloc(),
                                    &display,
                                    &NSArray::from_retained_slice(&applications),
                                    &NSArray::new(),
                                );
                                Ok(PreparedFilter(filter))
                            }
                            Some(_) => Err(backend_error("capture application missing from shareable content")),
                            None => Err(CaptureError::TargetChanged),
                        }
                    } else {
                        Err(backend_error("ScreenCaptureKit returned no shareable content"))
                    }
                };
                let _ = sender.send(result);
            });
            unsafe {
                SCShareableContent::getShareableContentWithCompletionHandler(&callback);
            }
            let filter = receiver
                .recv_timeout(CALLBACK_TIMEOUT)
                .map_err(|_| backend_error("ScreenCaptureKit content discovery timed out"))??;
            let config = unsafe {
                let config = SCStreamConfiguration::new();
                config.setWidth(width as usize);
                config.setHeight(height as usize);
                config.setShowsCursor(false);
                config.setPixelFormat(u32::from_be_bytes(*b"BGRA"));
                config
            };
            Ok(Self {
                monitor,
                geometry,
                filter,
                config,
            })
        })
    }
}

impl CaptureSource for ScreenSource {
    fn capture(&mut self) -> Result<CapturedFrame, CaptureError> {
        autoreleasepool(|_| {
            if monitor_geometry(&self.monitor)? != self.geometry {
                return Err(CaptureError::TargetChanged);
            }
            let (sender, receiver) = mpsc::sync_channel(1);
            let expected_size = self.geometry.pixel_size;
            let callback = RcBlock::new(move |image: *mut CGImage, error: *mut NSError| {
                // Copy inside the callback: the borrowed CGImage must not escape
                // Apple's lifetime, and only an owned Rust buffer crosses queues.
                let result = unsafe {
                    if !error.is_null() {
                        Err(native_error(error))
                    } else if let Some(image) = image.as_ref() {
                        copy_image(image, expected_size)
                    } else {
                        Err(backend_error("ScreenCaptureKit returned no image"))
                    }
                };
                let _ = sender.send(result);
            });
            unsafe {
                SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(&self.filter.0, &self.config, Some(&callback));
            }
            let image = receiver
                .recv_timeout(CALLBACK_TIMEOUT)
                .map_err(|_| backend_error("ScreenCaptureKit frame acquisition timed out"))??;
            if monitor_geometry(&self.monitor)? != self.geometry {
                return Err(CaptureError::TargetChanged);
            }
            CapturedFrame::new(ImageAsset::new(image), self.geometry.clone()).map_err(Into::into)
        })
    }
}

unsafe fn native_error(error: *mut NSError) -> CaptureError {
    // Called only with a non-null NSError valid for the callback duration.
    backend_error(unsafe { &*error }.localizedDescription())
}

fn copy_image(image: &CGImage, expected: (u32, u32)) -> Result<RgbaImage, CaptureError> {
    let (width, height) = (CGImage::width(Some(image)), CGImage::height(Some(image)));
    if (width, height) != (expected.0 as usize, expected.1 as usize) {
        return Err(CaptureError::TargetChanged);
    }
    let row_bytes = width.checked_mul(4).ok_or_else(|| backend_error("capture image size overflow"))?;
    let stride = CGImage::bytes_per_row(Some(image));
    if CGImage::bits_per_component(Some(image)) != 8 || CGImage::bits_per_pixel(Some(image)) != 32 || stride < row_bytes {
        return Err(backend_error("unsupported ScreenCaptureKit pixel layout"));
    }
    // pixelFormat describes sample buffers; inspect the actual CGImage too.
    let alpha = CGImage::alpha_info(Some(image));
    if CGImage::byte_order_info(Some(image)) != CGImageByteOrderInfo::Order32Little
        || !matches!(
            alpha,
            CGImageAlphaInfo::PremultipliedFirst | CGImageAlphaInfo::First | CGImageAlphaInfo::NoneSkipFirst
        )
    {
        return Err(backend_error("unsupported ScreenCaptureKit channel order"));
    }
    let data = CGImage::data_provider(Some(image))
        .and_then(|provider| CGDataProvider::data(Some(&provider)))
        .ok_or_else(|| backend_error("capture image has no pixel storage"))?;
    // CGDataProviderCopyData returns an immutable copy owned by `data`.
    let bytes = unsafe { data.as_bytes_unchecked() };
    let source_len = stride.checked_mul(height).ok_or_else(|| backend_error("capture stride overflow"))?;
    if bytes.len() < source_len {
        return Err(backend_error("truncated capture image"));
    }
    let mut rgba = Vec::new();
    rgba.try_reserve_exact(row_bytes * height).map_err(|_| CaptureError::AllocationFailed)?;
    for row in bytes[..source_len].chunks_exact(stride) {
        for bgra in row[..row_bytes].as_chunks::<4>().0 {
            let a = if alpha == CGImageAlphaInfo::NoneSkipFirst { 255 } else { bgra[3] };
            let channel = |value: u8| {
                if alpha == CGImageAlphaInfo::PremultipliedFirst && a > 0 && a < 255 {
                    ((u16::from(value) * 255 + u16::from(a) / 2) / u16::from(a)).min(255) as u8
                } else {
                    value
                }
            };
            rgba.extend_from_slice(&[channel(bgra[2]), channel(bgra[1]), channel(bgra[0]), a]);
        }
    }
    RgbaImage::from_raw(expected.0, expected.1, rgba).ok_or_else(|| backend_error("invalid capture image"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2_core_graphics::{CGBitmapInfo, CGColorRenderingIntent, CGColorSpace};
    use std::ptr;

    fn with_image(data: &'static [u8], info: CGBitmapInfo, check: impl FnOnce(&CGImage)) {
        let space = CGColorSpace::new_device_rgb().unwrap();
        // Static bytes outlive the provider and image; no release callback is needed.
        let provider = unsafe { CGDataProvider::with_data(ptr::null_mut(), data.as_ptr().cast(), data.len(), None) }.unwrap();
        // Two pixels per row plus four bytes of padding, with all storage supplied.
        let image = unsafe {
            CGImage::new(
                2,
                2,
                8,
                32,
                12,
                Some(&space),
                info,
                Some(&provider),
                ptr::null(),
                false,
                CGColorRenderingIntent::RenderingIntentDefault,
            )
        }
        .unwrap();
        check(&image);
    }

    const PIXELS: &[u8] = &[
        16, 32, 64, 128, 5, 10, 20, 255, 99, 99, 99, 99, 0, 0, 0, 0, 30, 60, 90, 255, 99, 99, 99, 99,
    ];

    #[test]
    fn copies_padded_bgra_and_respects_each_supported_alpha_layout() {
        for (alpha, first_pixel, transparent_pixel) in [
            (CGImageAlphaInfo::PremultipliedFirst, [128, 64, 32, 128], [0, 0, 0, 0]),
            (CGImageAlphaInfo::First, [64, 32, 16, 128], [0, 0, 0, 0]),
            (CGImageAlphaInfo::NoneSkipFirst, [64, 32, 16, 255], [0, 0, 0, 255]),
        ] {
            let info = CGBitmapInfo(CGImageByteOrderInfo::Order32Little.0) | CGBitmapInfo(alpha.0);
            with_image(PIXELS, info, |image| {
                let actual = copy_image(image, (2, 2)).unwrap();
                assert_eq!(
                    actual.as_raw(),
                    &[first_pixel, [20, 10, 5, 255], transparent_pixel, [90, 60, 30, 255],].concat()
                );
            });
        }
    }

    #[test]
    fn rejects_changed_dimensions_before_reading_pixel_storage() {
        with_image(
            PIXELS,
            CGBitmapInfo(CGImageByteOrderInfo::Order32Little.0) | CGBitmapInfo(CGImageAlphaInfo::PremultipliedFirst.0),
            |image| {
                assert_eq!(copy_image(image, (3, 2)), Err(CaptureError::TargetChanged));
            },
        );
    }

    #[test]
    fn rejects_unexpected_channel_order_instead_of_swapping_colors_silently() {
        with_image(
            PIXELS,
            CGBitmapInfo(CGImageByteOrderInfo::Order32Big.0) | CGBitmapInfo(CGImageAlphaInfo::PremultipliedLast.0),
            |image| {
                assert!(matches!(copy_image(image, (2, 2)), Err(CaptureError::Backend(_))));
            },
        );
    }
}

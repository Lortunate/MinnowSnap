//! Admission for pixel allocations retained by Pin, not an RSS or GPU limit.
use minnow_application::{ErrorKind, OperationError, OperationResult};
use minnow_imaging::{ImageId, ImageRef};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

const MAX_PIN_WINDOWS: usize = 16;
const MAX_PIN_PIXEL_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(in crate::features::pin) struct PinBudget(Arc<Mutex<BudgetState>>);

impl Default for PinBudget {
    fn default() -> Self {
        Self::with_limits(MAX_PIN_WINDOWS, MAX_PIN_PIXEL_BYTES)
    }
}

impl PinBudget {
    pub(super) fn with_limits(max_windows: usize, max_pixel_bytes: u64) -> Self {
        Self(Arc::new(Mutex::new(BudgetState {
            max_windows,
            max_pixel_bytes,
            windows: 0,
            pixel_bytes: 0,
            sources: HashMap::new(),
        })))
    }

    fn state(&self) -> MutexGuard<'_, BudgetState> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(in crate::features::pin) fn reserve(&self, image: &ImageRef) -> OperationResult<PinLease> {
        let preview_size = crate::presentation::render_image::display_dimensions(image.width(), image.height())
            .ok_or_else(|| OperationError::new(ErrorKind::InvalidInput, "pin image must have a positive area"))?;
        let source_bytes = image.pixels().as_raw().capacity() as u64;
        let preview_bytes = u64::from(preview_size.0) * u64::from(preview_size.1) * 4;
        let mut state = self.state();
        let source = state.sources.get(&image.id());
        let additional_source = source.map_or(source_bytes, |_| 0);
        let required_bytes = state
            .pixel_bytes
            .checked_add(additional_source)
            .and_then(|bytes| bytes.checked_add(preview_bytes))
            .ok_or_else(|| OperationError::new(ErrorKind::InvalidInput, "pin pixel allocation size overflow"))?;
        if state.windows >= state.max_windows || required_bytes > state.max_pixel_bytes {
            return Err(OperationError::new(
                ErrorKind::Busy,
                "pin window or pixel storage limit reached; close a pin and retry",
            ));
        }
        state.windows += 1;
        state.pixel_bytes = required_bytes;
        state
            .sources
            .entry(image.id())
            .and_modify(|source| source.windows += 1)
            .or_insert(SourceAllocation {
                bytes: source_bytes,
                windows: 1,
            });
        Ok(PinLease(Arc::new(Reservation {
            budget: self.clone(),
            image_id: image.id(),
            preview_bytes,
            preview_size,
        })))
    }
}

#[derive(Debug)]
struct BudgetState {
    max_windows: usize,
    max_pixel_bytes: u64,
    windows: usize,
    pixel_bytes: u64,
    sources: HashMap<ImageId, SourceAllocation>,
}

#[derive(Debug)]
struct SourceAllocation {
    bytes: u64,
    windows: usize,
}

/// Clones share one reservation. Only the last owner returns its allocations.
#[derive(Clone, Debug)]
pub(in crate::features::pin) struct PinLease(Arc<Reservation>);

impl PinLease {
    pub(in crate::features::pin) fn preview_size(&self) -> (u32, u32) {
        self.0.preview_size
    }
}

#[derive(Debug)]
struct Reservation {
    budget: PinBudget,
    image_id: ImageId,
    preview_bytes: u64,
    preview_size: (u32, u32),
}

impl Drop for Reservation {
    fn drop(&mut self) {
        let mut state = self.budget.state();
        state.windows -= 1;
        state.pixel_bytes -= self.preview_bytes;
        let source = state.sources.get_mut(&self.image_id).expect("reserved pin source exists");
        source.windows -= 1;
        if source.windows == 0 {
            let source = state.sources.remove(&self.image_id).expect("last pin source exists");
            state.pixel_bytes -= source.bytes;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use minnow_imaging::ImageAsset;

    fn test_image(width: u32, height: u32) -> ImageRef {
        ImageAsset::new(image::RgbaImage::new(width, height))
    }

    fn usage(budget: &PinBudget) -> (usize, u64, usize) {
        let state = budget.state();
        (state.windows, state.pixel_bytes, state.sources.len())
    }

    #[test]
    fn source_identity_is_deduplicated_but_each_window_has_its_own_preview() {
        let budget = PinBudget::with_limits(3, 100_000);
        let image = test_image(64, 32);
        let first = budget.reserve(&image).unwrap();
        let second = budget.reserve(&image).unwrap();
        assert_eq!(usage(&budget), (2, 3 * 64 * 32 * 4, 1));
        let distinct_source = test_image(64, 32);
        let distinct = budget.reserve(&distinct_source).unwrap();
        assert_eq!(usage(&budget), (3, 5 * 64 * 32 * 4, 2));
        drop(distinct);
        let first_clone = first.clone();
        drop(first);
        assert_eq!(usage(&budget), (2, 3 * 64 * 32 * 4, 1));
        drop(second);
        assert_eq!(usage(&budget), (1, 2 * 64 * 32 * 4, 1));
        drop(first_clone);
        assert_eq!(usage(&budget), (0, 0, 0));
    }

    #[test]
    fn refused_reservations_leave_the_image_retryable_and_dropped_preparations_release() {
        let budget = PinBudget::with_limits(1, 100_000);
        let image = test_image(64, 32);
        let prepared = budget.reserve(&image).unwrap();
        let before = usage(&budget);
        assert_eq!(budget.reserve(&image).unwrap_err().kind, ErrorKind::Busy);
        assert_eq!(usage(&budget), before);
        // A window opener owns this reservation until it transfers it to a session.
        drop(prepared);
        let retry = budget.reserve(&image).unwrap();
        assert_eq!(usage(&budget), before);
        drop(retry);
        assert_eq!(usage(&budget), (0, 0, 0));
        assert_eq!(image.dimensions(), (64, 32));
    }

    #[test]
    fn actual_source_capacity_and_bounded_display_pixels_determine_admission() {
        let mut raw = Vec::with_capacity(1024 * 1024);
        raw.extend_from_slice(&[1, 2, 3, 255]);
        let image = ImageAsset::new(image::RgbaImage::from_raw(1, 1, raw).unwrap());
        let required = image.pixels().as_raw().capacity() as u64 + 4;
        let insufficient = PinBudget::with_limits(1, required - 1);
        assert_eq!(insufficient.reserve(&image).unwrap_err().kind, ErrorKind::Busy);
        assert_eq!(usage(&insufficient), (0, 0, 0));
        let fitting = PinBudget::with_limits(1, required);
        let lease = fitting.reserve(&image).unwrap();
        assert_eq!(usage(&fitting), (1, required, 1));
        drop(lease);

        let wide = test_image(9000, 1);
        let budget = PinBudget::default();
        let lease = budget.reserve(&wide).unwrap();
        assert_eq!(lease.preview_size(), (8192, 1));
        assert_eq!(usage(&budget).1, wide.pixels().as_raw().capacity() as u64 + 8192 * 4);
        assert_eq!(budget.reserve(&test_image(0, 1)).unwrap_err().kind, ErrorKind::InvalidInput);
    }
}

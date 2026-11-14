//! Per-call admission limits; these are not a process RSS or native ORT limit.
use minnow_application::ports::{ErrorKind, OperationError, OperationResult};

/// Limits for one OCR call, independent of the number of service callers.
///
/// Working-set admission includes the input Vec's retained capacity, module-owned
/// plans/results/tensors, image resampling scratch and an estimated model output.
/// Resident models/dictionaries, ORT arenas/intermediates, allocator overhead and
/// other callers are outside this estimate. Native output shape checks happen
/// after Run and cannot prevent an unexpected native allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InferenceBudget {
    pub max_input_pixels: usize,
    pub max_boxes: usize,
    pub max_crop_pixels: usize,
    /// Includes the recognizer's 32-pixel width alignment. Overwide text errors;
    /// it is never squeezed to this limit or silently truncated.
    pub max_normalized_width: u32,
    pub max_batch_tensor_bytes: usize,
    pub max_working_set_bytes: usize,
}

impl Default for InferenceBudget {
    fn default() -> Self {
        Self {
            max_input_pixels: 64 * 1024 * 1024,
            max_boxes: 1024,
            max_crop_pixels: 8 * 1024 * 1024,
            max_normalized_width: 4096,
            max_batch_tensor_bytes: 16 * 1024 * 1024,
            max_working_set_bytes: 512 * 1024 * 1024,
        }
    }
}

impl InferenceBudget {
    pub(crate) fn validate(&self) -> OperationResult<()> {
        if self.max_input_pixels == 0
            || self.max_boxes == 0
            || self.max_crop_pixels == 0
            || self.max_normalized_width == 0
            || self.max_batch_tensor_bytes == 0
            || self.max_working_set_bytes == 0
        {
            return Err(OperationError::new(ErrorKind::InvalidInput, "OCR inference budgets must be positive"));
        }
        Ok(())
    }

    /// Cheap admission before model work. The engine additionally checks actual
    /// retained input capacity and plans its stage-specific allocations.
    pub fn check_input(&self, width: u32, height: u32) -> OperationResult<()> {
        self.validate()?;
        let pixels = (width as usize).checked_mul(height as usize);
        let bytes = pixels.and_then(|pixels| pixels.checked_mul(4));
        if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err(OperationError::new(
                ErrorKind::InvalidInput,
                "OCR input dimensions must be positive and fit signed coordinates",
            ));
        }
        match (pixels, bytes) {
            (Some(pixels), Some(bytes)) if pixels <= self.max_input_pixels && bytes <= self.max_working_set_bytes => Ok(()),
            _ => Err(OperationError::new(
                ErrorKind::InvalidInput,
                format!(
                    "OCR input {width}x{height} exceeds pixel/working-set budget ({} pixels, {} bytes)",
                    self.max_input_pixels, self.max_working_set_bytes
                ),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_visible_budget_checks_dimensions_and_capacity_arithmetic() {
        let budget = InferenceBudget {
            max_input_pixels: 100,
            max_working_set_bytes: 400,
            ..Default::default()
        };
        assert!(budget.check_input(10, 10).is_ok());
        assert!(budget.check_input(10, 11).is_err());
        assert!(budget.check_input(0, 1).is_err());
        assert!(budget.check_input(u32::MAX, u32::MAX).is_err());
        assert!(InferenceBudget { max_boxes: 0, ..budget }.check_input(1, 1).is_err());
    }
}

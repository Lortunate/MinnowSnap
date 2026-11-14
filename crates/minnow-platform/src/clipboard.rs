use arboard::{Clipboard, ImageData};
use image::RgbaImage;
use minnow_application::ports::{ErrorKind, OperationError, OperationResult};
use std::borrow::Cow;
use std::sync::Mutex;

/// Retains clipboard ownership for the host lifetime, which is required by
/// Linux selection-based clipboards. Opening remains lazy and fallible.
#[derive(Default)]
pub struct NativeClipboard {
    clipboard: Mutex<Option<Clipboard>>,
}

impl std::fmt::Debug for NativeClipboard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("NativeClipboard").finish_non_exhaustive()
    }
}

impl NativeClipboard {
    pub fn copy_image(&self, image: &RgbaImage) -> OperationResult<()> {
        if image.width() == 0 || image.height() == 0 {
            return Err(OperationError::new(ErrorKind::InvalidInput, "cannot copy an empty image"));
        }
        self.with_clipboard(|clipboard| {
            clipboard.set_image(ImageData {
                width: image.width() as usize,
                height: image.height() as usize,
                bytes: Cow::Borrowed(image.as_raw()),
            })
        })
    }

    pub fn copy_text(&self, text: &str) -> OperationResult<()> {
        self.with_clipboard(|clipboard| clipboard.set_text(text))
    }

    fn with_clipboard(&self, operation: impl FnOnce(&mut Clipboard) -> Result<(), arboard::Error>) -> OperationResult<()> {
        let mut clipboard = self
            .clipboard
            .lock()
            .map_err(|_| OperationError::new(ErrorKind::Task, "clipboard ownership was poisoned"))?;
        if clipboard.is_none() {
            *clipboard = Some(Clipboard::new().map_err(|error| native_error("open clipboard", error))?);
        }
        operation(clipboard.as_mut().expect("clipboard initialized")).map_err(|error| native_error("update clipboard", error))
    }
}

fn native_error(operation: &str, error: arboard::Error) -> OperationError {
    OperationError::new(ErrorKind::Native, format!("failed to {operation}: {error}"))
}

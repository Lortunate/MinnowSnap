use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use minnow_application::ports::{ErrorKind, ExportOptions, ExportReceipt, ImageOutput, OperationError, OperationResult};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_EXPORT: AtomicU64 = AtomicU64::new(0);
const MAX_NAME_ATTEMPTS: usize = 64;

#[derive(Clone, Debug)]
pub struct DesktopOutput {
    default_directory: PathBuf,
    clipboard: Arc<crate::clipboard::NativeClipboard>,
}

impl DesktopOutput {
    pub fn new(default_directory: PathBuf) -> Self {
        Self {
            default_directory,
            clipboard: Arc::default(),
        }
    }
}

impl ImageOutput for DesktopOutput {
    fn copy_image(&self, image: &RgbaImage) -> OperationResult<()> {
        self.clipboard.copy_image(image)
    }

    fn copy_text(&self, text: &str) -> OperationResult<()> {
        self.clipboard.copy_text(text)
    }

    fn default_directory(&self) -> OperationResult<PathBuf> {
        if self.default_directory.as_os_str().is_empty() {
            Err(OperationError::new(ErrorKind::Unavailable, "the default image directory is unavailable"))
        } else {
            Ok(self.default_directory.clone())
        }
    }

    fn save_image(&self, image: &RgbaImage, options: &ExportOptions) -> OperationResult<ExportReceipt> {
        let encoded = encode_png(image, options.optimize)?;
        publish_image(&encoded, &options.directory, || {
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
            let sequence = NEXT_EXPORT.fetch_add(1, Ordering::Relaxed);
            format!("snap_{timestamp}_{sequence}.png")
        })
    }
}

fn encode_png(image: &RgbaImage, optimize: bool) -> OperationResult<Vec<u8>> {
    if image.width() == 0 || image.height() == 0 {
        return Err(OperationError::new(ErrorKind::InvalidInput, "cannot export an empty image"));
    }
    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(image.as_raw(), image.width(), image.height(), ExtendedColorType::Rgba8)
        .map_err(|error| OperationError::new(ErrorKind::Validation, format!("failed to encode PNG: {error}")))?;
    if !optimize {
        return Ok(encoded);
    }

    let mut options = oxipng::Options::from_preset(0);
    options.strip = oxipng::StripChunks::All;
    options.interlace = None;
    oxipng::optimize_from_memory(&encoded, &options)
        .map_err(|error| OperationError::new(ErrorKind::Validation, format!("failed to optimize PNG: {error}")))
}

fn publish_image(bytes: &[u8], directory: &Path, mut filename: impl FnMut() -> String) -> OperationResult<ExportReceipt> {
    if directory.as_os_str().is_empty() {
        return Err(OperationError::new(ErrorKind::InvalidInput, "the image output directory is empty"));
    }
    std::fs::create_dir_all(directory).map_err(|error| io_error("create output directory", directory, error))?;
    let mut staging = tempfile::Builder::new()
        .prefix(".minnow-export-")
        .tempfile_in(directory)
        .map_err(|error| io_error("create staging file", directory, error))?;
    staging.write_all(bytes).map_err(|error| io_error("write PNG", staging.path(), error))?;
    staging.flush().map_err(|error| io_error("flush PNG", staging.path(), error))?;
    staging
        .as_file()
        .sync_all()
        .map_err(|error| io_error("sync PNG", staging.path(), error))?;

    for _ in 0..MAX_NAME_ATTEMPTS {
        let destination = directory.join(filename());
        match staging.persist_noclobber(&destination) {
            Ok(_) => {
                // No fallible work after publication: an error must never be
                // reported for an image that this call already committed.
                return Ok(ExportReceipt {
                    path: destination,
                    bytes_written: bytes.len() as u64,
                });
            }
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
                staging = error.file;
            }
            Err(error) => return Err(io_error("commit PNG", &destination, error.error)),
        }
    }
    Err(OperationError::new(ErrorKind::Conflict, "could not reserve a unique screenshot filename"))
}

fn io_error(operation: &str, path: &Path, error: io::Error) -> OperationError {
    OperationError::new(ErrorKind::Io, format!("failed to {operation} at {}: {error}", path.display()))
}

/// Resolve a default once at the composition root, then inject it into output.
pub fn user_picture_directory(application_name: &str) -> OperationResult<PathBuf> {
    directories::UserDirs::new()
        .and_then(|dirs| dirs.picture_dir().map(|path| path.join(application_name)))
        .ok_or_else(|| OperationError::new(ErrorKind::Unavailable, "the user's pictures directory is unavailable"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    use std::sync::Arc;

    #[test]
    fn committed_receipt_describes_a_decodable_image() {
        let directory = tempfile::tempdir().unwrap();
        let image = RgbaImage::from_pixel(4, 3, Rgba([12, 34, 56, 255]));
        let output = DesktopOutput::new(directory.path().to_path_buf());
        let receipt = output
            .save_image(
                &image,
                &ExportOptions {
                    directory: directory.path().to_path_buf(),
                    optimize: false,
                },
            )
            .unwrap();

        assert_eq!(receipt.bytes_written, std::fs::metadata(&receipt.path).unwrap().len());
        assert_eq!(image::open(&receipt.path).unwrap().to_rgba8(), image);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn existing_exports_are_never_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let existing = directory.path().join("existing.png");
        std::fs::write(&existing, b"existing user image").unwrap();
        let mut attempts = 0;
        let receipt = publish_image(b"new image", directory.path(), || {
            attempts += 1;
            if attempts == 1 { "existing.png".into() } else { "new.png".into() }
        })
        .unwrap();

        assert_eq!(std::fs::read(existing).unwrap(), b"existing user image");
        assert_eq!(std::fs::read(receipt.path).unwrap(), b"new image");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn concurrent_exports_commit_distinct_files() {
        let directory = tempfile::tempdir().unwrap();
        let output = Arc::new(DesktopOutput::new(directory.path().to_path_buf()));
        let options = ExportOptions {
            directory: directory.path().to_path_buf(),
            optimize: false,
        };
        let receipts = std::thread::scope(|scope| {
            (0..8)
                .map(|value| {
                    let output = output.clone();
                    let options = options.clone();
                    scope.spawn(move || {
                        output
                            .save_image(&RgbaImage::from_pixel(2, 2, Rgba([value, 0, 0, 255])), &options)
                            .unwrap()
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>()
        });
        let unique = receipts.iter().map(|receipt| &receipt.path).collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), receipts.len());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), receipts.len());
    }

    #[test]
    fn write_failure_preserves_source_and_leaves_no_partial_result() {
        let directory = tempfile::tempdir().unwrap();
        let blocker = directory.path().join("not-a-directory");
        std::fs::write(&blocker, b"keep").unwrap();
        let image = RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255]));
        let before = image.clone();
        let result = DesktopOutput::new(blocker.clone()).save_image(
            &image,
            &ExportOptions {
                directory: blocker.clone(),
                optimize: false,
            },
        );
        assert_eq!(result.unwrap_err().kind, ErrorKind::Io);
        assert_eq!(image, before);
        assert_eq!(std::fs::read(blocker).unwrap(), b"keep");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

use super::{ApplicationServices, blocking};
use crate::ports::{ExportOptions, ExportReceipt};
use crate::sessions::lock;
use crate::{
    CompletionPolicy, ErrorKind, ImageCommand, ImageCommandResult, ImageInput, JobAdmission, JobHandle, OperationError, PreparedImage,
    SaveDestination,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Recent committed saves survive a closed document or a dropped UI waiter.
#[derive(Clone, Default)]
pub(super) struct ExportHistory(Arc<Mutex<VecDeque<ExportReceipt>>>);

impl ExportHistory {
    pub(super) fn record(&self, receipt: ExportReceipt) {
        let mut receipts = lock(&self.0);
        receipts.push_back(receipt);
        while receipts.len() > 128 {
            receipts.pop_front();
        }
    }

    pub(super) fn receipts(&self) -> Vec<ExportReceipt> {
        lock(&self.0).iter().cloned().collect()
    }
}

impl ApplicationServices {
    /// Prepare presentation data from a frozen input. A document-bound result
    /// must still belong to its current output revision when it completes.
    pub fn prepare_image(&self, input: ImageInput) -> JobHandle<PreparedImage> {
        let runtime = self.runtime.clone();
        self.jobs.spawn(
            "prepare image",
            input.scope(),
            CompletionPolicy::RequireCurrentSession,
            JobAdmission::ImageOutput,
            move |cancel| async move {
                blocking(&runtime, move || {
                    if cancel.is_cancelled() {
                        return Err(OperationError::cancelled());
                    }
                    input.resolve()
                })
                .await
            },
        )
    }

    /// Every command here commits an external effect. Closing or editing its
    /// source document cannot discard an already accepted frozen output.
    pub fn execute_image_command(&self, command: ImageCommand, input: ImageInput) -> JobHandle<ImageCommandResult> {
        let output = self.output.clone();
        let qr_decoder = self.qr_decoder.clone();
        let runtime = self.runtime.clone();
        let exports = self.exports.clone();
        let settings = self.settings_snapshot().output;
        self.jobs.spawn(
            "image output",
            input.scope(),
            CompletionPolicy::PreserveResult,
            JobAdmission::ImageOutput,
            move |cancel| async move {
                blocking(&runtime, move || {
                    if cancel.is_cancelled() {
                        return Err(OperationError::cancelled());
                    }
                    let PreparedImage { image, .. } = input.resolve()?;
                    match command {
                        ImageCommand::Copy => {
                            output.copy_image(image.pixels())?;
                            Ok(ImageCommandResult::Copied)
                        }
                        ImageCommand::Save { destination } => {
                            let directory = match destination {
                                SaveDestination::Directory(directory) => directory,
                                SaveDestination::Preferences => settings
                                    .save_path
                                    .as_custom_path()
                                    .map(std::path::Path::to_path_buf)
                                    .map(Ok)
                                    .unwrap_or_else(|| output.default_directory())?,
                            };
                            let receipt = output.save_image(
                                image.pixels(),
                                &ExportOptions {
                                    directory,
                                    optimize: settings.oxipng_enabled,
                                },
                            )?;
                            // Record the committed effect before presentation delivery.
                            exports.record(receipt.clone());
                            Ok(ImageCommandResult::Saved(receipt))
                        }
                        ImageCommand::CopyQr => {
                            let text = qr_decoder
                                .decode_qr(image.pixels())?
                                .ok_or_else(|| OperationError::new(ErrorKind::Validation, "no QR code found in the image"))?;
                            if cancel.is_cancelled() {
                                return Err(OperationError::cancelled());
                            }
                            output.copy_text(&text)?;
                            Ok(ImageCommandResult::QrCopied(text))
                        }
                        ImageCommand::CopyColor => {
                            let pixel = image.pixels().get_pixel(image.width() / 2, image.height() / 2);
                            let color = format!("#{:02X}{:02X}{:02X}", pixel[0], pixel[1], pixel[2]);
                            output.copy_text(&color)?;
                            Ok(ImageCommandResult::ColorCopied(color))
                        }
                    }
                })
                .await
            },
        )
    }
}

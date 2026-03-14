use crate::core::capture::service::CaptureService;
use crate::core::geometry::Rect;
use std::str::FromStr;
use tracing::info;

#[derive(Debug, PartialEq, Clone)]
pub enum CaptureAction {
    Copy,
    Save,
    Pin,
    Ocr,
    Scroll,
    QrCode,
    Unknown,
}

impl FromStr for CaptureAction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "copy" => Ok(CaptureAction::Copy),
            "save" => Ok(CaptureAction::Save),
            "pin" => Ok(CaptureAction::Pin),
            "ocr" => Ok(CaptureAction::Ocr),
            "scroll" => Ok(CaptureAction::Scroll),
            "qrcode" => Ok(CaptureAction::QrCode),
            _ => Ok(CaptureAction::Unknown),
        }
    }
}

#[derive(Debug)]
pub enum ActionResult {
    Copied,
    Saved(String),
    PinRequested(String, bool),
    OcrResult(String),
    NoOp,
    Error(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CaptureInputMode {
    CropSelection,
    FullImage,
}

pub struct ActionContext {
    pub path: String,
    pub rect: Rect,
    pub input_mode: CaptureInputMode,
}

impl ActionContext {
    pub fn crop_selection(path: String, rect: Rect) -> Self {
        Self {
            path,
            rect,
            input_mode: CaptureInputMode::CropSelection,
        }
    }

    pub fn full_image(path: String) -> Self {
        Self {
            path,
            rect: Rect::empty(),
            input_mode: CaptureInputMode::FullImage,
        }
    }
}

impl CaptureAction {
    pub fn execute(&self, ctx: ActionContext) -> ActionResult {
        info!("Executing action: {:?} for path: {}", self, ctx.path);

        match self {
            CaptureAction::Copy => Self::handle_copy(ctx),
            CaptureAction::Save => Self::handle_save(ctx),
            CaptureAction::Pin => Self::handle_pin_ocr(ctx, false),
            CaptureAction::Ocr => Self::handle_pin_ocr(ctx, true),
            CaptureAction::QrCode => Self::handle_qrcode(ctx),
            CaptureAction::Scroll | CaptureAction::Unknown => ActionResult::NoOp,
        }
    }

    fn handle_copy(ctx: ActionContext) -> ActionResult {
        if CaptureService::copy_image(&ctx.path, ctx.rect, ctx.input_mode) {
            ActionResult::Copied
        } else {
            ActionResult::Error("Failed to process image for Copy".to_string())
        }
    }

    fn handle_save(ctx: ActionContext) -> ActionResult {
        match CaptureService::save_region_to_user_dir(&ctx.path, ctx.rect, ctx.input_mode) {
            Ok(path) => ActionResult::Saved(path),
            Err(e) => ActionResult::Error(e),
        }
    }

    fn handle_pin_ocr(ctx: ActionContext, auto_ocr: bool) -> ActionResult {
        if let Some(cropped) = CaptureService::resolve_image(&ctx.path, ctx.rect, ctx.input_mode)
            && let Some(temp_path) = CaptureService::save_temp(&cropped)
        {
            return ActionResult::PinRequested(temp_path, auto_ocr);
        }
        ActionResult::Error("Failed to process image for Pin/OCR".to_string())
    }

    fn handle_qrcode(ctx: ActionContext) -> ActionResult {
        if let Some(content) = CaptureService::detect_qrcode(&ctx.path, ctx.rect, ctx.input_mode) {
            ActionResult::OcrResult(content)
        } else {
            ActionResult::Error("No QR Code detected".to_string())
        }
    }
}

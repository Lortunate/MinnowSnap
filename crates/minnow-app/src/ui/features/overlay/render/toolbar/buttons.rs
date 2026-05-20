use super::OverlayToolbarState;
use crate::services::assets::asset_paths;
use crate::services::capture::action::CaptureAction;
use crate::services::i18n;
use crate::services::ocr::service;
use crate::ui::features::overlay::state::{AnnotationCommand, AnnotationTool, CaptureCommand, LifecycleCommand, OverlayCommand};
use gpui::SharedString;
use gpui_component::IconNamed;

#[derive(Clone, Copy)]
pub(super) enum ToolbarIcon {
    Arrow,
    Rectangle,
    Circle,
    Counter,
    Text,
    Mosaic,
    Undo,
    Redo,
    Ocr,
    QrCode,
    Scroll,
    Save,
    Pin,
    Copy,
    Cancel,
}

impl IconNamed for ToolbarIcon {
    fn path(self) -> SharedString {
        match self {
            Self::Arrow => asset_paths::icons::ARROW_INSERT,
            Self::Rectangle => asset_paths::icons::SQUARE,
            Self::Circle => asset_paths::icons::CIRCLE,
            Self::Counter => asset_paths::icons::COUNTER_1,
            Self::Text => asset_paths::icons::TEXT_FIELDS,
            Self::Mosaic => asset_paths::icons::BLUR_ON,
            Self::Undo => asset_paths::icons::UNDO,
            Self::Redo => asset_paths::icons::REDO,
            Self::Ocr => asset_paths::icons::TEXT_FIELDS,
            Self::QrCode => asset_paths::icons::CROP_FREE,
            Self::Scroll => asset_paths::icons::SCROLL,
            Self::Save => asset_paths::icons::SAVE,
            Self::Pin => asset_paths::icons::KEEP,
            Self::Copy => asset_paths::icons::FILE_COPY,
            Self::Cancel => asset_paths::icons::CLOSE,
        }
        .into()
    }
}

#[derive(Clone, Copy)]
pub(super) struct ToolbarButtonSpec {
    pub(super) id: &'static str,
    pub(super) icon: ToolbarIcon,
    pub(super) tooltip: fn() -> String,
    pub(super) command: fn() -> OverlayCommand,
    pub(super) active: fn(OverlayToolbarState) -> bool,
    pub(super) disabled: fn(OverlayToolbarState) -> bool,
    pub(super) visible: fn() -> bool,
}

const fn always_inactive(_: OverlayToolbarState) -> bool {
    false
}

const fn never_disabled(_: OverlayToolbarState) -> bool {
    false
}

const fn always_visible() -> bool {
    true
}

fn ocr_enabled() -> bool {
    service::is_enabled()
}

fn is_arrow_active(state: OverlayToolbarState) -> bool {
    state.tool == Some(AnnotationTool::Arrow)
}

fn is_rectangle_active(state: OverlayToolbarState) -> bool {
    state.tool == Some(AnnotationTool::Rectangle)
}

fn is_circle_active(state: OverlayToolbarState) -> bool {
    state.tool == Some(AnnotationTool::Circle)
}

fn is_counter_active(state: OverlayToolbarState) -> bool {
    state.tool == Some(AnnotationTool::Counter)
}

fn is_text_active(state: OverlayToolbarState) -> bool {
    state.tool == Some(AnnotationTool::Text)
}

fn is_mosaic_active(state: OverlayToolbarState) -> bool {
    state.tool == Some(AnnotationTool::Mosaic)
}

fn is_undo_disabled(state: OverlayToolbarState) -> bool {
    !state.can_undo
}

fn is_redo_disabled(state: OverlayToolbarState) -> bool {
    !state.can_redo
}

fn cmd_arrow() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Arrow))
}

fn cmd_rect() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Rectangle))
}

fn cmd_circle() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Circle))
}

fn cmd_counter() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Counter))
}

fn cmd_text() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Text))
}

fn cmd_mosaic() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::SetTool(AnnotationTool::Mosaic))
}

fn cmd_undo() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::Undo)
}

fn cmd_redo() -> OverlayCommand {
    OverlayCommand::Annotation(AnnotationCommand::Redo)
}

fn cmd_ocr() -> OverlayCommand {
    OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Ocr))
}

fn cmd_qr() -> OverlayCommand {
    OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::QrCode))
}

fn cmd_save() -> OverlayCommand {
    OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Save))
}

fn cmd_scroll() -> OverlayCommand {
    OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Scroll))
}

fn cmd_pin() -> OverlayCommand {
    OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Pin))
}

fn cmd_copy() -> OverlayCommand {
    OverlayCommand::Capture(CaptureCommand::Execute(CaptureAction::Copy))
}

fn cmd_cancel() -> OverlayCommand {
    OverlayCommand::Lifecycle(LifecycleCommand::CloseIntent)
}

pub(super) const SAVE_BUTTON_ID: &str = "overlay-save";

pub(super) const TOOL_BUTTONS: &[ToolbarButtonSpec] = &[
    ToolbarButtonSpec {
        id: "overlay-tool-arrow",
        icon: ToolbarIcon::Arrow,
        tooltip: i18n::overlay::annotation_tool_arrow,
        command: cmd_arrow,
        active: is_arrow_active,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-tool-rect",
        icon: ToolbarIcon::Rectangle,
        tooltip: i18n::overlay::annotation_tool_rectangle,
        command: cmd_rect,
        active: is_rectangle_active,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-tool-circle",
        icon: ToolbarIcon::Circle,
        tooltip: i18n::overlay::annotation_tool_circle,
        command: cmd_circle,
        active: is_circle_active,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-tool-counter",
        icon: ToolbarIcon::Counter,
        tooltip: i18n::overlay::annotation_tool_counter,
        command: cmd_counter,
        active: is_counter_active,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-tool-text",
        icon: ToolbarIcon::Text,
        tooltip: i18n::overlay::annotation_tool_text,
        command: cmd_text,
        active: is_text_active,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-tool-mosaic",
        icon: ToolbarIcon::Mosaic,
        tooltip: i18n::overlay::annotation_tool_mosaic,
        command: cmd_mosaic,
        active: is_mosaic_active,
        disabled: never_disabled,
        visible: always_visible,
    },
];

pub(super) const HISTORY_BUTTONS: &[ToolbarButtonSpec] = &[
    ToolbarButtonSpec {
        id: "overlay-undo",
        icon: ToolbarIcon::Undo,
        tooltip: i18n::overlay::annotation_undo,
        command: cmd_undo,
        active: always_inactive,
        disabled: is_undo_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-redo",
        icon: ToolbarIcon::Redo,
        tooltip: i18n::overlay::annotation_redo,
        command: cmd_redo,
        active: always_inactive,
        disabled: is_redo_disabled,
        visible: always_visible,
    },
];

pub(super) const CAPTURE_BUTTONS: &[ToolbarButtonSpec] = &[
    ToolbarButtonSpec {
        id: "overlay-ocr",
        icon: ToolbarIcon::Ocr,
        tooltip: i18n::common::ocr,
        command: cmd_ocr,
        active: always_inactive,
        disabled: never_disabled,
        visible: ocr_enabled,
    },
    ToolbarButtonSpec {
        id: "overlay-qr",
        icon: ToolbarIcon::QrCode,
        tooltip: i18n::common::scan_qr,
        command: cmd_qr,
        active: always_inactive,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-scroll",
        icon: ToolbarIcon::Scroll,
        tooltip: i18n::common::scroll,
        command: cmd_scroll,
        active: always_inactive,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: SAVE_BUTTON_ID,
        icon: ToolbarIcon::Save,
        tooltip: i18n::common::save,
        command: cmd_save,
        active: always_inactive,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-pin",
        icon: ToolbarIcon::Pin,
        tooltip: i18n::common::pin,
        command: cmd_pin,
        active: always_inactive,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-copy",
        icon: ToolbarIcon::Copy,
        tooltip: i18n::common::copy,
        command: cmd_copy,
        active: always_inactive,
        disabled: never_disabled,
        visible: always_visible,
    },
    ToolbarButtonSpec {
        id: "overlay-cancel",
        icon: ToolbarIcon::Cancel,
        tooltip: i18n::common::cancel,
        command: cmd_cancel,
        active: always_inactive,
        disabled: never_disabled,
        visible: always_visible,
    },
];

pub(crate) fn toolbar_button_count() -> usize {
    TOOL_BUTTONS.iter().filter(|spec| (spec.visible)()).count()
        + HISTORY_BUTTONS.iter().filter(|spec| (spec.visible)()).count()
        + CAPTURE_BUTTONS.iter().filter(|spec| (spec.visible)()).count()
}

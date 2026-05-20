use crate::services::assets::asset_paths;
use crate::services::i18n;
use gpui_component::IconNamed;

#[derive(Clone, Copy)]
pub(super) enum LongCaptureToolbarIcon {
    Save,
    Pin,
    Copy,
    Cancel,
}

impl IconNamed for LongCaptureToolbarIcon {
    fn path(self) -> gpui::SharedString {
        match self {
            Self::Save => asset_paths::icons::SAVE.into(),
            Self::Pin => asset_paths::icons::KEEP.into(),
            Self::Copy => asset_paths::icons::FILE_COPY.into(),
            Self::Cancel => asset_paths::icons::CLOSE.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LongCaptureToolbarAction {
    Save,
    Pin,
    Copy,
    Cancel,
}

impl LongCaptureToolbarAction {
    pub(crate) const ORDERED: [Self; 4] = [Self::Save, Self::Pin, Self::Copy, Self::Cancel];

    pub(super) fn id(self) -> &'static str {
        match self {
            Self::Save => "long-capture-save",
            Self::Pin => "long-capture-pin",
            Self::Copy => "long-capture-copy",
            Self::Cancel => "long-capture-cancel",
        }
    }

    pub(super) fn icon(self) -> LongCaptureToolbarIcon {
        match self {
            Self::Save => LongCaptureToolbarIcon::Save,
            Self::Pin => LongCaptureToolbarIcon::Pin,
            Self::Copy => LongCaptureToolbarIcon::Copy,
            Self::Cancel => LongCaptureToolbarIcon::Cancel,
        }
    }

    pub(super) fn tooltip(self) -> String {
        match self {
            Self::Save => i18n::common::save(),
            Self::Pin => i18n::common::pin(),
            Self::Copy => i18n::common::copy(),
            Self::Cancel => i18n::common::cancel(),
        }
    }

    pub(super) fn disabled_when_busy(self) -> bool {
        self != Self::Cancel
    }
}

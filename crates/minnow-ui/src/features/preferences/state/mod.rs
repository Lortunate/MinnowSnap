use crate::presentation::hotkeys::HotkeyAction;
use crate::presentation::i18n;
use gpui::SharedString;

pub(super) mod frame;
pub(super) mod general;
pub(super) mod ocr;
pub(super) mod shortcuts;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PreferencesPage {
    #[default]
    General,
    Shortcuts,
    Notifications,
    Ocr,
    About,
}

impl PreferencesPage {
    pub(crate) const ALL: [Self; 5] = [Self::General, Self::Shortcuts, Self::Notifications, Self::Ocr, Self::About];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Shortcuts => "shortcuts",
            Self::Notifications => "notifications",
            Self::Ocr => "ocr",
            Self::About => "about",
        }
    }

    pub(crate) fn title(self) -> SharedString {
        match self {
            Self::General => i18n::preferences::page_general().into(),
            Self::Shortcuts => i18n::preferences::page_shortcuts().into(),
            Self::Notifications => i18n::preferences::page_notifications().into(),
            Self::Ocr => i18n::preferences::page_ocr().into(),
            Self::About => i18n::preferences::page_about().into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NoticeTone {
    Info,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreferencesNotice {
    pub(crate) message: SharedString,
    pub(crate) tone: NoticeTone,
}

impl PreferencesNotice {
    pub(crate) fn info(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
            tone: NoticeTone::Info,
        }
    }

    pub(crate) fn error(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
            tone: NoticeTone::Error,
        }
    }

    pub(crate) fn is_error(&self) -> bool {
        self.tone == NoticeTone::Error
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PreferencesState {
    pub(crate) active_page: PreferencesPage,
    pub(crate) notice: Option<PreferencesNotice>,
    pub(crate) shortcut_recording: Option<HotkeyAction>,
}

impl PreferencesState {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn select_page(&mut self, page: PreferencesPage) -> bool {
        if self.active_page == page {
            return false;
        }

        self.active_page = page;
        self.clear_notice();
        if page != PreferencesPage::Shortcuts {
            self.stop_shortcut_recording();
        }

        true
    }

    pub(crate) fn show_notice(&mut self, notice: PreferencesNotice) {
        self.notice = Some(notice);
    }

    pub(crate) fn clear_notice(&mut self) -> bool {
        self.notice.take().is_some()
    }

    pub(crate) fn start_shortcut_recording(&mut self, action: HotkeyAction) {
        self.shortcut_recording = Some(action);
        self.clear_notice();
    }

    pub(crate) fn stop_shortcut_recording(&mut self) {
        self.shortcut_recording = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_general_page_and_idle_state() {
        let state = PreferencesState::new();

        assert_eq!(state.active_page, PreferencesPage::General);
        assert!(state.notice.is_none());
        assert!(state.shortcut_recording.is_none());
    }

    #[test]
    fn page_metadata_order_and_ids_are_stable() {
        let ids: Vec<&'static str> = PreferencesPage::ALL.into_iter().map(PreferencesPage::id).collect();

        assert_eq!(ids, vec!["general", "shortcuts", "notifications", "ocr", "about"]);
    }

    #[test]
    fn selecting_a_new_page_clears_notice_and_shortcut_recording() {
        let mut state = PreferencesState::new();
        state.show_notice(PreferencesNotice::info("Heads up"));
        state.start_shortcut_recording(HotkeyAction::Capture);

        assert!(state.select_page(PreferencesPage::Ocr));
        assert_eq!(state.active_page, PreferencesPage::Ocr);
        assert!(state.notice.is_none());
        assert!(state.shortcut_recording.is_none());
    }

    #[test]
    fn selecting_the_same_page_is_a_noop() {
        let mut state = PreferencesState::new();

        assert!(!state.select_page(PreferencesPage::General));
    }
}

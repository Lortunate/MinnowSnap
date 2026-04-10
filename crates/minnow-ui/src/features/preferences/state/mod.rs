pub(super) mod frame;
pub(super) mod general;
pub(super) mod notifications;
pub(super) mod ocr;
pub(super) mod shortcuts;
pub(super) mod state;
pub(super) mod store;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MutationResult {
    pub(crate) refresh_windows: bool,
    pub(crate) clear_notice: bool,
}

impl MutationResult {
    pub(crate) const NONE: Self = Self {
        refresh_windows: false,
        clear_notice: false,
    };

    pub(crate) fn refresh_windows() -> Self {
        Self {
            refresh_windows: true,
            clear_notice: false,
        }
    }

    pub(crate) fn clear_notice(mut self) -> Self {
        self.clear_notice = true;
        self
    }
}

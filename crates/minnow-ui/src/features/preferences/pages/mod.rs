mod about;
pub(super) mod chrome;
pub(super) mod components;
mod general;
mod notifications;
mod ocr;
mod shortcuts;

use super::{state::frame::ActivePageProps, view::PreferencesView};
use gpui::{AnyElement, Context};

pub(super) fn render_active_page(page: &ActivePageProps, cx: &mut Context<PreferencesView>) -> AnyElement {
    match page {
        ActivePageProps::General(props) => general::render(props, cx),
        ActivePageProps::Notifications(props) => notifications::render(props, cx),
        ActivePageProps::Shortcuts(props) => shortcuts::render(props, cx),
        ActivePageProps::Ocr(props) => ocr::render(props, cx),
        ActivePageProps::About(props) => about::render(props, cx),
    }
}

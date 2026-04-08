use gpui::{App, AsyncApp};

pub fn app_ready(cx: &mut AsyncApp) -> bool {
    !crate::core::shutdown::is_shutting_down() && cx.update(|_| ()).is_ok()
}

pub fn update_app(cx: &mut AsyncApp, f: impl FnOnce(&mut App)) -> bool {
    !crate::core::shutdown::is_shutting_down() && cx.update(f).is_ok()
}

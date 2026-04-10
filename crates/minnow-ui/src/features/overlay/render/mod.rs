pub(crate) mod annotation;
pub(crate) mod components;
#[cfg(feature = "overlay-diagnostics")]
pub(crate) mod diagnostics;
pub(crate) mod hud;
pub(crate) mod layout;
pub(crate) mod picker;
pub(crate) mod properties;
pub(crate) mod selection;
pub(crate) mod toolbar;

use gpui::{App, Window};
use std::rc::Rc;

use crate::features::overlay::state::OverlayCommand;

pub(crate) type OverlayActionHandler = Rc<dyn Fn(OverlayCommand, &mut Window, &mut App)>;

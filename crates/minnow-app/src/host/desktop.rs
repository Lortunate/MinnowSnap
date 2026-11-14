use super::{APP_NAME, assets::CAPTURE_SOUND_BYTES};
use ab_glyph::FontArc;
use minnow_application::ports::{ErrorKind, OperationError, OperationResult, ShutterSound};
use minnow_application::preferences::{AutoStartUpdate, EffectiveShortcuts, NativePreferences, NativePreferencesSnapshot, ShortcutUpdateFailure};
use minnow_platform::{
    AppPaths,
    autostart::AutoStartRegistration,
    fonts::SystemFonts,
    hotkeys::{NativeHotkeyRegistry, ShortcutBindings},
};
use minnow_settings::ShortcutSettings;
use minnow_ui::{UiHost, UiPaths};
use raw_window_handle::HasWindowHandle;
use std::{cell::RefCell, path::Path};

pub(crate) struct DesktopShutter;

impl ShutterSound for DesktopShutter {
    fn play(&self) -> OperationResult<()> {
        minnow_platform::sound::play_shutter(CAPTURE_SOUND_BYTES)
    }
}

/// Main-thread native state. Application policy does not live in this adapter.
pub(crate) struct DesktopHost {
    pub(crate) hotkeys: RefCell<Option<NativeHotkeyRegistry>>,
    autostart: OperationResult<AutoStartRegistration>,
    native: RefCell<NativePreferencesSnapshot>,
    fonts: SystemFonts,
    annotation_font: OperationResult<FontArc>,
    paths: UiPaths,
}

impl DesktopHost {
    pub(crate) fn new(paths: &AppPaths, executable: &Path) -> Self {
        let fonts = SystemFonts::default();
        let annotation_font = fonts
            .annotation_font()
            .inspect_err(|error| tracing::warn!(%error, "Annotation text font is unavailable"));
        let (hotkeys, shortcut_failure) = match NativeHotkeyRegistry::new() {
            Ok(registry) => (Some(registry), None),
            Err(error) => {
                tracing::error!(%error, "Global shortcuts are unavailable");
                (None, Some(shortcut_failure(error)))
            }
        };
        let autostart = AutoStartRegistration::new(APP_NAME, executable);
        let auto_start = autostart.as_ref().map_err(Clone::clone).and_then(AutoStartRegistration::is_enabled);
        Self {
            hotkeys: RefCell::new(hotkeys),
            autostart,
            native: RefCell::new(NativePreferencesSnapshot {
                auto_start,
                auto_start_failure: None,
                shortcuts: EffectiveShortcuts::default(),
                shortcut_failure,
            }),
            fonts,
            annotation_font,
            paths: UiPaths {
                config_file: paths.config_file().into(),
                logs_dir: paths.logs_dir().into(),
                models_dir: paths.ocr_models_dir().into(),
            },
        }
    }

    pub(crate) fn release_hotkeys(&self) {
        if let Some(mut registry) = self.hotkeys.borrow_mut().take()
            && let Err(error) = registry.unregister_all()
        {
            tracing::error!(?error, "Failed to release all global shortcuts");
            let failure = shortcut_failure(error);
            let mut native = self.native.borrow_mut();
            native.shortcuts = failure.actual.as_ref().clone();
            native.shortcut_failure = Some(failure);
        }
    }
}

impl NativePreferences for DesktopHost {
    fn set_auto_start(&self, enabled: bool) -> AutoStartUpdate {
        let result = self
            .autostart
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|registration| registration.set_enabled(enabled));
        let actual = self.autostart.as_ref().map_err(Clone::clone).and_then(AutoStartRegistration::is_enabled);
        let mut native = self.native.borrow_mut();
        let update = AutoStartUpdate {
            actual,
            failure: result.err(),
        };
        native.auto_start = update.actual.clone();
        native.auto_start_failure = update.failure.clone();
        update
    }

    fn apply_shortcuts(&self, proposed: &ShortcutSettings) -> Result<EffectiveShortcuts, ShortcutUpdateFailure> {
        let mut registry = self.hotkeys.borrow_mut();
        let result = match registry.as_mut() {
            Some(registry) => registry
                .apply(&ShortcutBindings {
                    capture: proposed.capture.clone(),
                    quick_capture: proposed.quick_capture.clone(),
                })
                .map(effective_shortcuts)
                .map_err(shortcut_failure),
            None => Err(self.native.borrow().shortcut_failure.clone().unwrap_or_else(|| ShortcutUpdateFailure {
                message: "global shortcut registration is unavailable".into(),
                actual: Box::default(),
                compensation_errors: Vec::new(),
            })),
        };
        let mut native = self.native.borrow_mut();
        match &result {
            Ok(actual) => {
                native.shortcuts = actual.clone();
                native.shortcut_failure = None;
            }
            Err(error) => {
                native.shortcuts = error.actual.as_ref().clone();
                native.shortcut_failure = Some(error.clone());
            }
        }
        result
    }

    fn snapshot(&self) -> NativePreferencesSnapshot {
        self.native.borrow().clone()
    }

    fn refresh(&self) -> NativePreferencesSnapshot {
        let auto_start = self.autostart.as_ref().map_err(Clone::clone).and_then(AutoStartRegistration::is_enabled);
        let shortcuts = self.hotkeys.borrow().as_ref().map(|registry| effective_shortcuts(registry.current()));
        let mut native = self.native.borrow_mut();
        native.auto_start = auto_start;
        if let Some(shortcuts) = shortcuts {
            native.shortcuts = shortcuts;
        }
        native.clone()
    }
}

impl UiHost for DesktopHost {
    fn set_always_on_top(&self, window: &gpui::Window) -> OperationResult<()> {
        minnow_platform::window::set_always_on_top(HasWindowHandle::window_handle(window).map_err(native_error)?, true)
    }

    fn set_click_through(&self, window: &gpui::Window, enabled: bool) -> OperationResult<()> {
        minnow_platform::window::set_click_through(HasWindowHandle::window_handle(window).map_err(native_error)?, enabled)
    }

    fn start_window_drag(&self, window: &gpui::Window) -> OperationResult<()> {
        minnow_platform::window::start_system_drag(HasWindowHandle::window_handle(window).map_err(native_error)?)
    }

    fn system_fonts(&self) -> Vec<String> {
        self.fonts.families().unwrap_or_else(|error| {
            tracing::warn!(%error, "Failed to enumerate system fonts");
            Vec::new()
        })
    }

    fn annotation_font(&self) -> OperationResult<FontArc> {
        self.annotation_font.clone()
    }
    fn paths(&self) -> &UiPaths {
        &self.paths
    }
}

fn effective_shortcuts(actual: minnow_platform::hotkeys::EffectiveBindings) -> EffectiveShortcuts {
    EffectiveShortcuts {
        capture: actual.capture,
        quick_capture: actual.quick_capture,
        unrouted: actual.unrouted,
        uncertain: actual.uncertain,
    }
}

fn shortcut_failure(error: minnow_platform::hotkeys::HotkeyUpdateFailure) -> ShortcutUpdateFailure {
    ShortcutUpdateFailure {
        message: error.message,
        actual: Box::new(effective_shortcuts(*error.actual)),
        compensation_errors: error.compensation_errors,
    }
}

fn native_error(error: impl std::fmt::Display) -> OperationError {
    OperationError::new(ErrorKind::Native, error.to_string())
}

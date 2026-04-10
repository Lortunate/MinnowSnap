use crate::shell::async_ui::{app_ready, update_app};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui::{App, AsyncApp, Global};
use minnow_core::settings::{SETTINGS, ShortcutSettings};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub const DEFAULT_CAPTURE_SHORTCUT: &str = "F1";
pub const DEFAULT_QUICK_CAPTURE_SHORTCUT: &str = "F2";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotkeyAction {
    Capture,
    QuickCapture,
}

impl HotkeyAction {
    fn default_shortcut(self) -> &'static str {
        match self {
            Self::Capture => DEFAULT_CAPTURE_SHORTCUT,
            Self::QuickCapture => DEFAULT_QUICK_CAPTURE_SHORTCUT,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShortcutBindings {
    pub capture: String,
    pub quick_capture: String,
}

impl Default for ShortcutBindings {
    fn default() -> Self {
        Self {
            capture: DEFAULT_CAPTURE_SHORTCUT.to_string(),
            quick_capture: DEFAULT_QUICK_CAPTURE_SHORTCUT.to_string(),
        }
    }
}

impl ShortcutBindings {
    pub fn from_settings(settings: &ShortcutSettings) -> Self {
        Self {
            capture: resolve_shortcut(&settings.capture, HotkeyAction::Capture),
            quick_capture: resolve_shortcut(&settings.quick_capture, HotkeyAction::QuickCapture),
        }
    }

    pub fn with_capture(&self, shortcut: &str) -> Self {
        Self {
            capture: resolve_shortcut(shortcut, HotkeyAction::Capture),
            quick_capture: self.quick_capture.clone(),
        }
    }

    pub fn with_quick_capture(&self, shortcut: &str) -> Self {
        Self {
            capture: self.capture.clone(),
            quick_capture: resolve_shortcut(shortcut, HotkeyAction::QuickCapture),
        }
    }

    pub fn has_conflict(&self) -> bool {
        shortcuts_conflict(&self.capture, &self.quick_capture)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotkeyUpdateError {
    Conflict,
}

#[derive(Default)]
pub struct HotkeyIds {
    pub screen_capture: Option<u32>,
    pub quick_capture: Option<u32>,
}

pub struct HotkeyManager {
    pub manager: Option<GlobalHotKeyManager>,
    pub ids: Arc<Mutex<HotkeyIds>>,
    pub screen_hotkey: Option<HotKey>,
    pub quick_hotkey: Option<HotKey>,
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self {
            manager: None,
            ids: Arc::new(Mutex::new(HotkeyIds::default())),
            screen_hotkey: None,
            quick_hotkey: None,
        }
    }
}

pub struct HotkeyService {
    manager: HotkeyManager,
    action_tx: UnboundedSender<HotkeyAction>,
    sink: HotkeyActionSink,
}

impl Global for HotkeyService {}

#[derive(Clone)]
pub struct HotkeyActionSink {
    open_capture_overlay: Arc<dyn Fn(&mut App) + Send + Sync>,
    run_quick_capture: Arc<dyn Fn() + Send + Sync>,
}

impl HotkeyActionSink {
    pub fn new<F1, F2>(open_capture_overlay: F1, run_quick_capture: F2) -> Self
    where
        F1: Fn(&mut App) + Send + Sync + 'static,
        F2: Fn() + Send + Sync + 'static,
    {
        Self {
            open_capture_overlay: Arc::new(open_capture_overlay),
            run_quick_capture: Arc::new(run_quick_capture),
        }
    }

    fn open_capture_overlay(&self, app: &mut App) {
        (self.open_capture_overlay)(app);
    }

    fn run_quick_capture(&self) {
        (self.run_quick_capture)();
    }
}

pub fn install_hotkey_service(cx: &mut App, sink: HotkeyActionSink) {
    let (action_tx, action_rx) = unbounded_channel();
    let mut service = HotkeyService::new(action_tx, sink);
    service.register_from_settings();
    let sink = service.sink.clone();
    let shutdown_token = minnow_core::platform::shutdown::cancellation_token().unwrap_or_default();
    cx.spawn(async move |cx| {
        hotkey_action_loop(action_rx, shutdown_token, sink, cx).await;
        GlobalHotKeyEvent::set_event_handler::<fn(GlobalHotKeyEvent)>(None);
    })
    .detach();
    cx.set_global(service);
}

pub fn parse_hotkey(shortcut: &str) -> Option<HotKey> {
    if shortcut.is_empty() {
        return None;
    }
    match HotKey::from_str(shortcut) {
        Ok(hk) => Some(hk),
        Err(e) => {
            error!("Failed to parse hotkey '{shortcut}': {e}");
            None
        }
    }
}

pub fn resolve_shortcut(shortcut: &str, action: HotkeyAction) -> String {
    let trimmed = shortcut.trim();
    if trimmed.is_empty() {
        action.default_shortcut().to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn shortcuts_conflict(capture: &str, quick_capture: &str) -> bool {
    normalize_shortcut_for_compare(capture, HotkeyAction::Capture) == normalize_shortcut_for_compare(quick_capture, HotkeyAction::QuickCapture)
}

pub fn format_keystroke(keystroke: &gpui::Keystroke) -> Option<String> {
    if is_modifier_only_key(&keystroke.key) {
        return None;
    }

    let mut tokens = Vec::new();
    if keystroke.modifiers.control {
        tokens.push("Ctrl".to_string());
    }
    if keystroke.modifiers.alt {
        tokens.push("Alt".to_string());
    }
    if keystroke.modifiers.shift {
        tokens.push("Shift".to_string());
    }
    if keystroke.modifiers.platform {
        #[cfg(target_os = "macos")]
        tokens.push("Cmd".to_string());

        #[cfg(not(target_os = "macos"))]
        tokens.push("Win".to_string());
    }
    if keystroke.modifiers.function {
        tokens.push("Fn".to_string());
    }

    tokens.push(display_key_token(&keystroke.key));
    Some(tokens.join("+"))
}

impl HotkeyManager {
    pub fn register_global_hotkeys<F1, F2>(&mut self, screen_shortcut: &str, quick_shortcut: &str, screen_callback: F1, quick_callback: F2)
    where
        F1: Fn() + Send + Sync + 'static,
        F2: Fn() + Send + Sync + 'static,
    {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to create hotkey manager: {e}");
                return;
            }
        };

        self.manager = Some(manager);
        let screen_hotkey = parse_hotkey(screen_shortcut);
        let quick_hotkey = parse_hotkey(quick_shortcut);

        if let Some(ref m) = self.manager {
            if let Some(hk) = screen_hotkey {
                if let Err(e) = m.register(hk) {
                    error!("Failed to register screen hotkey: {e}");
                } else {
                    self.ids.lock().unwrap().screen_capture = Some(hk.id());
                    self.screen_hotkey = Some(hk);
                    info!("Screen capture hotkey registered: {screen_shortcut}");
                }
            }

            if let Some(hk) = quick_hotkey {
                if let Err(e) = m.register(hk) {
                    error!("Failed to register quick hotkey: {e}");
                } else {
                    self.ids.lock().unwrap().quick_capture = Some(hk.id());
                    self.quick_hotkey = Some(hk);
                    info!("Quick capture hotkey registered: {quick_shortcut}");
                }
            }
        }

        let ids_clone = self.ids.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state != HotKeyState::Pressed {
                return;
            }

            let ids = ids_clone.lock().unwrap();

            if let Some(id) = ids.screen_capture
                && event.id == id
            {
                info!("Screen capture hotkey triggered (id: {id})");
                screen_callback();
            }

            if let Some(id) = ids.quick_capture
                && event.id == id
            {
                info!("Quick capture hotkey triggered (id: {id})");
                quick_callback();
            }
        }));

        info!("Global hotkeys registered");
    }

    pub fn update_shortcut(&mut self, shortcut: &str, is_screen: bool) {
        let mut shortcut_str = shortcut.to_string();
        if shortcut_str.is_empty() {
            let defaults = ShortcutSettings::default();
            shortcut_str = if is_screen { defaults.capture } else { defaults.quick_capture };
        }

        let Some(manager) = &self.manager else {
            return;
        };
        let new_hotkey = parse_hotkey(&shortcut_str);

        let current_hotkey = if is_screen { &mut self.screen_hotkey } else { &mut self.quick_hotkey };

        if let Some(old) = current_hotkey
            && let Err(e) = manager.unregister(*old)
        {
            error!("Failed to unregister hotkey: {e}");
        }

        let mut next_hotkey = None;

        if let Some(hotkey) = new_hotkey {
            if let Err(e) = manager.register(hotkey) {
                error!("Failed to register hotkey: {e}");
            } else {
                next_hotkey = Some(hotkey);
                let label = if is_screen { "Screen capture" } else { "Quick capture" };
                info!("{label} hotkey updated to: {shortcut_str}");
            }
        } else {
            let label = if is_screen { "Screen capture" } else { "Quick capture" };
            info!("{label} hotkey cleared");
        }

        *current_hotkey = next_hotkey;

        let mut ids = self.ids.lock().unwrap();
        if is_screen {
            ids.screen_capture = next_hotkey.map(|hk| hk.id());
        } else {
            ids.quick_capture = next_hotkey.map(|hk| hk.id());
        }
    }
}

impl HotkeyService {
    pub fn new(action_tx: UnboundedSender<HotkeyAction>, sink: HotkeyActionSink) -> Self {
        Self {
            manager: HotkeyManager::default(),
            action_tx,
            sink,
        }
    }

    pub fn current_bindings(&self) -> ShortcutBindings {
        let settings = SETTINGS.lock().unwrap().get();
        ShortcutBindings::from_settings(&settings.shortcuts)
    }

    pub fn register_from_settings(&mut self) {
        if self.manager.manager.is_some() {
            return;
        }

        let bindings = self.current_bindings();
        let screen_capture = self.action_tx.clone();
        let quick_capture = self.action_tx.clone();
        self.manager.register_global_hotkeys(
            &bindings.capture,
            &bindings.quick_capture,
            move || enqueue_action(&screen_capture, HotkeyAction::Capture),
            move || enqueue_action(&quick_capture, HotkeyAction::QuickCapture),
        );
    }

    pub fn update_bindings(&mut self, bindings: ShortcutBindings) -> Result<(), HotkeyUpdateError> {
        if bindings.has_conflict() {
            return Err(HotkeyUpdateError::Conflict);
        }

        {
            let mut settings = SETTINGS.lock().unwrap();
            settings.set_capture_shortcut(bindings.capture.clone());
            settings.set_quick_capture_shortcut(bindings.quick_capture.clone());
        }

        if self.manager.manager.is_none() {
            self.register_from_settings();
        } else {
            self.manager.update_shortcut(&bindings.capture, true);
            self.manager.update_shortcut(&bindings.quick_capture, false);
        }

        Ok(())
    }
}

fn normalize_shortcut_for_compare(shortcut: &str, action: HotkeyAction) -> String {
    resolve_shortcut(shortcut, action)
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn is_modifier_only_key(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "shift" | "control" | "ctrl" | "alt" | "command" | "cmd" | "super" | "platform" | "function" | "fn"
    )
}

fn display_key_token(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.len() == 1 {
        return lower.to_ascii_uppercase();
    }

    if lower.starts_with('f') && lower.chars().skip(1).all(|ch| ch.is_ascii_digit()) {
        return lower.to_ascii_uppercase();
    }

    match lower.as_str() {
        "escape" => "Escape".to_string(),
        "space" => "Space".to_string(),
        "tab" => "Tab".to_string(),
        "enter" => "Enter".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" => "Delete".to_string(),
        "up" => "Up".to_string(),
        "down" => "Down".to_string(),
        "left" => "Left".to_string(),
        "right" => "Right".to_string(),
        other => other.to_ascii_uppercase(),
    }
}

fn enqueue_action(action_tx: &UnboundedSender<HotkeyAction>, action: HotkeyAction) {
    if let Err(err) = action_tx.send(action) {
        error!("Failed to enqueue hotkey action: {err}");
    }
}

async fn hotkey_action_loop(
    mut action_rx: UnboundedReceiver<HotkeyAction>,
    shutdown_token: CancellationToken,
    sink: HotkeyActionSink,
    cx: &mut AsyncApp,
) {
    loop {
        tokio::select! {
            _ = shutdown_token.cancelled() => return,
            action = action_rx.recv() => {
                let Some(action) = action else {
                    return;
                };

                if !handle_hotkey_action(action, &sink, cx) {
                    return;
                }
            }
        }
    }
}

fn handle_hotkey_action(action: HotkeyAction, sink: &HotkeyActionSink, async_app: &mut AsyncApp) -> bool {
    if !app_ready(async_app) {
        return false;
    }

    match action {
        HotkeyAction::Capture => {
            if !update_app(async_app, |app| {
                sink.open_capture_overlay(app);
            }) {
                return false;
            }
        }
        HotkeyAction::QuickCapture => {
            if !app_ready(async_app) {
                return false;
            }
            sink.run_quick_capture();
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_CAPTURE_SHORTCUT, DEFAULT_QUICK_CAPTURE_SHORTCUT, HotkeyAction, ShortcutBindings, format_keystroke, resolve_shortcut,
        shortcuts_conflict,
    };

    #[test]
    fn empty_shortcuts_fall_back_to_defaults() {
        assert_eq!(resolve_shortcut("", HotkeyAction::Capture), DEFAULT_CAPTURE_SHORTCUT);
        assert_eq!(resolve_shortcut("   ", HotkeyAction::QuickCapture), DEFAULT_QUICK_CAPTURE_SHORTCUT);
    }

    #[test]
    fn bindings_update_independently() {
        let bindings = ShortcutBindings::default().with_capture("Ctrl+Shift+A").with_quick_capture("Ctrl+Alt+B");

        assert_eq!(bindings.capture, "Ctrl+Shift+A");
        assert_eq!(bindings.quick_capture, "Ctrl+Alt+B");
    }

    #[test]
    fn repeated_shortcuts_are_rejected() {
        assert!(shortcuts_conflict("ctrl+shift+a", "Ctrl+Shift+A"));
        assert!(ShortcutBindings::default().with_quick_capture("F1").has_conflict());
    }

    #[test]
    fn blank_shortcuts_still_use_default_values_for_conflict_checks() {
        assert!(shortcuts_conflict("", DEFAULT_CAPTURE_SHORTCUT));
        assert!(!shortcuts_conflict("", DEFAULT_QUICK_CAPTURE_SHORTCUT));
    }

    #[test]
    fn keystrokes_are_formatted_for_global_hotkeys() {
        let keystroke = gpui::Keystroke::parse("ctrl-shift-f2").expect("valid keystroke");
        assert_eq!(format_keystroke(&keystroke), Some("Ctrl+Shift+F2".to_string()));
    }
}

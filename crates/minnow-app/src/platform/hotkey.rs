use crate::platform::{app_ready, update_app};
use crate::services::hotkeys::{HotkeyAction, HotkeyUpdateError, ShortcutBindings};
use crate::services::settings::{self, SettingsAction};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui::{App, AsyncApp, Global};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

#[derive(Default)]
struct HotkeyIds {
    screen_capture: Option<u32>,
    quick_capture: Option<u32>,
}

impl HotkeyIds {
    fn action_for_event(&self, event: &GlobalHotKeyEvent) -> Option<HotkeyAction> {
        if event.state != HotKeyState::Pressed {
            return None;
        }

        if self.screen_capture == Some(event.id) {
            Some(HotkeyAction::Capture)
        } else if self.quick_capture == Some(event.id) {
            Some(HotkeyAction::QuickCapture)
        } else {
            None
        }
    }

    fn set(&mut self, action: HotkeyAction, id: Option<u32>) {
        match action {
            HotkeyAction::Capture => self.screen_capture = id,
            HotkeyAction::QuickCapture => self.quick_capture = id,
        }
    }
}

struct NativeHotkeyRegistry {
    backend: Option<GlobalHotKeyManager>,
    ids: Arc<Mutex<HotkeyIds>>,
    screen_hotkey: Option<HotKey>,
    quick_hotkey: Option<HotKey>,
}

impl Default for NativeHotkeyRegistry {
    fn default() -> Self {
        Self {
            backend: None,
            ids: Arc::new(Mutex::new(HotkeyIds::default())),
            screen_hotkey: None,
            quick_hotkey: None,
        }
    }
}

pub struct HotkeyService {
    registry: NativeHotkeyRegistry,
    action_tx: UnboundedSender<HotkeyAction>,
    sink: HotkeyActionSink,
}

fn hotkey_ids_guard<'a>(ids: &'a Mutex<HotkeyIds>) -> MutexGuard<'a, HotkeyIds> {
    match ids.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            error!("Hotkey id lock was poisoned; recovering registered ids");
            let guard = poisoned.into_inner();
            ids.clear_poison();
            guard
        }
    }
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
    let shutdown_token = crate::platform::shutdown::cancellation_token().unwrap_or_default();
    cx.spawn(async move |cx| {
        hotkey_action_loop(action_rx, shutdown_token, sink, cx).await;
        GlobalHotKeyEvent::set_event_handler::<fn(GlobalHotKeyEvent)>(None);
    })
    .detach();
    cx.set_global(service);
}

impl NativeHotkeyRegistry {
    fn is_initialized(&self) -> bool {
        self.backend.is_some()
    }

    fn register(&mut self, bindings: &ShortcutBindings, action_tx: UnboundedSender<HotkeyAction>) {
        let manager = match GlobalHotKeyManager::new() {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to create hotkey manager: {e}");
                return;
            }
        };

        self.backend = Some(manager);
        let screen_hotkey = crate::services::hotkeys::parse_hotkey(&bindings.capture);
        let quick_hotkey = crate::services::hotkeys::parse_hotkey(&bindings.quick_capture);

        if let Some(ref backend) = self.backend {
            if let Some(hk) = screen_hotkey {
                if let Err(e) = backend.register(hk) {
                    error!("Failed to register screen hotkey: {e}");
                } else {
                    hotkey_ids_guard(&self.ids).screen_capture = Some(hk.id());
                    self.screen_hotkey = Some(hk);
                    info!("Screen capture hotkey registered: {}", bindings.capture);
                }
            }

            if let Some(hk) = quick_hotkey {
                if let Err(e) = backend.register(hk) {
                    error!("Failed to register quick hotkey: {e}");
                } else {
                    hotkey_ids_guard(&self.ids).quick_capture = Some(hk.id());
                    self.quick_hotkey = Some(hk);
                    info!("Quick capture hotkey registered: {}", bindings.quick_capture);
                }
            }
        }

        let ids_clone = self.ids.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            let action = hotkey_ids_guard(&ids_clone).action_for_event(&event);
            if let Some(action) = action {
                info!("Hotkey triggered (id: {}, action: {action:?})", event.id);
                enqueue_action(&action_tx, action);
            }
        }));

        info!("Global hotkeys registered");
    }

    fn update_shortcut(&mut self, shortcut: &str, action: HotkeyAction) {
        let mut shortcut_str = shortcut.to_string();
        if shortcut_str.is_empty() {
            let defaults = ShortcutBindings::default();
            shortcut_str = match action {
                HotkeyAction::Capture => defaults.capture,
                HotkeyAction::QuickCapture => defaults.quick_capture,
            };
        }

        let Some(backend) = &self.backend else {
            return;
        };
        let new_hotkey = crate::services::hotkeys::parse_hotkey(&shortcut_str);

        let current_hotkey = match action {
            HotkeyAction::Capture => &mut self.screen_hotkey,
            HotkeyAction::QuickCapture => &mut self.quick_hotkey,
        };

        if let Some(old) = current_hotkey
            && let Err(e) = backend.unregister(*old)
        {
            error!("Failed to unregister hotkey: {e}");
        }

        let mut next_hotkey = None;

        if let Some(hotkey) = new_hotkey {
            if let Err(e) = backend.register(hotkey) {
                error!("Failed to register hotkey: {e}");
            } else {
                next_hotkey = Some(hotkey);
                info!("{} hotkey updated to: {shortcut_str}", action_label(action));
            }
        } else {
            info!("{} hotkey cleared", action_label(action));
        }

        *current_hotkey = next_hotkey;
        hotkey_ids_guard(&self.ids).set(action, next_hotkey.map(|hotkey| hotkey.id()));
    }
}

fn action_label(action: HotkeyAction) -> &'static str {
    match action {
        HotkeyAction::Capture => "Screen capture",
        HotkeyAction::QuickCapture => "Quick capture",
    }
}

impl HotkeyService {
    fn new(action_tx: UnboundedSender<HotkeyAction>, sink: HotkeyActionSink) -> Self {
        Self {
            registry: NativeHotkeyRegistry::default(),
            action_tx,
            sink,
        }
    }

    pub fn current_bindings(&self) -> ShortcutBindings {
        let settings = settings::shortcut_settings();
        ShortcutBindings::from_settings(&settings)
    }

    fn register_from_settings(&mut self) {
        if self.registry.is_initialized() {
            return;
        }

        let bindings = self.current_bindings();
        self.registry.register(&bindings, self.action_tx.clone());
    }

    pub fn update_bindings(&mut self, bindings: ShortcutBindings) -> Result<(), HotkeyUpdateError> {
        if bindings.has_conflict() {
            return Err(HotkeyUpdateError::Conflict);
        }

        settings::apply(SettingsAction::Shortcuts {
            capture: bindings.capture.clone(),
            quick_capture: bindings.quick_capture.clone(),
        });

        if !self.registry.is_initialized() {
            self.register_from_settings();
        } else {
            self.registry.update_shortcut(&bindings.capture, HotkeyAction::Capture);
            self.registry.update_shortcut(&bindings.quick_capture, HotkeyAction::QuickCapture);
        }

        Ok(())
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
            sink.run_quick_capture();
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::HotkeyIds;
    use crate::services::hotkeys::{
        DEFAULT_CAPTURE_SHORTCUT, DEFAULT_QUICK_CAPTURE_SHORTCUT, HotkeyAction, ShortcutBindings, format_keystroke, resolve_shortcut,
        shortcuts_conflict,
    };
    use global_hotkey::{GlobalHotKeyEvent, HotKeyState};

    #[test]
    fn native_events_map_to_domain_actions_only_on_press() {
        let ids = HotkeyIds {
            screen_capture: Some(7),
            quick_capture: Some(11),
        };

        assert_eq!(
            ids.action_for_event(&GlobalHotKeyEvent {
                id: 7,
                state: HotKeyState::Pressed,
            }),
            Some(HotkeyAction::Capture)
        );
        assert_eq!(
            ids.action_for_event(&GlobalHotKeyEvent {
                id: 11,
                state: HotKeyState::Pressed,
            }),
            Some(HotkeyAction::QuickCapture)
        );
        assert_eq!(
            ids.action_for_event(&GlobalHotKeyEvent {
                id: 7,
                state: HotKeyState::Released,
            }),
            None
        );
        assert_eq!(
            ids.action_for_event(&GlobalHotKeyEvent {
                id: 99,
                state: HotKeyState::Pressed,
            }),
            None
        );
    }

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

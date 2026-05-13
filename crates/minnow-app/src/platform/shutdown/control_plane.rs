use super::ShutdownTrigger;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct ShutdownControlPlane {
    bus: broadcast::Sender<ShutdownTrigger>,
    token: CancellationToken,
}

static SHUTDOWN_CONTROL: Lazy<Mutex<Option<ShutdownControlPlane>>> = Lazy::new(|| Mutex::new(None));

pub(super) fn init() {
    let (bus, _) = broadcast::channel(8);
    if let Ok(mut slot) = SHUTDOWN_CONTROL.lock() {
        *slot = Some(ShutdownControlPlane {
            bus,
            token: CancellationToken::new(),
        });
    }
}

pub(super) fn clear() {
    if let Ok(mut slot) = SHUTDOWN_CONTROL.lock() {
        *slot = None;
    }
}

pub(super) fn subscribe() -> Option<broadcast::Receiver<ShutdownTrigger>> {
    SHUTDOWN_CONTROL
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|control| control.bus.subscribe()))
}

pub(super) fn cancellation_token() -> Option<CancellationToken> {
    SHUTDOWN_CONTROL
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|control| control.token.clone()))
}

pub(super) fn broadcast(trigger: ShutdownTrigger) {
    if let Ok(slot) = SHUTDOWN_CONTROL.lock()
        && let Some(control) = slot.as_ref()
    {
        let _ = control.bus.send(trigger);
    }
}

pub(super) fn cancel() {
    if let Ok(slot) = SHUTDOWN_CONTROL.lock()
        && let Some(control) = slot.as_ref()
    {
        control.token.cancel();
    }
}

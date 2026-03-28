#![cfg_attr(not(feature = "overlay-diagnostics"), allow(dead_code))]

use std::time::Instant;

#[derive(Clone, Copy, Debug, Default)]
struct CounterSet {
    pointer_events: u64,
    pointer_queued: u64,
    pointer_coalesced: u64,
    pointer_applied: u64,
    render_calls: u64,
    refresh_calls: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct OverlayDiagnosticsSnapshot {
    pub pointer_hz: f64,
    pub render_hz: f64,
    pub refresh_hz: f64,
    pub apply_ratio: f64,
    pub coalesced_ratio: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct OverlayDiagnostics {
    counters: CounterSet,
    last_counters: CounterSet,
    last_instant: Instant,
    cached_snapshot: OverlayDiagnosticsSnapshot,
}

impl Default for OverlayDiagnostics {
    fn default() -> Self {
        Self {
            counters: CounterSet::default(),
            last_counters: CounterSet::default(),
            last_instant: Instant::now(),
            cached_snapshot: OverlayDiagnosticsSnapshot::default(),
        }
    }
}

impl OverlayDiagnostics {
    pub fn on_pointer_event(&mut self) {
        self.counters.pointer_events += 1;
    }

    pub fn on_pointer_queue(&mut self, queued_first: bool) {
        self.counters.pointer_queued += 1;
        if !queued_first {
            self.counters.pointer_coalesced += 1;
        }
    }

    pub fn on_pointer_applied(&mut self) {
        self.counters.pointer_applied += 1;
    }

    pub fn on_render(&mut self) {
        self.counters.render_calls += 1;
    }

    pub fn on_refresh(&mut self) {
        self.counters.refresh_calls += 1;
    }

    pub fn snapshot(&mut self) -> OverlayDiagnosticsSnapshot {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_instant).as_secs_f64();
        if elapsed < 0.2 {
            return self.cached_snapshot.clone();
        }

        let pointer_delta = self.counters.pointer_events.saturating_sub(self.last_counters.pointer_events) as f64;
        let render_delta = self.counters.render_calls.saturating_sub(self.last_counters.render_calls) as f64;
        let refresh_delta = self.counters.refresh_calls.saturating_sub(self.last_counters.refresh_calls) as f64;
        let queued_delta = self.counters.pointer_queued.saturating_sub(self.last_counters.pointer_queued) as f64;
        let applied_delta = self.counters.pointer_applied.saturating_sub(self.last_counters.pointer_applied) as f64;
        let coalesced_delta = self.counters.pointer_coalesced.saturating_sub(self.last_counters.pointer_coalesced) as f64;

        self.cached_snapshot = OverlayDiagnosticsSnapshot {
            pointer_hz: pointer_delta / elapsed,
            render_hz: render_delta / elapsed,
            refresh_hz: refresh_delta / elapsed,
            apply_ratio: if queued_delta > 0.0 { applied_delta / queued_delta } else { 0.0 },
            coalesced_ratio: if queued_delta > 0.0 { coalesced_delta / queued_delta } else { 0.0 },
        };

        self.last_counters = self.counters;
        self.last_instant = now;
        self.cached_snapshot.clone()
    }
}

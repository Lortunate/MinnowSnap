#![cfg_attr(not(feature = "overlay-diagnostics"), allow(dead_code))]

use std::time::Instant;

use crate::ui::overlay::annotation::AnnotationRasterDiagnostics;

const DIAG_LOG_EVERY_ENV: &str = "MINNOW_OVERLAY_DIAG_LOG_EVERY";

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
    pub annotation_committed_rebuilds: u64,
    pub annotation_composed_rebuilds: u64,
    pub annotation_interaction_base_rebuilds: u64,
    pub annotation_drawing_fast_path_hits: u64,
    pub annotation_moving_fast_path_hits: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct OverlayDiagnostics {
    counters: CounterSet,
    last_counters: CounterSet,
    last_instant: Instant,
    cached_snapshot: OverlayDiagnosticsSnapshot,
    log_every_windows: u64,
    elapsed_windows: u64,
}

impl Default for OverlayDiagnostics {
    fn default() -> Self {
        Self {
            counters: CounterSet::default(),
            last_counters: CounterSet::default(),
            last_instant: Instant::now(),
            cached_snapshot: OverlayDiagnosticsSnapshot::default(),
            log_every_windows: diagnostics_log_every(),
            elapsed_windows: 0,
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

    pub fn snapshot(&mut self, annotation: AnnotationRasterDiagnostics) -> OverlayDiagnosticsSnapshot {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_instant).as_secs_f64();
        if elapsed < 0.2 {
            let mut snapshot = self.cached_snapshot.clone();
            snapshot.annotation_committed_rebuilds = annotation.committed_rebuilds;
            snapshot.annotation_composed_rebuilds = annotation.composed_rebuilds;
            snapshot.annotation_interaction_base_rebuilds = annotation.interaction_base_rebuilds;
            snapshot.annotation_drawing_fast_path_hits = annotation.drawing_fast_path_hits;
            snapshot.annotation_moving_fast_path_hits = annotation.moving_fast_path_hits;
            return snapshot;
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
            annotation_committed_rebuilds: annotation.committed_rebuilds,
            annotation_composed_rebuilds: annotation.composed_rebuilds,
            annotation_interaction_base_rebuilds: annotation.interaction_base_rebuilds,
            annotation_drawing_fast_path_hits: annotation.drawing_fast_path_hits,
            annotation_moving_fast_path_hits: annotation.moving_fast_path_hits,
        };

        self.last_counters = self.counters;
        self.last_instant = now;
        self.elapsed_windows = self.elapsed_windows.saturating_add(1);
        self.maybe_log_snapshot();
        self.cached_snapshot.clone()
    }

    fn maybe_log_snapshot(&self) {
        if self.log_every_windows == 0 {
            return;
        }
        if !self.elapsed_windows.is_multiple_of(self.log_every_windows) {
            return;
        }
        tracing::info!(
            target: "overlay::diagnostics",
            pointer_hz = self.cached_snapshot.pointer_hz,
            render_hz = self.cached_snapshot.render_hz,
            refresh_hz = self.cached_snapshot.refresh_hz,
            applied_ratio = self.cached_snapshot.apply_ratio,
            coalesced_ratio = self.cached_snapshot.coalesced_ratio,
            raster_committed_rebuilds = self.cached_snapshot.annotation_committed_rebuilds,
            raster_composed_rebuilds = self.cached_snapshot.annotation_composed_rebuilds,
            raster_interaction_base_rebuilds = self.cached_snapshot.annotation_interaction_base_rebuilds,
            raster_drawing_fast_hits = self.cached_snapshot.annotation_drawing_fast_path_hits,
            raster_moving_fast_hits = self.cached_snapshot.annotation_moving_fast_path_hits,
            "overlay diagnostics snapshot"
        );
    }
}

fn diagnostics_log_every() -> u64 {
    std::env::var(DIAG_LOG_EVERY_ENV)
        .ok()
        .as_deref()
        .and_then(parse_positive_u64)
        .unwrap_or(0)
}

fn parse_positive_u64(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok().filter(|next| *next > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn annotation_stats(committed: u64, composed: u64, base: u64, drawing_fast: u64, moving_fast: u64) -> AnnotationRasterDiagnostics {
        AnnotationRasterDiagnostics {
            committed_rebuilds: committed,
            composed_rebuilds: composed,
            interaction_base_rebuilds: base,
            drawing_fast_path_hits: drawing_fast,
            moving_fast_path_hits: moving_fast,
        }
    }

    #[test]
    fn snapshot_carries_annotation_stats_on_regular_update() {
        let mut diagnostics = OverlayDiagnostics {
            last_instant: Instant::now() - Duration::from_millis(250),
            ..OverlayDiagnostics::default()
        };
        diagnostics.on_render();
        diagnostics.on_refresh();

        let snapshot = diagnostics.snapshot(annotation_stats(3, 9, 2, 7, 5));
        assert_eq!(snapshot.annotation_committed_rebuilds, 3);
        assert_eq!(snapshot.annotation_composed_rebuilds, 9);
        assert_eq!(snapshot.annotation_interaction_base_rebuilds, 2);
        assert_eq!(snapshot.annotation_drawing_fast_path_hits, 7);
        assert_eq!(snapshot.annotation_moving_fast_path_hits, 5);
    }

    #[test]
    fn snapshot_carries_annotation_stats_on_cached_path() {
        let mut diagnostics = OverlayDiagnostics {
            last_instant: Instant::now(),
            cached_snapshot: OverlayDiagnosticsSnapshot {
                pointer_hz: 60.0,
                ..OverlayDiagnosticsSnapshot::default()
            },
            ..OverlayDiagnostics::default()
        };

        let snapshot = diagnostics.snapshot(annotation_stats(11, 21, 4, 13, 8));
        assert_eq!(snapshot.pointer_hz, 60.0);
        assert_eq!(snapshot.annotation_committed_rebuilds, 11);
        assert_eq!(snapshot.annotation_composed_rebuilds, 21);
        assert_eq!(snapshot.annotation_interaction_base_rebuilds, 4);
        assert_eq!(snapshot.annotation_drawing_fast_path_hits, 13);
        assert_eq!(snapshot.annotation_moving_fast_path_hits, 8);
    }

    #[test]
    fn parse_positive_u64_accepts_positive_values() {
        assert_eq!(parse_positive_u64("1"), Some(1));
        assert_eq!(parse_positive_u64(" 42 "), Some(42));
    }

    #[test]
    fn parse_positive_u64_rejects_invalid_values() {
        assert_eq!(parse_positive_u64("0"), None);
        assert_eq!(parse_positive_u64("-1"), None);
        assert_eq!(parse_positive_u64("abc"), None);
        assert_eq!(parse_positive_u64(""), None);
    }
}

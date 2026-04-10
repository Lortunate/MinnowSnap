use crate::capture::stitcher::{ScrollStitcher, StitchFrameStatus};
use crate::capture::{active_monitor, perform_crop};
use crate::geometry::{Rect, RectF};
use image::RgbaImage;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

const SCALE_EPSILON: f32 = 0.01;
const CAPTURE_LOOP_INTERVAL: Duration = Duration::from_millis(16);
const PREVIEW_EVENT_INTERVAL: Duration = Duration::from_millis(33);

#[derive(Debug)]
pub enum LongCaptureEvent {
    Started,
    Progress { height: i32, preview_image: RgbaImage },
    Warning { text: String },
    Finished { final_image: Option<RgbaImage> },
}

#[derive(Clone)]
pub struct LongCaptureRuntime {
    active: Arc<AtomicBool>,
    events_tx: mpsc::Sender<LongCaptureEvent>,
    events_rx: Arc<Mutex<mpsc::Receiver<LongCaptureEvent>>>,
    final_image: Arc<Mutex<Option<RgbaImage>>>,
}

impl Default for LongCaptureRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl LongCaptureRuntime {
    pub fn new() -> Self {
        let (events_tx, events_rx) = mpsc::channel();
        Self {
            active: Arc::new(AtomicBool::new(false)),
            events_tx,
            events_rx: Arc::new(Mutex::new(events_rx)),
            final_image: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start(&self, rect: Rect, scale: f32) {
        self.start_with_viewport(rect, RectF::new(0.0, 0.0, 0.0, 0.0), scale);
    }

    pub fn start_with_viewport(&self, rect: Rect, viewport_rect: RectF, scale_hint: f32) {
        self.stop();
        self.clear_pending_events();
        if let Ok(mut final_image) = self.final_image.lock() {
            *final_image = None;
        }
        self.active.store(true, Ordering::SeqCst);

        let active = self.active.clone();
        let tx = self.events_tx.clone();
        let final_image = self.final_image.clone();

        crate::RUNTIME.spawn_blocking(move || {
            let _ = tx.send(LongCaptureEvent::Started);

            let Some(monitor) = active_monitor() else {
                let _ = tx.send(LongCaptureEvent::Warning {
                    text: "No active monitor found for long capture".to_string(),
                });
                let _ = tx.send(LongCaptureEvent::Finished { final_image: None });
                return;
            };

            let mut stitcher = ScrollStitcher::new();
            let mut low_confidence_streak = 0usize;
            let mut warned = false;
            let mut preview_emitted = false;
            let mut last_preview_emit = Instant::now();

            while active.load(Ordering::SeqCst) {
                match monitor.capture_image() {
                    Ok(full_screen) => {
                        if let Some(cropped) = crop_frame_with_scale_candidates(&full_screen, rect, &viewport_rect, scale_hint) {
                            let result = stitcher.process_frame_detailed(cropped);
                            match result.status {
                                StitchFrameStatus::Appended => {
                                    low_confidence_streak = 0;
                                    if warned {
                                        warned = false;
                                        let _ = tx.send(LongCaptureEvent::Warning { text: String::new() });
                                    }
                                    let should_emit_preview = !preview_emitted || last_preview_emit.elapsed() >= PREVIEW_EVENT_INTERVAL;
                                    if should_emit_preview {
                                        if let Some(thumbnail) = stitcher.make_thumbnail(500) {
                                            let _ = tx.send(LongCaptureEvent::Progress {
                                                height: result.height,
                                                preview_image: thumbnail,
                                            });
                                        }
                                        preview_emitted = true;
                                        last_preview_emit = Instant::now();
                                    }
                                }
                                StitchFrameStatus::Stationary => {
                                    low_confidence_streak = 0;
                                }
                                StitchFrameStatus::LowConfidence | StitchFrameStatus::Reverse => {
                                    low_confidence_streak += 1;
                                    if low_confidence_streak >= 6 {
                                        warned = true;
                                        let _ = tx.send(LongCaptureEvent::Warning {
                                            text: result
                                                .warning
                                                .unwrap_or_else(|| "Long capture is unstable; try smoother scrolling".to_string()),
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        let _ = tx.send(LongCaptureEvent::Warning {
                            text: "Failed to capture screen frame".to_string(),
                        });
                    }
                }

                std::thread::sleep(CAPTURE_LOOP_INTERVAL);
            }

            let final_img = stitcher.get_final_image();
            if let Ok(mut slot) = final_image.lock() {
                *slot = final_img.clone();
            }
            let _ = tx.send(LongCaptureEvent::Finished { final_image: final_img });
        });
    }

    pub fn stop(&self) {
        self.active.store(false, Ordering::SeqCst);
    }

    pub fn stop_and_take_result(&self, timeout: Duration) -> Option<RgbaImage> {
        self.stop();

        if let Some(image) = self.take_result() {
            return Some(image);
        }

        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(image) = self.take_result() {
                return Some(image);
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let wait = remaining.min(CAPTURE_LOOP_INTERVAL);

            let recv_result = {
                let Ok(rx) = self.events_rx.lock() else {
                    return None;
                };
                rx.recv_timeout(wait)
            };

            match recv_result {
                Ok(LongCaptureEvent::Finished { final_image }) => {
                    if let Some(image) = final_image {
                        if let Ok(mut slot) = self.final_image.lock() {
                            *slot = Some(image.clone());
                        }
                        return Some(image);
                    }
                }
                Ok(_) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        self.take_result()
    }

    pub fn take_result(&self) -> Option<RgbaImage> {
        self.final_image.lock().ok().and_then(|mut slot| slot.take())
    }

    pub fn drain_events(&self) -> Vec<LongCaptureEvent> {
        let mut events = Vec::new();
        let Ok(rx) = self.events_rx.lock() else {
            return events;
        };

        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        events
    }

    fn clear_pending_events(&self) {
        let Ok(rx) = self.events_rx.lock() else {
            return;
        };

        while rx.try_recv().is_ok() {}
    }
}

fn crop_frame_with_scale_candidates(full_screen: &RgbaImage, rect: Rect, viewport_rect: &RectF, scale_hint: f32) -> Option<RgbaImage> {
    let candidates = build_scale_candidates(full_screen.width(), full_screen.height(), viewport_rect, scale_hint);
    for scale in candidates {
        if let Some(mut cropped) = perform_crop(full_screen, rect, scale) {
            normalize_alpha_opaque(&mut cropped);
            return Some(cropped);
        }
    }
    None
}

fn build_scale_candidates(frame_width: u32, frame_height: u32, viewport_rect: &RectF, scale_hint: f32) -> Vec<f32> {
    let mut candidates = Vec::with_capacity(5);

    let inferred_x = if viewport_rect.width > 1.0 {
        Some(frame_width as f32 / viewport_rect.width as f32)
    } else {
        None
    };
    let inferred_y = if viewport_rect.height > 1.0 {
        Some(frame_height as f32 / viewport_rect.height as f32)
    } else {
        None
    };

    push_scale_candidate(&mut candidates, inferred_x);
    push_scale_candidate(&mut candidates, inferred_y);
    if let (Some(scale_x), Some(scale_y)) = (inferred_x, inferred_y) {
        push_scale_candidate(&mut candidates, Some((scale_x + scale_y) * 0.5));
    }

    push_scale_candidate(&mut candidates, Some(scale_hint));
    push_scale_candidate(&mut candidates, Some(1.0));

    if candidates.is_empty() {
        candidates.push(1.0);
    }

    candidates
}

fn push_scale_candidate(candidates: &mut Vec<f32>, scale: Option<f32>) {
    let Some(scale) = scale else {
        return;
    };
    if !scale.is_finite() || scale <= SCALE_EPSILON {
        return;
    }
    if candidates.iter().any(|candidate| (*candidate - scale).abs() <= SCALE_EPSILON) {
        return;
    }
    candidates.push(scale);
}

fn normalize_alpha_opaque(image: &mut RgbaImage) {
    for pixel in image.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn approx_eq(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.0001
    }

    #[test]
    fn scale_candidates_prioritize_inferred_scale() {
        let viewport = RectF::new(0.0, 0.0, 960.0, 540.0);
        let candidates = build_scale_candidates(1920, 1080, &viewport, 1.0);
        assert!(approx_eq(candidates[0], 2.0));
        assert!(candidates.iter().any(|value| approx_eq(*value, 1.0)));
    }

    #[test]
    fn crop_prefers_inferred_scale_when_hint_is_unreliable() {
        let source = RgbaImage::from_pixel(1920, 1080, Rgba([10, 20, 30, 255]));
        let rect = Rect {
            x: 10,
            y: 20,
            width: 200,
            height: 100,
        };
        let viewport = RectF::new(0.0, 0.0, 960.0, 540.0);

        let cropped = crop_frame_with_scale_candidates(&source, rect, &viewport, 1.0).expect("crop should succeed");

        assert_eq!(cropped.width(), 400);
        assert_eq!(cropped.height(), 200);
    }

    #[test]
    fn cropped_frames_are_forced_opaque() {
        let source = RgbaImage::from_pixel(400, 300, Rgba([25, 50, 75, 0]));
        let rect = Rect {
            x: 0,
            y: 0,
            width: 200,
            height: 100,
        };
        let viewport = RectF::new(0.0, 0.0, 400.0, 300.0);

        let cropped = crop_frame_with_scale_candidates(&source, rect, &viewport, 1.0).expect("crop should succeed");
        assert!(cropped.pixels().all(|pixel| pixel[3] == 255));
    }
}

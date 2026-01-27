use crate::core::capture::stitcher::{ScrollStitcher, StitchResult};
use crate::core::capture::{get_primary_monitor, get_primary_monitor_scale, perform_crop};
use image::RgbaImage;
use log::{error, info};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub trait ScrollObserver: Send + 'static {
    fn on_update(&self, height: i32, thumbnail: RgbaImage);
    fn on_warning(&self, message: String);
    fn on_finished(&self, final_image: Option<RgbaImage>);
}

pub fn start_scroll_capture_thread(x: i32, y: i32, width: i32, height: i32, active_flag: Arc<AtomicBool>, observer: Box<dyn ScrollObserver>) {
    let scale_factor = get_primary_monitor_scale();

    crate::core::RUNTIME.spawn_blocking(move || {
        info!("Scroll capture thread started");
        // Small delay to let UI hide if needed
        thread::sleep(Duration::from_millis(250));

        let Some(monitor) = get_primary_monitor() else {
            error!("No primary monitor found for scroll capture");
            observer.on_finished(None);
            return;
        };

        let mut consecutive_failures = 0;
        let mut stitcher = ScrollStitcher::new();

        while active_flag.load(Ordering::SeqCst) {
            if let Ok(full_screen) = monitor.capture_image() {
                if let Some(cropped) = perform_crop(&full_screen, x, y, width, height, scale_factor) {
                    let result = stitcher.process_frame(cropped);

                    match result {
                        StitchResult::Success => {
                            consecutive_failures = 0;
                            let real_h = stitcher.current_image().map_or(0, |(_, h)| h as i32);
                            if let Some(thumbnail) = stitcher.make_thumbnail(500) {
                                observer.on_update(real_h, thumbnail);
                            }
                        }
                        StitchResult::Stationary => {
                            consecutive_failures = 0;
                        }
                        StitchResult::Failure => {
                            consecutive_failures += 1;
                            if consecutive_failures == 15 {
                                observer.on_warning("Scroll slower...".to_string());
                            }
                        }
                    }
                }
            } else {
                error!("Capture image failed");
            }
            thread::sleep(Duration::from_millis(20));
        }

        info!("Scroll capture loop exited");
        let final_img = stitcher.get_final_image();
        observer.on_finished(final_img);
    });
}

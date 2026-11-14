mod assets;
mod background;
mod desktop;
mod logging;
mod shutdown;
mod tray;

use anyhow::Context;
use desktop::{DesktopHost, DesktopShutter};
use gpui::{App, Global, Subscription, Task};
use minnow_application::ports::{NotificationKind, NotificationRequest};
use minnow_application::{ApplicationDependencies, ApplicationServices};
use minnow_platform::{AppPaths, DataLayout, DesktopNotifications, DesktopOutput, NativeCapture};
use minnow_recognition::{LocalQrDecoder, RecognitionOptions, RecognitionService};
use minnow_settings::{SettingsHandle, SettingsStore};
use minnow_ui::i18n;
use shutdown::{ShutdownRequest, ShutdownTrigger};
use single_instance::SingleInstance;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
    time::Duration,
};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::mpsc;
use tray::SystemTray;

const APP_ID: &str = "com.lortunate.minnowsnap";
const APP_NAME: &str = "MinnowSnap";
const APP_LOCK_ID: &str = "com.lortunate.minnowsnap.lock";
const DRAIN_TIMEOUT: Duration = Duration::from_secs(10);

enum HostEvent {
    Capture,
    QuickCapture,
    Preferences,
}

/// Retained GPUI subscriptions and tasks are cancelled when the host is released.
/// Core services are explicitly injected and do not depend on this GPUI global.
struct HostLifetime {
    _events: Task<()>,
    _shutdown: Task<()>,
    _quit: Subscription,
    _quick_capture: Rc<RefCell<Option<Task<()>>>>,
}

impl Global for HostLifetime {}

/// These services remain owned together until process shutdown has drained them.
#[derive(Clone)]
struct BackgroundServices {
    application: Arc<ApplicationServices>,
    recognition: Arc<RecognitionService>,
    settings: SettingsHandle,
}

fn build_runtime() -> anyhow::Result<Runtime> {
    let cpus = std::thread::available_parallelism().map(usize::from).unwrap_or(4);
    Builder::new_multi_thread()
        .worker_threads(cpus.clamp(2, 8))
        .max_blocking_threads(32)
        .enable_all()
        .thread_name("minnow-app-rt")
        .build()
        .context("failed to create application runtime")
}

pub(crate) fn run() -> anyhow::Result<()> {
    let executable = std::env::current_exe()?;
    #[cfg(feature = "portable")]
    let layout = DataLayout::Portable {
        executable: executable.clone(),
    };
    #[cfg(not(feature = "portable"))]
    let layout = DataLayout::Installed;
    let paths = AppPaths::resolve("com", "lortunate", APP_NAME, layout)?;
    let _log = logging::init_logger(paths.logs_dir());
    tracing::info!("Starting MinnowSnap...");

    #[cfg(target_os = "macos")]
    let instance_id = {
        std::fs::create_dir_all(paths.temp_dir())?;
        paths.temp_file(APP_LOCK_ID).to_string_lossy().into_owned()
    };
    #[cfg(not(target_os = "macos"))]
    let instance_id = APP_LOCK_ID.to_owned();
    let instance = SingleInstance::new(&instance_id).context("failed to acquire application instance lock")?;
    if !instance.is_single() {
        tracing::info!("Another instance is running, exiting.");
        return Ok(());
    }

    let runtime = build_runtime()?;
    // GPUI futures can poll Tokio timers on the main thread while the runtime's
    // worker pool remains owned here, outside GPUI's event loop.
    let entered = runtime.enter();
    let request = ShutdownRequest::default();
    if let Err(error) = request.install_signal_handler() {
        tracing::warn!(%error, "Failed to install process signal handler");
    }
    #[cfg(target_os = "windows")]
    let control_server = runtime.spawn(shutdown::control_pipe::serve(request.clone()));

    let settings = match SettingsStore::open(paths.config_file()) {
        Ok(settings) => settings,
        Err(error) => {
            tracing::error!(%error, "Settings could not be loaded; using non-persistent defaults and preserving the original file");
            SettingsStore::degraded(Default::default(), error)
        }
    };
    let model_type = settings.snapshot().ocr.model_type;
    let recognition = Arc::new(RecognitionService::new(
        paths.ocr_models_dir(),
        runtime.handle().clone(),
        RecognitionOptions {
            model_type,
            ..Default::default()
        },
    )?);
    let output_directory = minnow_platform::export::user_picture_directory(APP_NAME).unwrap_or_else(|error| {
        // Explicit output directories and non-export features remain available.
        tracing::warn!(%error, "Default screenshot directory is unavailable");
        Default::default()
    });
    let notifications = Arc::new(DesktopNotifications::new(
        APP_ID,
        APP_NAME,
        Some(paths.temp_file("minnowsnap-toast-icon.png")),
    ));
    let application = Arc::new(ApplicationServices::new(ApplicationDependencies {
        settings: settings.clone(),
        capture: Arc::new(NativeCapture),
        text_recognition: recognition.clone(),
        qr_decoder: Arc::new(LocalQrDecoder::new()),
        recognition_models: if cfg!(feature = "local-ocr") { Some(recognition.clone()) } else { None },
        output: Arc::new(DesktopOutput::new(output_directory)),
        notifications: notifications.clone(),
        shutter_sound: Arc::new(DesktopShutter),
        runtime: runtime.handle().clone(),
    }));

    let services = BackgroundServices {
        application,
        recognition,
        settings,
    };
    let application_ui = services.application.clone();
    let services_ui = services.clone();
    let request_ui = request.clone();
    let app = gpui_platform::application()
        .with_assets(assets::AppAssets)
        .with_quit_mode(gpui::QuitMode::Explicit);
    // Let GPUI create its NSApplication subclass; Headless selects accessory
    // activation on macOS while preserving native window creation.
    #[cfg(target_os = "macos")]
    let app = app.with_windowing(gpui::WindowingRequest::Headless);
    app.run(move |cx| {
        gpui_component::init(cx);
        // Notification registration can alter NSBundle identifiers. GPUI must
        // inspect the unbundled process before that integration is installed.
        if let Err(error) = notifications.register(Some(assets::LOGO_BYTES)) {
            tracing::warn!(%error, "Desktop notifications are unavailable");
        }
        let host = Rc::new(DesktopHost::new(&paths, &executable));
        minnow_ui::install(cx, application_ui.clone(), host.clone(), host.clone());
        for failure in application_ui.preferences().activate_saved(host.as_ref()) {
            tracing::warn!(?failure, "Saved native preference is not fully effective");
        }

        let (events_tx, events_rx) = mpsc::channel(32);
        if let Some(registry) = host.hotkeys.borrow().as_ref() {
            let router = registry.router();
            let events = events_tx.clone();
            // global-hotkey accepts one process-wide handler. Its owned router
            // becomes empty when the registry is released during shutdown.
            global_hotkey::GlobalHotKeyEvent::set_event_handler(Some(move |event| {
                if let Some(action) = router.action_for_event(&event) {
                    use minnow_platform::hotkeys::ShortcutAction;
                    let event = match action {
                        ShortcutAction::Capture => HostEvent::Capture,
                        ShortcutAction::QuickCapture => HostEvent::QuickCapture,
                    };
                    let _ = events.try_send(event);
                }
            }));
        }
        let tray = match SystemTray::new(events_tx, request_ui.clone()) {
            Ok(tray) => Some(tray),
            Err(error) => {
                tracing::error!(%error, "Failed to install system tray");
                request_ui.request(ShutdownTrigger::Tray);
                None
            }
        };
        if let Err(error) = background::install(cx) {
            tracing::error!(%error, "Failed to install background host window");
            request_ui.request(ShutdownTrigger::Tray);
        }
        install_lifetime(cx, services_ui, host, tray, request_ui, events_rx);
    });

    // Also covers native quit and initialization failure, where GPUI may stop
    // polling a foreground drain future before its work has completed.
    request.request(ShutdownTrigger::Signal);
    let drained = runtime.block_on(drain(&services, &request));
    #[cfg(target_os = "windows")]
    runtime.block_on(async {
        let _ = control_server.await;
    });
    drop(entered);
    runtime.shutdown_timeout(Duration::from_secs(2));
    drop(instance);
    anyhow::ensure!(drained, "application shutdown did not complete all work");
    Ok(())
}

fn install_lifetime(
    cx: &mut App,
    services: BackgroundServices,
    host: Rc<DesktopHost>,
    tray: Option<SystemTray>,
    request: ShutdownRequest,
    mut events: mpsc::Receiver<HostEvent>,
) {
    let tray = Rc::new(RefCell::new(tray));
    let event_request = request.clone();
    let event_application = services.application.clone();
    let quick_capture = Rc::new(RefCell::new(None::<Task<()>>));
    let event_quick_capture = quick_capture.clone();
    let quick_busy = Rc::new(Cell::new(false));
    let event_task = cx.spawn(async move |cx| {
        while let Some(event) = tokio::select! {
            _ = event_request.cancelled() => None,
            event = events.recv() => event,
        } {
            if event_request.is_requested() {
                break;
            }
            match event {
                HostEvent::Capture => cx.update(minnow_ui::open_capture),
                HostEvent::Preferences => cx.update(minnow_ui::open_preferences),
                HostEvent::QuickCapture => {
                    if quick_busy.replace(true) {
                        continue;
                    }
                    let application = event_application.clone();
                    let busy = quick_busy.clone();
                    let task = cx.spawn(async move |_| {
                        let notification = match application.quick_capture().await {
                            Ok(()) => {
                                let _sound = application.play_shutter();
                                NotificationRequest {
                                    title: i18n::app::capture_name(),
                                    message: i18n::notify::quick_capture_copied(),
                                    kind: NotificationKind::Copy,
                                }
                            }
                            Err(error) => {
                                tracing::error!(%error, "Quick capture failed");
                                NotificationRequest {
                                    title: i18n::app::name(),
                                    message: i18n::notify::quick_capture_failed(),
                                    kind: NotificationKind::Info,
                                }
                            }
                        };
                        let _ = application.notify(notification).await;
                        busy.set(false);
                    });
                    *event_quick_capture.borrow_mut() = Some(task);
                }
            }
        }
    });
    let quit_request = request.clone();
    let quit_host = host.clone();
    let quit_tray = tray.clone();
    let quit_services = services.clone();
    let quit_quick_capture = quick_capture.clone();
    let quit = cx.on_app_quit(move |_| {
        quit_request.request(ShutdownTrigger::Signal);
        quit_quick_capture.borrow_mut().take();
        quit_host.release_hotkeys();
        quit_tray.borrow_mut().take();
        let services = quit_services.clone();
        let request = quit_request.clone();
        async move {
            drain(&services, &request).await;
        }
    });
    let shutdown_quick_capture = quick_capture.clone();
    let shutdown_task = cx.spawn(async move |cx| {
        request.cancelled().await;
        cx.update(|cx| {
            shutdown_quick_capture.borrow_mut().take();
            minnow_ui::close_all(cx);
            host.release_hotkeys();
            tray.borrow_mut().take();
        });
        drain(&services, &request).await;
        cx.update(|cx| cx.quit());
    });
    cx.set_global(HostLifetime {
        _events: event_task,
        _shutdown: shutdown_task,
        _quit: quit,
        _quick_capture: quick_capture,
    });
}

async fn drain(services: &BackgroundServices, request: &ShutdownRequest) -> bool {
    let BackgroundServices {
        application,
        recognition,
        settings,
    } = services;
    if let Some(completed) = request.completion() {
        return completed;
    }
    let budget = request.remaining(DRAIN_TIMEOUT);
    // Close settings admission immediately. A zero-budget call queues the final
    // write without waiting; its writer remains independent of the blocking pool.
    let _ = settings.shutdown(Duration::ZERO);
    let settings = settings.clone();
    let (jobs, recognition_result, persisted) = tokio::join!(
        tokio::time::timeout(budget, application.shutdown_with_timeout(budget)),
        tokio::time::timeout(budget, recognition.shutdown()),
        tokio::time::timeout(budget, tokio::task::spawn_blocking(move || settings.shutdown(budget))),
    );
    let jobs_drained = matches!(&jobs, Ok(report) if report.drained);
    if !jobs_drained {
        tracing::error!(result = ?jobs, "Application tasks did not finish before shutdown deadline");
    }
    let recognition_drained = matches!(recognition_result, Ok(Ok(())));
    if !recognition_drained {
        tracing::error!(result = ?recognition_result, "Recognition work did not finish cleanly");
    }
    let persistence_ok = matches!(persisted, Ok(Ok(Ok(_))));
    if !persistence_ok {
        tracing::error!(result = ?persisted, "Settings persistence failed during shutdown");
    }
    let drained = jobs_drained && recognition_drained && persistence_ok;
    request.complete(drained);
    drained
}

pub(crate) fn shutdown_running_instance() -> u8 {
    #[cfg(target_os = "windows")]
    {
        let runtime = match build_runtime() {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("{error}");
                return 3;
            }
        };
        match runtime.block_on(shutdown::control_pipe::request_shutdown()) {
            Ok(()) => 0,
            Err(shutdown::control_pipe::ClientError::NotRunning) => 2,
            Err(error) => {
                eprintln!("Failed to request graceful shutdown: {error}");
                3
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("Shutdown command is only supported on Windows.");
        3
    }
}

#[allow(unused_imports)]
use minnow_app::{
    app::{self, bootstrap as _, composition as _, runtime as _},
    platform, services, ui,
};
use gpui::AssetSource;
use std::process::ExitCode;
use std::str::FromStr;

#[test]
fn app_public_api_matches_task1_surface() {
    let command = app::parse_command_from(std::iter::empty::<String>()).expect("command");
    assert_eq!(command, app::Command::Run);

    let _parse_command: fn() -> Result<app::Command, String> = app::parse_command;
    let _run_command: fn(app::Command) -> ExitCode = app::run_command;
    let _runtime_run: fn() = app::runtime::run;
    let _runtime_shutdown: fn() -> u8 = app::runtime::shutdown_running_instance;
}

#[test]
fn top_level_modules_export_stub_surface() {
    assert_eq!(platform::module_tag, "platform");
    assert_eq!(services::module_tag, "services");
    assert_eq!(ui::module_tag, "ui");
}

#[test]
fn ui_and_platform_modules_export_task4_runtime_surface() {
    fn open_app(_: &mut gpui::App) {}
    fn run_no_app() {}
    fn set_auto_start(_: bool) {}

    let _overlay_open: fn(&mut gpui::App) = ui::features::overlay::open_window;
    let _overlay_bind: fn(&mut gpui::App) = ui::features::overlay::bind_keys;
    let _overlay_handle_new: fn(&mut gpui::App) -> ui::features::overlay::OverlayHandle =
        ui::features::overlay::OverlayHandle::new;
    let _pin_bind: fn(&mut gpui::App) = ui::features::pin::bind_keys;
    let _pin_install: fn(&mut gpui::App) = ui::features::pin::install;
    let _preferences_open: fn(&mut gpui::App) = ui::features::preferences::open_window;
    let _locale_apply: fn(&str) -> String = ui::support::locale::apply;
    let _appearance_apply: fn(Option<&mut gpui::Window>, &mut gpui::App) =
        ui::support::appearance::apply_saved_preferences;

    let _ = platform::hotkey::HotkeyActionSink::new(open_app, run_no_app);
    let _ = platform::tray::TrayActions::new(open_app, run_no_app, open_app);
    let _ = platform::system::UiSystemActions::new(set_auto_start);
    let _hotkey_install = platform::hotkey::install_hotkey_service;
    let _tray_install = platform::tray::SystemTray::install;
    let _background_install = platform::background_host::install;
    let _system_install = platform::system::install_ui_system_actions::<fn(bool)>;
    let _notification_type = platform::notify::NotificationType::Info;
    let _shutdown_trigger = platform::shutdown::ShutdownTrigger::TrayMenu;
    let _copy_text: fn(String) -> bool = platform::io::clipboard::copy_text_to_clipboard;
    let _fonts: fn() -> Vec<String> = platform::io::fonts::get_system_fonts;
    let _storage: fn(&image::RgbaImage, bool) -> Option<String> = platform::io::storage::save_temp_image;
}

#[test]
fn composition_runtime_callback_wiring_matches_expected_actions() {
    let hotkey = app::composition::hotkey_callback_bindings();
    assert!(std::ptr::fn_addr_eq(
        hotkey.open_capture_overlay,
        app::composition::open_capture_overlay as fn(&mut gpui::App)
    ));
    assert!(std::ptr::fn_addr_eq(
        hotkey.run_quick_capture,
        app::composition::run_quick_capture_with_notification as fn()
    ));

    let tray = app::composition::tray_callback_bindings();
    assert!(std::ptr::fn_addr_eq(
        tray.open_capture_overlay,
        app::composition::open_capture_overlay as fn(&mut gpui::App)
    ));
    assert!(std::ptr::fn_addr_eq(
        tray.run_quick_capture,
        app::composition::run_quick_capture_with_notification as fn()
    ));
    assert!(std::ptr::fn_addr_eq(
        tray.open_preferences,
        app::composition::open_preferences_window as fn(&mut gpui::App)
    ));
}

#[test]
fn services_own_assets_and_paths_surface() {
    use minnow_app::services::{
        assets::{asset_bytes, asset_paths, AppAssets},
        paths,
    };

    assert_eq!(asset_paths::LOGO_PATH, "resources/logo.png");
    assert!(!asset_bytes::LOGO_BYTES.is_empty());
    assert!(!asset_bytes::CAPTURE_SOUND_BYTES.is_empty());

    let assets = AppAssets;
    let logo = assets
        .load(asset_paths::LOGO_PATH)
        .expect("logo load")
        .expect("logo present");
    assert!(!logo.is_empty());

    let list = assets.list("resources/icons/").expect("icon list");
    assert!(list.iter().any(|path| path.as_ref() == asset_paths::icons::CLOSE));

    let paths = paths::app_paths();
    assert_eq!(paths.temp_file("x"), paths.temp_dir().join("x"));
}

#[test]
fn services_assets_constants_stay_in_parity_with_legacy_crate() {
    use minnow_app::services::assets::{asset_bytes, asset_paths};

    assert_eq!(asset_paths::LOGO_PATH, minnow_assets::asset_paths::LOGO_PATH);
    assert_eq!(asset_paths::icons::ADD, minnow_assets::asset_paths::icons::ADD);
    assert_eq!(
        asset_paths::icons::ARROW_DROP_DOWN,
        minnow_assets::asset_paths::icons::ARROW_DROP_DOWN
    );
    assert_eq!(
        asset_paths::icons::ARROW_DROP_UP,
        minnow_assets::asset_paths::icons::ARROW_DROP_UP
    );
    assert_eq!(
        asset_paths::icons::ARROW_INSERT,
        minnow_assets::asset_paths::icons::ARROW_INSERT
    );
    assert_eq!(asset_paths::icons::BLUR_ON, minnow_assets::asset_paths::icons::BLUR_ON);
    assert_eq!(asset_paths::icons::CIRCLE, minnow_assets::asset_paths::icons::CIRCLE);
    assert_eq!(asset_paths::icons::CLOSE, minnow_assets::asset_paths::icons::CLOSE);
    assert_eq!(
        asset_paths::icons::COUNTER_1,
        minnow_assets::asset_paths::icons::COUNTER_1
    );
    assert_eq!(
        asset_paths::icons::CROP_FREE,
        minnow_assets::asset_paths::icons::CROP_FREE
    );
    assert_eq!(
        asset_paths::icons::FILE_COPY,
        minnow_assets::asset_paths::icons::FILE_COPY
    );
    assert_eq!(asset_paths::icons::GRID_ON, minnow_assets::asset_paths::icons::GRID_ON);
    assert_eq!(asset_paths::icons::KEEP, minnow_assets::asset_paths::icons::KEEP);
    assert_eq!(
        asset_paths::icons::LENS_BLUR,
        minnow_assets::asset_paths::icons::LENS_BLUR
    );
    assert_eq!(asset_paths::icons::REDO, minnow_assets::asset_paths::icons::REDO);
    assert_eq!(asset_paths::icons::SAVE, minnow_assets::asset_paths::icons::SAVE);
    assert_eq!(asset_paths::icons::SCROLL, minnow_assets::asset_paths::icons::SCROLL);
    assert_eq!(asset_paths::icons::SQUARE, minnow_assets::asset_paths::icons::SQUARE);
    assert_eq!(
        asset_paths::icons::SQUARE_FILL,
        minnow_assets::asset_paths::icons::SQUARE_FILL
    );
    assert_eq!(
        asset_paths::icons::TEXT_FIELDS,
        minnow_assets::asset_paths::icons::TEXT_FIELDS
    );
    assert_eq!(asset_paths::icons::UNDO, minnow_assets::asset_paths::icons::UNDO);

    assert_eq!(asset_bytes::LOGO_BYTES, minnow_assets::asset_bytes::LOGO_BYTES);
    assert_eq!(
        asset_bytes::CAPTURE_SOUND_BYTES,
        minnow_assets::asset_bytes::CAPTURE_SOUND_BYTES
    );
}

#[test]
fn services_app_assets_behavior_stays_in_parity_with_legacy_crate() {
    use minnow_app::services::assets::{asset_paths, AppAssets as NewAssets};

    let new_assets = NewAssets;
    let old_assets = minnow_assets::AppAssets;

    assert_eq!(
        new_assets.load(asset_paths::LOGO_PATH).expect("new logo load"),
        old_assets
            .load(minnow_assets::asset_paths::LOGO_PATH)
            .expect("old logo load")
    );
    assert_eq!(
        new_assets
            .load(asset_paths::icons::CLOSE)
            .expect("new close icon load"),
        old_assets
            .load(minnow_assets::asset_paths::icons::CLOSE)
            .expect("old close icon load")
    );
    assert_eq!(new_assets.load("").expect("new empty path"), old_assets.load("").expect("old empty path"));
    let new_missing = new_assets
        .load("resources/missing.file")
        .map(|asset| asset.map(|bytes| bytes.into_owned()));
    let old_missing = old_assets
        .load("resources/missing.file")
        .map(|asset| asset.map(|bytes| bytes.into_owned()));
    match (new_missing, old_missing) {
        (Ok(new_value), Ok(old_value)) => assert_eq!(new_value, old_value),
        (Err(new_err), Err(old_err)) => assert_eq!(new_err.to_string(), old_err.to_string()),
        (new_outcome, old_outcome) => {
            panic!("missing asset parity mismatch: new={new_outcome:?} old={old_outcome:?}");
        }
    }

    let mut new_list: Vec<String> = new_assets
        .list("resources/icons/")
        .expect("new icon list")
        .into_iter()
        .map(|item: gpui::SharedString| item.to_string())
        .collect();
    let mut old_list: Vec<String> = old_assets
        .list("resources/icons/")
        .expect("old icon list")
        .into_iter()
        .map(|item: gpui::SharedString| item.to_string())
        .collect();
    new_list.sort();
    old_list.sort();
    assert_eq!(new_list, old_list);
}

#[test]
fn services_paths_semantics_stay_in_parity_with_legacy_crate() {
    let new_paths = minnow_app::services::paths::app_paths();
    let old_paths = minnow_paths::app_paths();

    assert_eq!(new_paths.data_dir(), old_paths.data_dir());
    assert_eq!(new_paths.config_file(), old_paths.config_file());
    assert_eq!(new_paths.logs_dir(), old_paths.logs_dir());
    assert_eq!(new_paths.temp_dir(), old_paths.temp_dir());
    assert_eq!(new_paths.temp_file("parity.lock"), old_paths.temp_file("parity.lock"));
    assert_eq!(new_paths.ocr_models_dir(), old_paths.ocr_models_dir());
}

#[test]
fn services_expose_task3_domain_surface() {
    use minnow_app::services::{
        app_meta,
        capture::{
            self,
            action::{ActionContext, CaptureAction, CaptureInputMode},
            long_capture::LongCaptureRuntime,
            source,
            stitcher::{ScrollStitcher, StitchFrameStatus},
        },
        geometry::{self, Rect, RectF},
        i18n,
        ocr::{
            self,
            config::OcrModelType,
            engine::OcrResult,
            model_manager::ModelManager,
            service::OcrModelStatus,
            OcrBlock,
        },
        settings::{self, AppSettings},
    };

    assert_eq!(app_meta::APP_ID, "com.lortunate.minnow");
    assert_eq!(app_meta::APP_NAME, "MinnowSnap");
    assert_eq!(app_meta::APP_LOCK_ID, "com.lortunate.minnow.lock");

    let rect = Rect::new(1, 2, 3, 4);
    assert!(rect.has_area());
    assert_eq!(Rect::empty(), Rect::new(0, 0, 0, 0));
    assert!(RectF::new(0.0, 0.0, 10.0, 10.0).contains_point(5.0, 5.0));
    assert_eq!(geometry::normalize_rect(1.2, 2.3, 3.4, 4.5), Rect::new(1, 2, 4, 5));
    assert_eq!(rect.intersect(Rect::new(2, 3, 10, 10)), Some(Rect::new(2, 3, 2, 3)));

    assert_eq!(i18n::SUPPORTED_LOCALES[0], i18n::SYSTEM_LOCALE);
    assert_eq!(i18n::normalize_locale_tag("zh_CN"), "zh-CN");
    assert_eq!(i18n::normalize_locale_tag("en_US"), "en");
    assert!(!i18n::app::name().is_empty());

    let settings_value = AppSettings::default();
    assert_eq!(settings_value.general.language, "System");
    assert_eq!(settings_value.shortcuts.capture, "F1");
    let _settings_handle = &settings::SETTINGS;

    assert_eq!(source::parse_virtual_source(source::PREVIEW_SOURCE), Some(source::VirtualCaptureSource::Preview));
    assert_eq!(capture::active_monitor_target(), None);

    let ctx = ActionContext::crop_selection("capture.png".to_string(), rect);
    assert_eq!(ctx.input_mode, CaptureInputMode::CropSelection);
    assert_eq!(
        CaptureAction::from_str("copy").expect("action parse"),
        CaptureAction::Copy
    );
    assert_eq!(
        ActionContext::full_image("capture.png".to_string()).input_mode,
        CaptureInputMode::FullImage
    );

    assert_eq!(OcrModelType::Mobile, OcrModelType::Mobile);
    let results = vec![
        OcrResult {
            text: "hello".to_string(),
            confidence: 0.9,
            box_points: vec![(10, 20), (30, 20), (30, 40), (10, 40)],
        },
        OcrResult {
            text: "world".to_string(),
            confidence: 0.8,
            box_points: vec![(40, 20), (60, 20), (60, 40), (40, 40)],
        },
    ];
    let blocks: Vec<OcrBlock> = ocr::build_ocr_blocks(results, 100.0, 100.0);
    assert_eq!(blocks.len(), 2);
    assert!(blocks[0].percentage_coordinates);
    assert_eq!(ocr::format_selected_blocks(&blocks, &[0, 1]).as_deref(), Some("hello world"));
    assert!(ModelManager::default_dir().expect("ocr dir").ends_with("ocr_models"));
    assert_eq!(OcrModelStatus::Ready.progress_percent(), 100);

    let runtime = LongCaptureRuntime::new();
    assert!(runtime.drain_events().is_empty());

    let mut stitcher = ScrollStitcher::new();
    let image = image::RgbaImage::from_pixel(64, 64, image::Rgba([1, 2, 3, 255]));
    let detail = stitcher.process_frame_detailed(image);
    assert_eq!(detail.status, StitchFrameStatus::Appended);
    assert_eq!(detail.height, 64);
}

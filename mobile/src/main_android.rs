//! Android entry point for Bingtray
//!
//! Initializes the eframe app with Android-specific services injected

use android_activity::AndroidApp;
use eframe::NativeOptions;
use std::sync::Arc;

use crate::bingtray::{BingtrayApp, WallpaperSetter};

/// Android wallpaper setter service
struct AndroidWallpaperService;

impl WallpaperSetter for AndroidWallpaperService {
    fn set_wallpaper_from_bytes(&self, bytes: &[u8]) -> std::io::Result<bool> {
        // Delegate to android_wallpaper module
        crate::android_wallpaper::set_wallpaper_from_bytes(bytes)
    }
}

/// Load font file from Android assets at runtime
fn load_font_from_assets(filename: &str) -> Vec<u8> {
    use std::ffi::CString;

    // Get AssetManager from ndk-context
    let asset_manager = ndk_context::android_context()
        .asset_manager();

    // Open asset file
    let filename_cstr = CString::new(filename).expect("CString::new failed");

    unsafe {
        let asset = ndk_sys::AAssetManager_open(
            asset_manager.ptr().as_ptr() as *mut _,
            filename_cstr.as_ptr(),
            ndk_sys::AASSET_MODE_BUFFER as i32,
        );

        if asset.is_null() {
            panic!("Failed to open asset: {}", filename);
        }

        // Get asset length
        let length = ndk_sys::AAsset_getLength64(asset) as usize;

        // Read asset data
        let mut buffer = vec![0u8; length];
        let bytes_read = ndk_sys::AAsset_read(
            asset,
            buffer.as_mut_ptr() as *mut _,
            length,
        );

        // Close asset
        ndk_sys::AAsset_close(asset);

        if bytes_read != length as i32 {
            panic!("Failed to read complete asset: {}", filename);
        }

        buffer
    }
}

/// Android entry point
#[no_mangle]
pub fn android_main(app: AndroidApp) {
    // Initialize Android logger
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)  // Changed to Debug to see android-activity logs
            .with_tag("bingtray"),
    );

    log::info!("Bingtray v{} starting on Android", env!("CARGO_PKG_VERSION"));

    // Initialize wallpaper bridge
    crate::android_wallpaper::init_wallpaper_bridge();

    // Set up panic handler
    std::panic::set_hook(Box::new(|panic_info| {
        let payload = panic_info.payload();

        // Check if this is the expected "winit window doesn't exist" panic
        // This happens when Android destroys the activity after setting wallpaper
        let is_expected_window_panic = if let Some(s) = payload.downcast_ref::<&str>() {
            s.contains("winit window doesn't exist")
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.contains("winit window doesn't exist")
        } else {
            false
        };

        if is_expected_window_panic {
            log::warn!("Expected window destruction during activity lifecycle change: {}", panic_info);
            // Don't treat this as a critical error - it's normal when wallpaper changes
            return;
        }

        // For other panics, log as errors
        log::error!("PANIC: {}", panic_info);
        if let Some(location) = panic_info.location() {
            log::error!("Location: {}:{}", location.file(), location.line());
        }
    }));

    let options = NativeOptions {
        android_app: Some(app),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    match eframe::run_native(
        "Bingtray",
        options,
        Box::new(|cc| {
            // Load Material3 fonts and theme
            use egui_material3::theme::{
                load_fonts, load_themes,
                setup_local_fonts_from_bytes, setup_local_theme,
            };
            // Load fonts from APK assets at runtime (not embedded in .so)
            let material_symbols_data = load_font_from_assets("MaterialSymbolsOutlined_subset.ttf");
            let noto_sans_kr_data = load_font_from_assets("noto-sans-kr.ttf");

            setup_local_fonts_from_bytes(
                "MaterialSymbolsOutlined",
                &material_symbols_data,
            );
            setup_local_fonts_from_bytes("NotoSansKr", &noto_sans_kr_data);

            // Register Korean font with egui for proper text rendering
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "NotoSansKr".to_owned(),
                std::sync::Arc::new(egui::FontData::from_owned(noto_sans_kr_data.clone())),
            );
            // Put Korean font first in proportional and monospace families
            fonts.families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "NotoSansKr".to_owned());
            fonts.families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("NotoSansKr".to_owned());
            cc.egui_ctx.set_fonts(fonts);

            // Prepare themes from build-time constants
            setup_local_theme(None);
            // Install image loaders
            egui_extras::install_image_loaders(&cc.egui_ctx);
            // Load all prepared fonts and themes
            load_fonts(&cc.egui_ctx);
            load_themes();

            // Initialize i18n with Auto language detection
            if let Err(e) = crate::i18n::init_i18n("Auto") {
                log::error!("Failed to initialize i18n: {}", e);
            }

            let mut app = BingtrayApp::default();

            // Inject Android wallpaper setter
            app.set_wallpaper_setter(Arc::new(AndroidWallpaperService));

            log::info!("BingtrayApp initialized with Android services");

            Ok(Box::new(app))
        }),
    ) {
        Ok(_) => {
            log::info!("BingtrayApp exited successfully");
        }
        Err(e) => {
            log::error!("BingtrayApp failed: {}", e);
        }
    }
}

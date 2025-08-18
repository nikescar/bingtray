#![allow(clippy::float_cmp)]
#![allow(clippy::manual_range_contains)]

#[cfg(target_os = "android")]
use android_activity::AndroidApp;
use eframe::{egui, NativeOptions};

mod android_wallpaper;
mod android_screensize;
mod bingtray_service;

use bingtray_core::gui;

// Export modules for external use
pub use gui::{Demo, View};
// Don't export BingtrayApp from gui to avoid name collision
pub use android_wallpaper::{set_wallpaper_from_bytes, set_wallpaper_from_bytes_with_crop};
pub use android_screensize::get_screen_size;
pub use bingtray_service::AndroidBingtrayService;

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    // Initialize Android logger
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("BingtrayApp"),
    );
    
    // Log initialization message to confirm logging is working
    log::info!("Android logger initialized successfully");
    log::info!("Starting mobile application with egui");
    
    // Also use println! as backup logging method
    println!("BingtrayApp: Application starting");
    eprintln!("BingtrayApp: Error stream test");
    
    // Set up panic handler to catch crashes
    std::panic::set_hook(Box::new(|panic_info| {
        log::error!("PANIC OCCURRED: {}", panic_info);
        eprintln!("BingtrayApp PANIC: {}", panic_info);
        if let Some(location) = panic_info.location() {
            log::error!("Panic location: {}:{}", location.file(), location.line());
            eprintln!("BingtrayApp PANIC LOCATION: {}:{}", location.file(), location.line());
        }
    }));

    std::env::set_var("RUST_BACKTRACE", "full");

    let options = NativeOptions {
        android_app: Some(app),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    match BingtrayApp::run(options) {
        Ok(_) => {
            log::info!("BingtrayApp exited successfully");
        }
        Err(e) => {
            log::error!("BingtrayApp failed: {}", e);
            eprintln!("BingtrayApp failed: {}", e);
        }
    }
}

pub struct BingtrayApp {
    bingtray_app: gui::BingtrayApp,
}

impl Default for BingtrayApp {
    fn default() -> Self {
        let mut bingtray_app = gui::BingtrayApp::default();
        
        // Set up platform services using the proper methods
        bingtray_app.set_wallpaper_setter(std::sync::Arc::new(AndroidBingtrayService));
        bingtray_app.set_screen_size_provider(std::sync::Arc::new(AndroidBingtrayService));
        
        Self { bingtray_app }
    }
}

impl BingtrayApp {
    pub fn run(options: NativeOptions) -> Result<(), eframe::Error> {
        eframe::run_native(
            "bingtray-android",
            options,
            Box::new(|cc| {
                egui_extras::install_image_loaders(&cc.egui_ctx);
                Ok(Box::<BingtrayApp>::default())
            }), 
        )
    }
}

impl eframe::App for BingtrayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.bingtray_app.ui(ui);
        });
    }
}

/// Detect narrow screens. This is used to show a simpler UI on mobile devices,
/// especially for the web demo at <https://egui.rs>.
pub fn is_mobile(ctx: &egui::Context) -> bool {
    let screen_size = ctx.screen_rect().size();
    screen_size.x < 1081.0
}

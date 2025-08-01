//! Demo-code for showing how egui is used.
//!
//! This library can be used to test 3rd party egui integrations (see for instance <https://github.com/not-fl3/egui-miniquad/blob/master/examples/demo.rs>).
//!
//! The demo is also used in benchmarks and tests.
//!
//! ## Feature flags

#![allow(clippy::float_cmp)]
#![allow(clippy::manual_range_contains)]

use eframe::{egui, NativeOptions};

mod gui;

// Android wallpaper management module
mod android_wallpaper;

// iOS Bevy app module
#[cfg(target_os = "ios")]
mod ios_app;

#[cfg(target_os = "android")]
use egui_winit::winit;
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use eframe::Renderer;

    std::env::set_var("RUST_BACKTRACE", "full");
    
    // Simpler logger configuration
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("BingtrayApp"),
    );
    
    // Log initialization message to confirm logging is working
    log::info!("Android logger initialized successfully");
    log::info!("Starting mobile application");
    
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

    let options = NativeOptions {
        android_app: Some(app),
        renderer: Renderer::Wgpu,
        ..Default::default()
    };
    DemoApp::run(options).unwrap();
}

#[derive(Default)]
pub struct DemoApp {
    demo_windows: gui::DemoWindows,
}

impl DemoApp {
    pub fn run(options: NativeOptions) -> Result<(), eframe::Error> {
        eframe::run_native(
            "bingtray-mobile",
            options,
            Box::new(|_cc| {
                egui_extras::install_image_loaders(&_cc.egui_ctx);
                Ok(Box::<DemoApp>::default())
            }), 
        )
    }
}

impl eframe::App for DemoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.demo_windows.ui(ctx);
    }
}

// pub use demo::{Demo, DemoWindows, View, WidgetGallery};
pub use gui::{Demo, DemoWindows, View};

#[cfg(target_os = "android")]
pub use android_wallpaper::{set_wallpaper_from_path, set_wallpaper_from_bytes};

#[cfg(not(target_os = "android"))]
pub use android_wallpaper::{set_wallpaper_from_path, set_wallpaper_from_bytes};

/// View some Rust code with syntax highlighting and selection.
pub(crate) fn rust_view_ui(ui: &mut egui::Ui, code: &str) {
    let language = "rs";
    let theme = egui_extras::syntax_highlighting::CodeTheme::from_memory(ui.ctx(), ui.style());
    egui_extras::syntax_highlighting::code_view_ui(ui, &theme, code, language);
}

/// Detect narrow screens. This is used to show a simpler UI on mobile devices,
/// especially for the web demo at <https://egui.rs>.
pub fn is_mobile(ctx: &egui::Context) -> bool {
    let screen_size = ctx.screen_rect().size();
    screen_size.x < 550.0
}


pub const LOREM_IPSUM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";

pub const LOREM_IPSUM_LONG: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.

Curabitur pretium tincidunt lacus. Nulla gravida orci a odio. Nullam various, turpis et commodo pharetra, est eros bibendum elit, nec luctus magna felis sollicitudin mauris. Integer in mauris eu nibh euismod gravida. Duis ac tellus et risus vulputate vehicula. Donec lobortis risus a elit. Etiam tempor. Ut ullamcorper, ligula eu tempor congue, eros est euismod turpis, id tincidunt sapien risus a quam. Maecenas fermentum consequat mi. Donec fermentum. Pellentesque malesuada nulla a mi. Duis sapien sem, aliquet nec, commodo eget, consequat quis, neque. Aliquam faucibus, elit ut dictum aliquet, felis nisl adipiscing sapien, sed malesuada diam lacus eget erat. Cras mollis scelerisque nunc. Nullam arcu. Aliquam consequat. Curabitur augue lorem, dapibus quis, laoreet et, pretium ac, nisi. Aenean magna nisl, mollis quis, molestie eu, feugiat in, orci. In hac habitasse platea dictumst.";

/// Re-export iOS main function
#[cfg(target_os = "ios")]
pub use ios_app::main_rs;









//! Demo-code for showing how egui is used.
//!
//! This library can be used to test 3rd party egui integrations (see for instance <https://github.com/not-fl3/egui-miniquad/blob/master/examples/demo.rs>).
//!
//! The demo is also used in benchmarks and tests.
//!
//! ## Feature flags
#![cfg_attr(feature = "document-features", doc = document_features::document_features!())]

#![allow(clippy::float_cmp)]
#![allow(clippy::manual_range_contains)]

use eframe::{egui, NativeOptions};

mod gui;

// Android wallpaper management module
mod android_wallpaper;

#[cfg(target_os = "android")]
use egui_winit::winit;
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use eframe::Renderer;

    std::env::set_var("RUST_BACKTRACE", "full");
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Trace),
    );

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
            "bingtray-android",
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
pub use android_wallpaper::set_wallpaper_from_path;

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











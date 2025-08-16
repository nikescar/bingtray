//! Bingtray demo app

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::{NativeOptions, egui};
use bingtray_core::gui::BingtrayApp;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc; // Much faster allocator, can give 20% speedups

#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn main() -> Result<(), eframe::Error> {
    // Initialize logging for desktop platforms
    {
        // Silence wgpu log spam (https://github.com/gfx-rs/wgpu/issues/3206)
        let mut rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| {
            if cfg!(debug_assertions) {
                "debug".to_owned()
            } else {
                "info".to_owned()
            }
        });
        for loud_crate in ["naga", "wgpu_core", "wgpu_hal"] {
            if !rust_log.contains(&format!("{loud_crate}=")) {
                rust_log += &format!(",{loud_crate}=warn");
            }
        }

        // SAFETY: we call this from the main thread without any other threads running.
        unsafe {
            std::env::set_var("RUST_LOG", rust_log);
        }
    }

    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    
    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 2340.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    
    eframe::run_native(
        "Bingtray",
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(BingtrayApp::default()))
        }),
    )
}

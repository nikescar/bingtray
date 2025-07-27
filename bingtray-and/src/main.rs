use eframe::NativeOptions;
use bingtray_and::DemoApp;
use egui::Vec2;

fn main() -> Result<(), eframe::Error> {
    // Initialize logging for desktop platforms
    #[cfg(not(target_os = "android"))]
    {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .init();
    }
    
    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 2340.0]),
        ..Default::default()
    };
    DemoApp::run(native_options)
}

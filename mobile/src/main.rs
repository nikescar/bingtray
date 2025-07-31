use eframe::NativeOptions;
use mobile::DemoApp;

#[cfg(not(target_os = "ios"))]
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

// iOS uses a different entry point through the ios_app module
#[cfg(target_os = "ios")]
fn main() {
    // This function is not used on iOS, the actual entry point is main_rs() in ios_app.rs
}

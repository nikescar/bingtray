use eframe::NativeOptions;
use bingtray_and::DemoApp;

fn main() -> Result<(), eframe::Error> {
    // Initialize logging for desktop platforms
    #[cfg(not(target_os = "android"))]
    {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Trace)
            .init();
    }
    
    let options = NativeOptions::default();
    DemoApp::run(options)
}

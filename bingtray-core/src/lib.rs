#[cfg(not(target_arch = "wasm32"))]
use anyhow::Result;
#[cfg(not(target_arch = "wasm32"))]
use std::future::Future;

// Module declarations
pub mod core;
pub mod services;
pub mod wallpaper;
pub mod web;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

// Helper function to run async code in sync context
#[cfg(not(target_arch = "wasm32"))]
pub fn run_async<F>(future: F) -> Result<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    use std::sync::OnceLock;
    use std::sync::mpsc;
    
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    
    let rt = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime")
    });
    
    // Use the runtime to spawn the task and wait for completion
    let (tx, rx) = mpsc::channel();
    rt.spawn(async move {
        let result = future.await;
        let _ = tx.send(result);
    });
    
    rx.recv().map_err(|e| anyhow::anyhow!("Failed to receive result from async task: {}", e))
}

// Public API re-exports from core::exports
pub use core::exports::{
    Config,
    BingImage,
    BingResponse,
    HistoricalImage,
    get_old_market_codes,
    load_market_codes,
    get_historical_page_info,
    download_historical_data,
    get_market_codes,
    get_bing_images,
    // Storage functions
    sanitize_filename,
    get_next_image,
    move_to_keepfavorite,
    blacklist_image,
    need_more_images,
    save_market_codes,
    get_image_metadata,
    load_historical_metadata,
    // Wallpaper functions
    set_wallpaper,
    set_wallpaper_with_service,
};

// Service trait exports
pub use services::{
    FileSystemService,
    WallpaperService,
    ServiceProvider,
    ProjectDirectories,
    DefaultServiceProvider,
};

// Conditional exports
#[cfg(not(target_os = "android"))]
pub use core::exports::open_config_directory;

// GUI-related exports only for non-WASM targets
#[cfg(not(target_arch = "wasm32"))]
pub use core::exports::{
    CarouselImage,
    WallpaperSetter,
    ScreenSizeProvider,
    BingtrayApp,
    BingtrayAppState,
    BingtrayEguiApp,
    EguiCarouselState,
    Resource,
    Demo,
    View,
    is_mobile,
    download_images_for_market,
    get_next_historical_page,
};

#[cfg(target_arch = "wasm32")]
pub use core::exports::{WasmBingtrayApp, SqliteDb, HttpClient, Anchor, WrapApp};

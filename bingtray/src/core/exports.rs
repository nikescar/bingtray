// Public API exports for bingtray

// Re-export core functionality
pub use crate::core::storage::{
    Config,
    sanitize_filename,
    get_next_image,
    move_to_keepfavorite,
    blacklist_image,
    need_more_images,
};

#[cfg(not(target_os = "android"))]
pub use crate::core::storage::open_config_directory;

#[cfg(not(target_arch = "wasm32"))]
pub use crate::core::storage::{
    download_images_for_market,
};

pub use crate::core::database::{
    load_market_codes,
    save_market_codes,
    get_old_market_codes,
    get_historical_page_info,
    download_historical_data,
    load_historical_metadata,
    save_image_metadata,
    get_image_metadata,
    is_blacklisted,
};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::core::database::{
    get_next_historical_page,
};

pub use crate::core::request::{
    BingImage,
    BingResponse,
    HistoricalImage,
    get_market_codes,
    get_bing_images,
};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::core::request::{
    try_bing_api_url,
};

pub use crate::core::view::{
    calculate_screen_rectangle,
    draw_selection_rectangle,
    calculate_image_crop_rect,
    handle_corner_dragging,
    center_rectangle_on_image,
};

// Re-export new core components
pub use crate::core::app::{
    WallpaperSetter,
    ScreenSizeProvider,
    BingtrayAppState,
    CarouselImage,
};

pub use crate::core::egui::{
    BingtrayEguiApp,
};

pub use crate::core::egui_carousel::{
    EguiCarouselState,
    Resource,
};

// Re-export main app components
pub use crate::core::app::{
    BingtrayApp,
    Demo,
    View,
    is_mobile,
};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::core::storage::{
    download_image,
    download_thumbnail_image,
};

// Wallpaper functionality - service-based
pub use crate::wallpaper::{set_wallpaper, set_wallpaper_with_service};

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub use crate::wallpaper::get_desktop_environment;

#[cfg(target_arch = "wasm32")]
pub use crate::wasm::{WasmBingtrayApp, SqliteDb, HttpClient};

#[cfg(target_arch = "wasm32")]
pub use crate::web::{Anchor, WrapApp};

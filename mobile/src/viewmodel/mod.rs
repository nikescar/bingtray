use crate::db::{BingImage, ImageStatus};
use diesel::prelude::*;
use std::sync::mpsc::{Sender, Receiver};
use std::path::PathBuf;
use std::sync::Arc;
use serde::{Serialize, Deserialize};

pub mod background;
pub mod commands;
pub mod sources;
pub mod cache_manager;

/// Crop coordinates (normalized 0.0-1.0 relative to image dimensions)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CropCoords {
    pub x: f32,      // 0.0-1.0, left edge
    pub y: f32,      // 0.0-1.0, top edge
    pub width: f32,  // 0.0-1.0, width
    pub height: f32, // 0.0-1.0, height
}

impl CropCoords {
    /// Validate and clamp coords to [0.0, 1.0] range
    pub fn clamp(self) -> Self {
        Self {
            x: self.x.clamp(0.0, 1.0),
            y: self.y.clamp(0.0, 1.0),
            width: self.width.clamp(0.01, 1.0),  // Min 1% width
            height: self.height.clamp(0.01, 1.0), // Min 1% height
        }
    }

    /// Convert to JSON string for database storage
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse from JSON string from database
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Commands sent from UI to ViewModel background thread
#[derive(Debug, Clone)]
pub enum ViewModelCommand {
    DownloadImages { market_code: String },
    SetWallpaper { url: String },
    ToggleFavorite { url: String },
    BlacklistImage { url: String },
    GetImagesByStatus { status: ImageStatus },
    GetImagesByMarket { market_code: String, page: usize },
    RefreshDatabase,
    Shutdown,

    // NEW: Carousel operations
    LoadCarouselPage {
        filter: Option<ImageStatus>,  // None = All
        page: usize,  // 0-indexed, 20 items per page
    },

    // NEW: Main panel operations
    LoadMainImage {
        url: String,
    },
    UpdateCropCoords {
        url: String,
        coords: CropCoords,
    },
}

/// Events sent from ViewModel background thread to UI
#[derive(Debug, Clone)]
pub enum ViewModelEvent {
    DownloadProgress { current: usize, total: usize },
    DownloadComplete { count: usize },
    ImagesLoaded { images: Vec<BingImage> },
    WallpaperSet { success: bool },
    StatusUpdated { url: String, status: ImageStatus },
    Error { message: String },

    // NEW: Carousel responses
    CarouselPageLoaded {
        page: usize,
        images: Vec<BingImage>,
        total_count: usize,
    },

    // NEW: Main panel responses
    MainImageLoaded {
        url: String,
        image_bytes: Vec<u8>,
        cached: bool,
    },
    MainImageRefreshed {
        url: String,
        image_bytes: Vec<u8>,
    },
    CropCoordsSaved {
        url: String,
    },
}

/// Result returned when setting wallpaper (CLI)
#[derive(Debug, Clone)]
pub struct WallpaperSetResult {
    pub title: String,
    pub url: String,
}

/// ViewModel struct
pub struct ViewModel {
    db_path: PathBuf,
    command_tx: Option<Sender<ViewModelCommand>>,
    event_rx: Option<Receiver<ViewModelEvent>>,
    cache_manager: Option<Arc<cache_manager::CacheManager>>,
}

use anyhow::Result;
use std::sync::mpsc::channel;

impl ViewModel {
    /// Create async ViewModel with background thread (GUI/Android)
    pub fn new_async(db_path: PathBuf) -> Result<Self> {
        let (cmd_tx, cmd_rx) = channel();
        let (evt_tx, evt_rx) = channel();

        // Initialize cache manager
        let cache_dir = db_path.parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid db_path"))?
            .join("cache")
            .join("images");

        let sources = Arc::new(sources::ImageSource::new(None));
        let cache_manager = Arc::new(cache_manager::CacheManager::new(
            cache_dir,
            db_path.clone(),
            Some(sources),
        ));

        // Initialize cache on startup (3 images) in background
        let cache_clone = cache_manager.clone();
        std::thread::spawn(move || {
            if let Err(e) = cache_clone.initialize() {
                log::error!("Cache initialization failed: {}", e);
            }
        });

        let db_path_clone = db_path.clone();
        std::thread::spawn(move || {
            background::run_background_loop(db_path_clone, cmd_rx, evt_tx);
        });

        Ok(Self {
            db_path,
            command_tx: Some(cmd_tx),
            event_rx: Some(evt_rx),
            cache_manager: Some(cache_manager),
        })
    }

    /// Send command to background thread
    pub fn send_command(&self, cmd: ViewModelCommand) -> Result<()> {
        self.command_tx.as_ref()
            .expect("command_tx initialized")
            .send(cmd)?;
        Ok(())
    }

    /// Poll for events from background thread (non-blocking)
    pub fn poll_events(&self) -> Vec<ViewModelEvent> {
        self.event_rx.as_ref()
            .expect("event_rx initialized")
            .try_iter()
            .collect()
    }

    /// Create sync ViewModel (CLI only)
    pub fn new_sync(db_path: PathBuf) -> Result<Self> {
        // Initialize cache manager
        let cache_dir = db_path.parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid db_path"))?
            .join("cache")
            .join("images");

        let sources = Arc::new(sources::ImageSource::new(None));
        let cache_manager = Arc::new(cache_manager::CacheManager::new(
            cache_dir,
            db_path.clone(),
            Some(sources),
        ));

        Ok(Self {
            db_path,
            command_tx: None,
            event_rx: None,
            cache_manager: Some(cache_manager),
        })
    }

    /// Check if ViewModel has cache manager
    pub fn has_cache_manager(&self) -> bool {
        self.cache_manager.is_some()
    }

    /// Get cache manager reference
    pub fn cache_manager(&self) -> Option<&Arc<cache_manager::CacheManager>> {
        self.cache_manager.as_ref()
    }

    // ========================================================================
    // Synchronous Methods (CLI mode)
    // ========================================================================

    /// Download images synchronously (CLI only)
    pub fn download_images_sync(&self, market_code: &str) -> Result<usize> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::download_images_sync(&mut conn, market_code)
    }

    /// Get images by status synchronously (CLI only)
    pub fn get_images_by_status_sync(&self, status: ImageStatus) -> Result<Vec<BingImage>> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        crate::db::operations::get_images_by_status(&mut conn, status)
    }

    /// Set wallpaper synchronously (CLI only)
    pub fn set_wallpaper_sync(&self, url: &str) -> Result<bool> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::set_wallpaper_sync(&mut conn, url)
    }

    /// Toggle favorite synchronously (CLI only)
    pub fn toggle_favorite_sync(&self, url: &str) -> Result<()> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::toggle_favorite_sync(&mut conn, url)
    }

    /// Blacklist image synchronously (CLI only)
    pub fn blacklist_image_sync(&self, url: &str) -> Result<()> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::blacklist_image_sync(&mut conn, url)
    }

    /// Get current desktop wallpaper URL by matching to database (CLI only)
    pub fn get_current_desktop_wallpaper_url_sync(&self) -> Result<Option<String>> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::get_current_desktop_wallpaper_url_sync(&mut conn)
    }

    /// Download and set next wallpaper (CLI only)
    pub fn download_and_set_next_wallpaper_sync(&self) -> Result<WallpaperSetResult> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::download_and_set_next_wallpaper_sync(&mut conn)
    }

    /// Mark current desktop wallpaper as favorite (CLI only)
    pub fn keep_current_wallpaper_sync(&self) -> Result<Option<String>> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::keep_current_wallpaper_sync(&mut conn)
    }

    /// Mark current desktop wallpaper as blacklisted (CLI only)
    pub fn blacklist_current_wallpaper_sync(&self) -> Result<Option<String>> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::blacklist_current_wallpaper_sync(&mut conn)
    }

    /// Set a random favorite as desktop wallpaper (CLI only)
    pub fn set_random_favorite_wallpaper_sync(&self) -> Result<Option<String>> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::set_random_favorite_wallpaper_sync(&mut conn)
    }

    /// Get market state (market_code, offset) from config (CLI only)
    pub fn get_market_state_sync(&self) -> Result<(String, u32)> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::get_market_state_sync(&mut conn)
    }

    /// Save market state (market_code, offset) to config (CLI only)
    pub fn save_market_state_sync(&self, market_code: &str, offset: u32) -> Result<()> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::save_market_state_sync(&mut conn, market_code, offset)
    }

    /// Increment market offset by 8 (CLI only)
    pub fn increment_market_offset_sync(&self) -> Result<()> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::increment_market_offset_sync(&mut conn)
    }

    /// Get database connection (for testing purposes)
    pub fn db_connection(&self) -> Result<SqliteConnection> {
        Ok(crate::db::establish_connection(&self.db_path))
    }

    /// Keep current wallpaper as favorite, set next instantly (CLI only)
    pub fn keep_current_wallpaper_instant_sync(&self) -> Result<String> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        let cache_mgr = self.cache_manager.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Cache manager not available"))?;

        commands::keep_current_wallpaper_instant_sync(&mut conn, cache_mgr)
    }

    /// Blacklist current wallpaper, set next instantly (CLI only)
    pub fn blacklist_current_wallpaper_instant_sync(&self) -> Result<String> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        let cache_mgr = self.cache_manager.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Cache manager not available"))?;

        commands::blacklist_current_wallpaper_instant_sync(&mut conn, cache_mgr)
    }
}

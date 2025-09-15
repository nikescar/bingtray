// cli, gui should depend on this module
// Core application state and traits for Bingtray

use anyhow::Result;
use log::{info, warn};
use std::sync::Arc;

use crate::core::conf::Conf;
use crate::core::sqlite::Sqlite;
use crate::core::bingwpclient::BingWPClient;
use crate::core::request::RequestQueue;

pub trait WallpaperSetter: Send + Sync {
    fn set_wallpaper_from_bytes(&self, image_bytes: &[u8]) -> std::io::Result<bool>;
}

pub trait ScreenSizeProvider: Send + Sync {
    fn get_screen_size(&self) -> std::io::Result<(i32, i32)>;
}

#[derive(Clone)]
pub struct CarouselImage {
    pub title: String,
    pub copyright: String,
    pub copyright_link: String,
    pub thumbnail_url: String,
    pub full_url: String,
    pub image_bytes: Option<Vec<u8>>,
}

pub struct App {
    // UI state
    is_dark_theme: bool,
    window_title: String,

    // Material3 components state
    switch_state: bool,
    slider_value: f32,
    checkbox_state: bool,

    // Application data
    wallpaper_path: Option<String>,

    // conf, sqlite instance
    conf: Conf,
    sqlite: Sqlite,
    request_queue: Arc<RequestQueue>,
}

impl App {
    pub async fn new() -> Result<Self> {
        let conf = Conf::new()?;

        // create sqlite table if not exists
        info!("Using SQLite database at: {:?}", conf.sqlite_file);
        let sqlite = Sqlite::new(conf.sqlite_file.to_str().unwrap())
            .map_err(|e| anyhow::anyhow!("Failed to create SQLite connection: {}", e))?;

        // Create request queue for HTTP requests
        let request_queue = RequestQueue::global();

        // For now, we'll create a simple initialization without external API calls
        // These can be moved to separate initialization methods that are called later

        Ok(Self {
            is_dark_theme: false,
            window_title: "BingTray".to_string(),
            switch_state: false,
            slider_value: 0.5,
            checkbox_state: false,
            wallpaper_path: None,
            conf,
            sqlite,
            request_queue,
        })
    }

    pub async fn initialize_data(&mut self) -> Result<()> {
        // Check if we have market codes
        let markets = self.sqlite.get_all_market()
            .map_err(|e| anyhow::anyhow!("Failed to get market codes: {}", e))?;
        if markets.is_empty() {
            warn!("No market codes found. You may want to fetch them from Bing API later.");
        }

        // Check metadata
        let metadata = self.sqlite.get_all_metadata()
            .map_err(|e| anyhow::anyhow!("Failed to get metadata: {}", e))?;
        if metadata.is_empty() {
            warn!("No metadata found. You may want to fetch images from Bing API later.");
        }

        Ok(())
    }

    pub async fn fetch_market_codes(&mut self) -> Result<()> {
        // TODO: Re-enable when BingWPClient is fixed for native compilation
        warn!("fetch_market_codes is currently disabled - BingWPClient needs WASM/native compatibility");
        Ok(())
    }

    pub async fn fetch_images_for_market(&mut self, market_code: &str) -> Result<()> {
        // TODO: Re-enable when BingWPClient is fixed for native compilation
        warn!("fetch_images_for_market({}) is currently disabled - BingWPClient needs WASM/native compatibility", market_code);
        Ok(())
    }
    
    
}


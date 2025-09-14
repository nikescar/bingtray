// cli, gui should depend on this module
// Core application state and traits for Bingtray

use anyhow::Result;
use log::{info, warn, error};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use poll_promise::Promise;

use crate::core::conf::Conf;
use crate::core::sqlite::Sqlite;

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
}

impl App {
    pub fn new() -> Result<Self> {
        let conf = Conf::new()?;

        // create sqlite table if not exists
        info!("Using SQLite database at: {:?}", conf.sqlite_file);
        let sqlite = Sqlite::new(conf.sqlite_file.to_str().unwrap());

        // download market codes if marketcodes sqlite table is empty
        if sqlite.get_market_codes().await?.is_empty() {
            let market_codes = BingWPClient::new(Arc::clone(&self.http_client)).get_market_codes().await?;
            sqlite.insert_market_codes(&market_codes).await?;
        }

        // get default market images list from 43 markets, 8 images each on weekly basis
        let default_market = conf.default_market.clone();
        let images = sqlite.get_images_by_market(&default_market, 8).await?;
        if images.is_empty() {
            warn!("No images found for default market: {}. Fetching from Bing API...", default_market);
            let images = BingWPClient::new(Arc::clone(&self.http_client)).get_images_by_market(&default_market, 8).await?;
            if images.is_empty() {
                error!("Failed to fetch images for default market: {}", default_market);
            } else {
                sqlite.insert_metadata_entries(&images).await?;
            }
        }

        // get historical metadata if hs_HS marketcode is not exists in market table
        if !sqlite.market_code_exists("hs_HS").await? {
            warn!("Historical market code 'hs_HS' not found in market table. Fetching historical metadata...");
            let historical_images = BingWPClient::new(Arc::clone(&self.http_client)).get_historical_metadata(30).await?;
            if historical_images.is_empty() {
                error!("Failed to fetch historical metadata.");
            } else {
                sqlite.insert_metadata_entries(&historical_images).await?;
            }
        }

        Ok(Self {
            is_dark_theme: false,
            window_title: "BingTray".to_string(),
            switch_state: false,
            slider_value: 0.5,
            checkbox_state: false,
            wallpaper_path: None,
            conf,
        })
    }
    
    
}


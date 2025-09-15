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

        // call initialize_data to check and populate initial data
        let app = Self {
            is_dark_theme: false,
            window_title: "BingTray".to_string(),
            switch_state: false,
            slider_value: 0.5,
            checkbox_state: false,
            wallpaper_path: None,
            conf,
            sqlite,
            request_queue: Arc::clone(&request_queue),
        };

        // Spawn background task to initialize data asynchronously
        let sqlite_path = app.conf.sqlite_file.clone();
        let request_queue = Arc::clone(&app.request_queue);
        tokio::spawn(async move {
            if let Err(e) = Self::initialize_data_background(sqlite_path, request_queue).await {
                log::error!("Failed to initialize app data in background: {}", e);
            }
        });

        Ok(app)
    }

    pub async fn fetch_images_for_market(&mut self, market_code: &str) -> Result<()> {
        let bing_client = BingWPClient::new(Arc::clone(&self.request_queue));
        let images = bing_client.get_bing_images(market_code)
            .map_err(|e| anyhow::anyhow!("Failed to fetch images: {}", e))?;

        // Insert images into database
        for image in images {
            if self.sqlite.find_metadata_by_title(&image.title)
                .map_err(|e| anyhow::anyhow!("Failed to check metadata: {}", e))?.is_none() {
                self.sqlite.new_metadata_entry(
                    false,
                    &image.title,
                    "",
                    &image.copyright,
                    &image.copyright,
                    &image.copyrightlink,
                    &image.url,
                    &image.url,
                ).map_err(|e| anyhow::anyhow!("Failed to insert metadata: {}", e))?;

                
            }
            // set lastvisit to current unix timestamp
            let current_timestamp = chrono::Utc::now().timestamp();
            self.sqlite.update_market_lastvisit(market_code, current_timestamp)
                    .map_err(|e| anyhow::anyhow!("Failed to update market lastvisit: {}", e))?;
        }

        Ok(())
    }

    // Static function to run initialization in background
    async fn initialize_data_background(sqlite_path: std::path::PathBuf, request_queue: Arc<RequestQueue>) -> Result<()> {

        let mut sqlite = Sqlite::new(sqlite_path.to_str().unwrap())
            .map_err(|e| anyhow::anyhow!("Failed to create SQLite connection: {}", e))?;

        // Check if we have market codes
        let market_count = sqlite.get_market_count()
            .map_err(|e| anyhow::anyhow!("Failed to get market count: {}", e))?;
        if market_count == 0 {
            info!("No market codes found. Fetching from Bing API...");
            let bing_client = BingWPClient::new(Arc::clone(&request_queue));
            let market_codes = bing_client.get_market_codes()
                .map_err(|e| anyhow::anyhow!("Failed to fetch market codes: {}", e))?;

            // Insert market codes into database
            for code in market_codes {
                if sqlite.find_market_by_mkcode(&code)
                    .map_err(|e| anyhow::anyhow!("Failed to check market code: {}", e))?.is_none() {
                    sqlite.new_market_entry(&code, 0)
                        .map_err(|e| anyhow::anyhow!("Failed to insert market code: {}", e))?;
                }
            }
            info!("Fetched and stored market codes.");
        }

        // Fetch images for markets that haven't been visited in 7+ days
        let markets = sqlite.get_all_market()
            .map_err(|e| anyhow::anyhow!("Failed to get all markets: {}", e))?;
        let seven_days_ago = chrono::Utc::now().timestamp() - 7 * 24 * 60 * 60;

        for market in markets {
            if market.lastvisit < seven_days_ago {
                info!("Market {} last visited more than 7 days ago. Fetching images...", market.mkcode);

                let bing_client = BingWPClient::new(Arc::clone(&request_queue));
                let images = bing_client.get_bing_images(&market.mkcode)
                    .map_err(|e| anyhow::anyhow!("Failed to fetch images: {}", e))?;

                // Insert images into database
                for image in images {
                    if sqlite.find_metadata_by_title(&image.title)
                        .map_err(|e| anyhow::anyhow!("Failed to check metadata: {}", e))?.is_none() {
                        sqlite.new_metadata_entry(
                            false,
                            &image.title,
                            "",
                            &image.copyright,
                            &image.copyright,
                            &image.copyrightlink,
                            &image.url,
                            &image.url,
                        ).map_err(|e| anyhow::anyhow!("Failed to insert metadata: {}", e))?;
                    }
                }

                // Update lastvisit timestamp
                let current_timestamp = chrono::Utc::now().timestamp();
                sqlite.update_market_lastvisit(&market.mkcode, current_timestamp)
                    .map_err(|e| anyhow::anyhow!("Failed to update market lastvisit: {}", e))?;

                info!("Fetched and stored metadata for market {}.", market.mkcode);

                // Wait 3 seconds before next request to avoid overwhelming the API
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            }
        }

        Ok(())
    }
}


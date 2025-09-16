// cli, gui should depend on this module
// Core application state and traits for Bingtray

use anyhow::Result;
use log::{info, warn};
use wgpu::wgc::instance::FailedLimit;
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
            info!("Initializing application data...");
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
            // image.urlbase is like "/th?id=OHR.Echasse_ROW7944797323"
            // take OHR* before _ from urlbase as image_id
            let parts: Vec<&str> = image.urlbase.split('_').collect();
            let display_name = if !parts.is_empty() { parts[0].trim_start_matches("/th?id=").to_string() } else { image.urlbase.clone() };

            if self.sqlite.find_metadata_by_title(&image.title)
                .map_err(|e| anyhow::anyhow!("Failed to check metadata: {}", e))?.is_none() {
                self.sqlite.new_metadata_entry(
                    false,
                    &image.fullstartdate,
                    display_name.as_str(),
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
        info!("Initializing application data...");
        let mut sqlite = Sqlite::new(sqlite_path.to_str().unwrap())
            .map_err(|e| anyhow::anyhow!("Failed to create SQLite connection: {}", e))?;

        // Check if we have market codes
        let market_count = sqlite.get_market_count()
            .map_err(|e| anyhow::anyhow!("Failed to get market count: {}", e))?;
        if market_count == 0 {
            // info!("No market codes found. Fetching from Bing API...");
            // let bing_client = BingWPClient::new(Arc::clone(&request_queue));
            // let market_codes = bing_client.get_market_codes()
            //     .map_err(|e| anyhow::anyhow!("Failed to fetch market codes: {}", e))?;

            // // Insert market codes into database
            // for code in market_codes {
            //     if sqlite.find_market_by_mkcode(&code)
            //         .map_err(|e| anyhow::anyhow!("Failed to check market code: {}", e))?.is_none() {
            //         sqlite.new_market_entry(&code, 0)
            //             .map_err(|e| anyhow::anyhow!("Failed to insert market code: {}", e))?;
            //     }
            // }
            // info!("Fetched and stored market codes.");

            // insert en-US market code as default
            sqlite.new_market_entry("en-US", 0)
                .map_err(|e| anyhow::anyhow!("Failed to insert default market code: {}", e))?;
            info!("Inserted default market code en-US.");

            sqlite.new_market_entry("historical", 0)
                .map_err(|e| anyhow::anyhow!("Failed to insert default market code: {}", e))?;
            info!("Inserted default market code historical.");

        }

        // Check if we need to fetch images for market en-US (only if last visit is more than 7 days ago)
        let current_timestamp = chrono::Utc::now().timestamp();
        let seven_days_ago = current_timestamp - (7 * 24 * 60 * 60); // 7 days in seconds
        
        let should_fetch = match sqlite.find_market_by_mkcode("en-US")
            .map_err(|e| anyhow::anyhow!("Failed to check market: {}", e))? {
            Some(market) => market.lastvisit < seven_days_ago,
            None => true, // If market doesn't exist, we should fetch
        };

        if should_fetch {
            info!("Fetching images for market en-US...");
            let bing_client = BingWPClient::new(Arc::clone(&request_queue));
            let images = bing_client.get_bing_images("en-US")
            .map_err(|e| anyhow::anyhow!("Failed to fetch images: {}", e))?;

            // Insert images into database
            for image in images {
            if sqlite.find_metadata_by_title(&image.title)
                .map_err(|e| anyhow::anyhow!("Failed to check metadata: {}", e))?.is_none() {

                let parts: Vec<&str> = image.urlbase.split('_').collect();
                let display_name = if !parts.is_empty() { parts[0].trim_start_matches("/th?id=").to_string() } else { image.urlbase.clone() };    
                
                sqlite.new_metadata_entry(
                false,
                &image.fullstartdate,
                display_name.as_str(),
                &image.title,
                &{
                    // Parse author from copyright like "© Oscar Dominguez/TANDEM Stills + Motion"
                    let copyright = &image.copyright;
                    if let Some(start) = copyright.find('©') {
                        let after_copyright = &copyright[start + '©'.len_utf8()..];
                        if let Some(end) = after_copyright.find('/') {
                            after_copyright[..end].trim()
                        } else {
                            // If no '/' found, take everything after '©'
                            after_copyright.trim()
                        }
                    } else {
                        // If no '©' found, use empty string
                        ""
                    }
                },
                &image.copyright,
                &{
                    // Extract text between parentheses at the end
                    let copyright = &image.copyright;
                    if let Some(start) = copyright.rfind('(') {
                        if let Some(end) = copyright.rfind(')') {
                            if end > start {
                                &copyright[start + 1..end]
                            } else {
                                &image.copyright
                            }
                        } else {
                            &image.copyright
                        }
                    } else {
                        &image.copyright
                    }
                },
                &image.copyrightlink,
                &image.url,
                &image.url,
                ).map_err(|e| anyhow::anyhow!("Failed to insert metadata: {}", e))?;
            }
            }

            // Update lastvisit timestamp for "en-US" market
            sqlite.update_market_lastvisit("en-US", current_timestamp)
            .map_err(|e| anyhow::anyhow!("Failed to update market lastvisit: {}", e))?;
            info!("Fetched and stored metadata for market {}.", "en-US");
        } else {
            info!("Skipping fetch for market en-US (last visit was less than 7 days ago)");
        }

        // Check if we need to fetch historical data (only if last visit is more than 7 days ago)
        let should_fetch_historical = match sqlite.find_market_by_mkcode("historical")
            .map_err(|e| anyhow::anyhow!("Failed to check historical market: {}", e))? {
            Some(market) => market.lastvisit < seven_days_ago,
            None => true, // If market doesn't exist, we should fetch
        };

        if should_fetch_historical {
            info!("Fetching historical data...");
            // fetching historical data using download_historical_data and put result into metadata table
            let bing_client = BingWPClient::new(Arc::clone(&request_queue));
            let historical_data = bing_client.download_historical_data()
            .map_err(|e| anyhow::anyhow!("Failed to download historical data: {}", e))?;

            // static failure counter
            let mut failure_count = 0;
            for image in historical_data {
            if sqlite.find_metadata_by_title(&image.title)
            .map_err(|e| anyhow::anyhow!("Failed to check metadata: {}", e))?.is_none() {   
                let parts: Vec<&str> = image.url.split('_').collect();
                let display_name = if !parts.is_empty() { parts[0].trim_start_matches("/th?id=").to_string() } else { image.url.clone() };

                if let Err(e) = sqlite.new_metadata_entry(
                false,
                &image.fullstartdate,
                display_name.as_str(),
                &image.title,
                &{
                    // Parse author from copyright like "© Oscar Dominguez/TANDEM Stills + Motion"
                    let copyright = &image.copyright;
                    if let Some(start) = copyright.find('©') {
                        let after_copyright = &copyright[start + '©'.len_utf8()..];
                        if let Some(end) = after_copyright.find('/') {
                            after_copyright[..end].trim()
                        } else {
                            // If no '/' found, take everything after '©'
                            after_copyright.trim()
                        }
                    } else {
                        // If no '©' found, use empty string
                        ""
                    }
                },
                &format!("{}({})", image.title, image.copyright),
                &image.copyright,
                &image.copyrightlink,
                &image.url,
                &image.url,
                ) {
                failure_count += 1;
                }else{
                failure_count = 0; // reset on success
                }
                if failure_count >= 10 {
                return Err(anyhow::anyhow!("Failed to insert metadata 10 times in a row, aborting."));
                }
            }
            }

            //update lastvisit timestamp for "historical" market
            sqlite.update_market_lastvisit("historical", current_timestamp)
            .map_err(|e| anyhow::anyhow!("Failed to update market lastvisit: {}", e))?;
            info!("Fetched and stored historical metadata.");
        } else {
            info!("Skipping historical data fetch (last visit was less than 7 days ago)");
        }

        Ok(())
    }

    // implement get_wallpaper_metadata_page to get a page of wallpaper metadata from sqlite
    pub fn get_wallpaper_metadata_page(&mut self, page: i32, page_size: i32) -> Result<Vec<CarouselImage>> {
        let offset = (page * page_size) as i64;
        let page_size_i64 = page_size as i64;
        let rows = self.sqlite.get_metadata_page(offset, page_size_i64)
            .map_err(|e| anyhow::anyhow!("Failed to get metadata page: {}", e))?;
        let images = rows.into_iter().map(|row| {
            CarouselImage {
                title: row.title,
                copyright: row.copyright,
                copyright_link: row.copyright_link,
                thumbnail_url: row.thumbnail_url,
                full_url: row.full_url,
                image_bytes: None,
            }
        }).collect();

        Ok(images)
    }   

}

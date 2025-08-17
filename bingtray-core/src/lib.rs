use anyhow::{Context, Result};
use chrono::{Utc, NaiveDate, Duration};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "serde")]
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use log::info;

pub mod gui;
pub mod web;

#[cfg(target_arch = "wasm32")]
pub use web::{Anchor, WrapApp};

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use directories::ProjectDirs;

#[cfg(not(target_os = "android"))]
use std::process::Command;

// Helper function to run async code in sync context
#[cfg(not(target_arch = "wasm32"))]
fn run_async<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    use std::sync::OnceLock;
    use std::sync::mpsc;
    
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    
    let rt = RUNTIME.get_or_init(|| {
        #[cfg(target_arch = "wasm32")]
        {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime")
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime")
        }
    });
    
    // Use the runtime to spawn the task and wait for completion
    let (tx, rx) = mpsc::channel();
    rt.spawn(async move {
        let result = future.await;
        let _ = tx.send(result);
    });
    
    rx.recv().expect("Failed to receive result from async task")
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BingImage {
    pub url: String,
    pub title: String,
    pub copyright: Option<String>,
    pub copyrightlink: Option<String>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BingResponse {
    pub images: Vec<BingImage>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct HistoricalImage {
    pub fullstartdate: String,
    pub url: String,
    pub copyright: String,
    pub copyrightlink: String,
    pub title: String,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub config_dir: PathBuf,
    pub unprocessed_dir: PathBuf,
    pub keepfavorite_dir: PathBuf,
    pub cached_dir: PathBuf,
    pub blacklist_file: PathBuf,
    pub marketcodes_file: PathBuf,
    pub metadata_file: PathBuf,
    pub historical_metadata_file: PathBuf,
}

impl Config {
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "android")]
        {
            // Android-specific paths
            let config_dir = PathBuf::from("/data/data/pe.nikescar.bingtray/files");
            let cache_dir = PathBuf::from("/data/data/pe.nikescar.bingtray/cache");
            
            log::info!("Android config paths - config_dir: {:?}, cache_dir: {:?}", config_dir, cache_dir);
            
            let unprocessed_dir = cache_dir.join("unprocessed");
            let keepfavorite_dir = cache_dir.join("keepfavorite");
            let cached_dir = cache_dir.join("cached");
            let blacklist_file = config_dir.join("blacklist.conf");
            let marketcodes_file = config_dir.join("marketcodes.conf");
            let metadata_file = config_dir.join("metadata.conf");
            let historical_metadata_file = config_dir.join("historical.metadata.conf");
            
            // Create directories if they don't exist
            match fs::create_dir_all(&config_dir) {
                Ok(()) => log::info!("Successfully created config_dir: {:?}", config_dir),
                Err(e) => log::error!("Failed to create config_dir: {:?} - Error: {}", config_dir, e),
            }
            match fs::create_dir_all(&cache_dir) {
                Ok(()) => log::info!("Successfully created cache_dir: {:?}", cache_dir),
                Err(e) => log::error!("Failed to create cache_dir: {:?} - Error: {}", cache_dir, e),
            }
            match fs::create_dir_all(&unprocessed_dir) {
                Ok(()) => log::info!("Successfully created unprocessed_dir: {:?}", unprocessed_dir),
                Err(e) => log::error!("Failed to create unprocessed_dir: {:?} - Error: {}", unprocessed_dir, e),
            }
            match fs::create_dir_all(&keepfavorite_dir) {
                Ok(()) => log::info!("Successfully created keepfavorite_dir: {:?}", keepfavorite_dir),
                Err(e) => log::error!("Failed to create keepfavorite_dir: {:?} - Error: {}", keepfavorite_dir, e),
            }
            match fs::create_dir_all(&cached_dir) {
                Ok(()) => log::info!("Successfully created cached_dir: {:?}", cached_dir),
                Err(e) => log::error!("Failed to create cached_dir: {:?} - Error: {}", cached_dir, e),
            }
            
            // Create config files if they don't exist
            if !blacklist_file.exists() {
                match fs::write(&blacklist_file, "") {
                    Ok(()) => log::info!("Successfully created blacklist.conf: {:?}", blacklist_file),
                    Err(e) => log::error!("Failed to create blacklist.conf: {:?} - Error: {}", blacklist_file, e),
                }
            } else {
                log::info!("blacklist.conf already exists: {:?}", blacklist_file);
            }
            if !metadata_file.exists() {
                match fs::write(&metadata_file, "") {
                    Ok(()) => log::info!("Successfully created metadata.conf: {:?}", metadata_file),
                    Err(e) => log::error!("Failed to create metadata.conf: {:?} - Error: {}", metadata_file, e),
                }
            } else {
                log::info!("metadata.conf already exists: {:?}", metadata_file);
            }
            
            // Automatically generate market codes on Android initialization (like desktop version)
            let should_generate_codes = if !marketcodes_file.exists() {
                log::info!("Marketcodes file doesn't exist, will generate market codes for Android");
                true
            } else {
                // Check if file is empty
                let file_size = fs::metadata(&marketcodes_file).map(|m| m.len()).unwrap_or(0);
                if file_size == 0 {
                    log::info!("Marketcodes file exists but is empty ({}bytes), will generate market codes for Android", file_size);
                    true
                } else {
                    log::info!("Marketcodes file already exists with {} bytes", file_size);
                    false
                }
            };
            
            if should_generate_codes {
                log::info!("Automatically generating market codes for Android...");
                match get_market_codes() {
                    Ok(codes) => {
                        log::info!("Successfully fetched {} market codes from web", codes.len());
                        let mut market_map = std::collections::HashMap::new();
                        for code in codes {
                            market_map.insert(code, 0);
                        }
                        match save_market_codes(&Config {
                            config_dir: config_dir.clone(),
                            unprocessed_dir: unprocessed_dir.clone(),
                            keepfavorite_dir: keepfavorite_dir.clone(),
                            cached_dir: cached_dir.clone(),
                            blacklist_file: blacklist_file.clone(),
                            marketcodes_file: marketcodes_file.clone(),
                            metadata_file: metadata_file.clone(),
                            historical_metadata_file: historical_metadata_file.clone(),
                        }, &market_map) {
                            Ok(()) => log::info!("Successfully saved {} market codes to Android config", market_map.len()),
                            Err(e) => log::error!("Failed to save market codes to Android config: {}", e),
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to fetch market codes from web for Android: {}, creating fallback codes", e);
                        let mut market_map = std::collections::HashMap::new();
                        let fallback_codes = vec![
                            "en-US".to_string(),
                            "en-GB".to_string(),
                            "de-DE".to_string(),
                            "fr-FR".to_string(),
                            "ja-JP".to_string(),
                            "zh-CN".to_string(),
                        ];
                        for code in fallback_codes {
                            market_map.insert(code, 0);
                        }
                        match save_market_codes(&Config {
                            config_dir: config_dir.clone(),
                            unprocessed_dir: unprocessed_dir.clone(),
                            keepfavorite_dir: keepfavorite_dir.clone(),
                            cached_dir: cached_dir.clone(),
                            blacklist_file: blacklist_file.clone(),
                            marketcodes_file: marketcodes_file.clone(),
                            metadata_file: metadata_file.clone(),
                            historical_metadata_file: historical_metadata_file.clone(),
                        }, &market_map) {
                            Ok(()) => log::info!("Successfully saved fallback market codes to Android config"),
                            Err(e) => log::error!("Failed to save fallback market codes to Android config: {}", e),
                        }
                    }
                }
            }
            
            log::info!("Config created with marketcodes_file: {:?}", marketcodes_file);
            
            Ok(Config {
                config_dir,
                unprocessed_dir,
                keepfavorite_dir,
                cached_dir,
                blacklist_file,
                marketcodes_file,
                metadata_file,
                historical_metadata_file,
            })
        }
        
        #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
        {
            let proj_dirs = ProjectDirs::from("com", "bingtray", "bingtray")
                .context("Failed to get project directories")?;
            
            let config_dir = proj_dirs.config_dir().to_path_buf();
            let unprocessed_dir = config_dir.join("unprocessed");
            let keepfavorite_dir = config_dir.join("keepfavorite");
            let cached_dir = config_dir.join("cached");
            let blacklist_file = config_dir.join("blacklist.conf");
            let marketcodes_file = config_dir.join("marketcodes.conf");
            let metadata_file = config_dir.join("metadata.conf");
            let historical_metadata_file = config_dir.join("historical.metadata.conf");

            // Create directories if they don't exist
            fs::create_dir_all(&config_dir)?;
            fs::create_dir_all(&unprocessed_dir)?;
            fs::create_dir_all(&keepfavorite_dir)?;
            fs::create_dir_all(&cached_dir)?;

            // Create blacklist.conf if it doesn't exist
            if !blacklist_file.exists() {
                fs::write(&blacklist_file, "")?;
            }

            // Create metadata.conf if it doesn't exist
            if !metadata_file.exists() {
                fs::write(&metadata_file, "")?;
            }

            // // Create historical.metadata.conf if it doesn't exist with first line as "0"
            // if !historical_metadata_file.exists() {
            //     fs::write(&historical_metadata_file, "0\n")?;
            // }

            Ok(Config {
                config_dir,
                unprocessed_dir,
                keepfavorite_dir,
                cached_dir,
                blacklist_file,
                marketcodes_file,
                metadata_file,
                historical_metadata_file,
            })
        }
        
        // WASM configuration - use in-memory paths
        #[cfg(target_arch = "wasm32")]
        {
            use std::path::PathBuf;
            
            let config_dir = PathBuf::from("/tmp/bingtray");
            let unprocessed_dir = config_dir.join("unprocessed");
            let keepfavorite_dir = config_dir.join("keepfavorite");
            let cached_dir = config_dir.join("cached");
            let blacklist_file = config_dir.join("blacklist.conf");
            let marketcodes_file = config_dir.join("marketcodes.conf");
            let metadata_file = config_dir.join("metadata.conf");
            let historical_metadata_file = config_dir.join("historical.metadata.conf");

            Ok(Config {
                config_dir,
                unprocessed_dir,
                keepfavorite_dir,
                cached_dir,
                blacklist_file,
                marketcodes_file,
                metadata_file,
                historical_metadata_file,
            })
        }
    }


}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_market_codes() -> Result<Vec<String>> {
    log::info!("get_market_codes: Fetching market codes from Microsoft docs");
    let url = "https://learn.microsoft.com/en-us/bing/search-apis/bing-web-search/reference/market-codes";
    
    let response = run_async(async move {
        reqwest::Client::new()
            .get(url)
            .timeout(std::time::Duration::from_secs(30))
            .header("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8")
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await
    });
    
    let response = match response {
        Ok(resp) => {
            log::info!("get_market_codes: HTTP request successful, status: {}", resp.status());
            resp
        },
        Err(e) => {
            log::error!("get_market_codes: HTTP request failed: {}", e);
            return Err(e.into());
        }
    };
    
    let html = run_async(async { response.text().await });
    let html = match html {
        Ok(text) => {
            log::info!("get_market_codes: Received {} bytes of HTML", text.len());
            text
        }
        Err(e) => {
            log::error!("get_market_codes: Failed to read response text: {}", e);
            return Err(e.into());
        }
    };
    
    let document = scraper::Html::parse_document(&html);
    let table_selector = scraper::Selector::parse("table").unwrap();
    let row_selector = scraper::Selector::parse("tr").unwrap();
    let cell_selector = scraper::Selector::parse("td").unwrap();
    
    let mut market_codes = Vec::new();
    
    for table in document.select(&table_selector) {
        for row in table.select(&row_selector).skip(1) { // Skip header row
            let cells: Vec<_> = row.select(&cell_selector).collect();
            if cells.len() >= 2 {
                if let Some(market_code) = cells.last() {
                    let code = market_code.text().collect::<String>().trim().to_string();
                    if !code.is_empty() && code.contains("-") {
                        market_codes.push(code);
                    }
                }
            }
        }
    }
    
    log::info!("get_market_codes: Parsed {} market codes from HTML", market_codes.len());
    if market_codes.is_empty() {
        log::warn!("get_market_codes: No market codes found, using fallback");
        // Fallback market codes if scraping fails
        market_codes = vec![
            "en-US".to_string(),
            "en-GB".to_string(),
            "de-DE".to_string(),
            "fr-FR".to_string(),
            "ja-JP".to_string(),
            "zh-CN".to_string(),
        ];
    }
    
    Ok(market_codes)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_market_codes(config: &Config) -> Result<HashMap<String, i64>> {
    log::info!("load_market_codes: Checking if marketcodes file exists: {:?}", config.marketcodes_file);
    if !config.marketcodes_file.exists() {
        log::info!("load_market_codes: marketcodes.conf doesn't exist, fetching from web");
        let codes = get_market_codes()?;
        log::info!("load_market_codes: Got {} market codes from web", codes.len());
        let mut market_map = HashMap::new();
        for code in codes {
            market_map.insert(code, 0);
        }
        log::info!("load_market_codes: Saving {} market codes to file", market_map.len());
        match save_market_codes(config, &market_map) {
            Ok(()) => log::info!("load_market_codes: Successfully saved market codes"),
            Err(e) => log::error!("load_market_codes: Failed to save market codes: {}", e),
        }
        return Ok(market_map);
    }
    
    log::info!("load_market_codes: marketcodes.conf exists, reading content");
    let content = fs::read_to_string(&config.marketcodes_file)?;
    log::info!("load_market_codes: Read {} bytes from marketcodes.conf", content.len());
    let mut market_map = HashMap::new();
    
    for line in content.lines() {
        if let Some((code, timestamp)) = line.split_once('|') {
            if let Ok(ts) = timestamp.parse::<i64>() {
                market_map.insert(code.to_string(), ts);
            }
        }
    }
    
    log::info!("load_market_codes: Loaded {} market codes from file", market_map.len());
    Ok(market_map)
}

pub fn save_market_codes(config: &Config, market_codes: &HashMap<String, i64>) -> Result<()> {
    log::info!("save_market_codes: Saving {} market codes to {:?}", market_codes.len(), config.marketcodes_file);
    let mut content = String::new();
    for (code, timestamp) in market_codes {
        content.push_str(&format!("{}|{}\n", code, timestamp));
    }
    match fs::write(&config.marketcodes_file, &content) {
        Ok(()) => {
            log::info!("save_market_codes: Successfully wrote {} bytes to marketcodes.conf", content.len());
            Ok(())
        },
        Err(e) => {
            log::error!("save_market_codes: Failed to write to marketcodes.conf: {}", e);
            Err(e.into())
        }
    }
}

// WASM stubs for unavailable functions
#[cfg(target_arch = "wasm32")]
pub fn load_market_codes(_config: &Config) -> Result<HashMap<String, i64>> {
    Ok(HashMap::new())
}

#[cfg(target_arch = "wasm32")]
pub fn download_images_for_market(_config: &Config, _market_code: &str, _thumb_mode: bool) -> Result<(usize, Vec<BingImage>)> {
    Ok((0, Vec::new()))
}

#[cfg(target_arch = "wasm32")]
pub fn get_bing_images(_market_code: &str) -> Result<Vec<BingImage>> {
    Ok(Vec::new())
}

#[cfg(target_arch = "wasm32")]
pub fn download_historical_data(_config: &Config, _starting_index: usize) -> Result<Vec<HistoricalImage>> {
    Ok(Vec::new())
}

#[cfg(target_arch = "wasm32")]
pub fn download_image(_image: &BingImage, _target_dir: &Path, _config: &Config) -> Result<PathBuf> {
    use std::path::PathBuf;
    Ok(PathBuf::from("/tmp/placeholder.jpg"))
}

#[cfg(target_arch = "wasm32")]
pub fn download_thumbnail_image(_image: &BingImage, _config: &Config) -> Result<PathBuf> {
    use std::path::PathBuf;
    Ok(PathBuf::from("/tmp/placeholder_thumb.jpg"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_bing_images(market_code: &str) -> Result<Vec<BingImage>> {
    // Try multiple URL configurations to find one that works
    let url = format!("https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=8&mkt={}", market_code);
    log::info!("URL: {}", url);
    
    if let Ok(result) = try_bing_api_url(&url, market_code, 1) {
        log::info!("SUCCESS: URL variant worked!");
        return Ok(result);
    }
    log::error!("FAILED: URL variant failed");
    
    Err(anyhow::anyhow!("All URL variants failed"))
}

#[cfg(not(target_arch = "wasm32"))]
fn try_bing_api_url(url: &str, market_code: &str, _attempt_num: usize) -> Result<Vec<BingImage>> {
    
    // Add comprehensive network diagnostics for Android debugging
    log::info!("=== NETWORK DIAGNOSTICS START ===");
    log::info!("Target URL: {}", url);
    log::info!("Market Code: {}", market_code);
    
    #[cfg(target_os = "android")]
    {
        log::info!("Platform: Android");
        log::info!("Checking network connectivity...");
        // Add basic connectivity check
        match std::net::TcpStream::connect_timeout(&"8.8.8.8:53".parse().unwrap(), std::time::Duration::from_secs(5)) {
            Ok(_) => log::info!("Basic internet connectivity: OK"),
            Err(e) => log::error!("Basic internet connectivity: FAILED - {}", e),
        }
        
        // Test DNS resolution
        match std::net::ToSocketAddrs::to_socket_addrs(&"bing.com:443") {
            Ok(addrs) => {
                let addrs: Vec<_> = addrs.collect();
                log::info!("DNS resolution for bing.com: OK - {} addresses resolved", addrs.len());
                for addr in addrs.iter().take(3) {
                    log::info!("  Resolved address: {}", addr);
                }
            }
            Err(e) => log::error!("DNS resolution for bing.com: FAILED - {}", e),
        }
    }
    
    log::info!("=== NETWORK DIAGNOSTICS END ===");
    
    // Add timeout and retry logic to handle "unexpected end of file" errors
    let max_retries = 3;
    let mut last_error = None;
    
    for attempt in 1..=max_retries {
        log::info!("Attempting to fetch Bing images (attempt {}/{}) for market: {}", attempt, max_retries, market_code);
        
        let url_owned = url.to_string(); // Convert &str to owned String
        let result = run_async(async move {
            reqwest::Client::new()
                .get(&url_owned)
                .timeout(std::time::Duration::from_secs(30)) // 30 second timeout
                .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64; rv:10.0) Gecko/20100101 Firefox/10.0")
                .header("Accept", "application/json, text/plain, */*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Cache-Control", "no-cache")
                .header("Referer", "https://www.bing.com/")
                .send()
                .await
        });
        
        match result
        {
            Ok(response) => {
                log::info!("HTTP response received, status: {}, content-length: {:?}", 
                          response.status(), response.headers().get("content-length"));
                
                let status = response.status();
                let text_result = run_async(async { response.text().await });
                match text_result {
                    Ok(text) => {
                        log::info!("Response text received, length: {} bytes", text.len());
                        if text.trim().is_empty() {
                            log::warn!("Empty response received, retrying...");
                            last_error = Some(anyhow::anyhow!("Empty response from server"));
                            continue;
                        }
                        
                        #[cfg(feature = "serde")]
                        match serde_json::from_str::<BingResponse>(&text) {
                            Ok(bing_response) => {
                                log::info!("Successfully parsed {} images from response", bing_response.images.len());
                                return Ok(bing_response.images);
                            }
                            Err(e) => {
                                log::error!("JSON parsing failed: {}", e);
                                log::error!("Full response content: {}", &text);
                                log::error!("Response status was: {}", status);
                                last_error = Some(e.into());
                                continue;
                            }
                        }
                        
                        #[cfg(not(feature = "serde"))]
                        {
                            log::error!("Serde feature not enabled - cannot parse JSON response");
                            last_error = Some(anyhow::anyhow!("Serde feature required for JSON parsing"));
                            continue;
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to read response text: {}", e);
                        last_error = Some(e.into());
                        
                        // Wait before retry
                        if attempt < max_retries {
                            std::thread::sleep(std::time::Duration::from_millis(1000 * attempt as u64));
                        }
                        continue;
                    }
                }
            }
            Err(e) => {
                log::error!("HTTP request failed (attempt {}): {}", attempt, e);
                log::error!("Error details: {:?}", e);
                
                // Android-specific error analysis
                #[cfg(target_os = "android")]
                {
                    let error_msg = format!("{}", e);
                    if error_msg.contains("unexpected end of file") {
                        log::error!("ANDROID ISSUE: Connection terminated prematurely - likely network security policy or DNS issue");
                    } else if error_msg.contains("connection refused") {
                        log::error!("ANDROID ISSUE: Connection refused - check firewall or network restrictions");
                    } else if error_msg.contains("timeout") {
                        log::error!("ANDROID ISSUE: Connection timeout - check network connectivity");
                    } else if error_msg.contains("certificate") || error_msg.contains("ssl") || error_msg.contains("tls") {
                        log::error!("ANDROID ISSUE: SSL/TLS certificate issue - check network security config");
                    } else {
                        log::error!("ANDROID ISSUE: Unknown network error - {}", error_msg);
                    }
                }
                
                last_error = Some(e.into());
                
                // Wait before retry
                if attempt < max_retries {
                    std::thread::sleep(std::time::Duration::from_millis(1000 * attempt as u64));
                }
                continue;
            }
        }
    }
    
    // All retries failed - try one final diagnostic test
    #[cfg(target_os = "android")]
    {
        log::error!("=== FINAL DIAGNOSTIC TEST ===");
        log::info!("Testing simple HTTP connection to google.com...");
        let google_http_result = run_async(async {
            reqwest::Client::new()
                .get("http://google.com")
                .timeout(std::time::Duration::from_secs(10))
                .send()
                .await
        });
        match google_http_result
        {
            Ok(response) => {
                log::info!("Google HTTP test: SUCCESS - Status: {}", response.status());
                log::error!("CONCLUSION: Basic HTTP works, issue is specific to Bing HTTPS endpoint");
            }
            Err(e) => {
                log::error!("Google HTTP test: FAILED - {}", e);
                log::error!("CONCLUSION: General network connectivity issue on Android");
            }
        }
        
        log::info!("Testing HTTPS connection to google.com...");
        let google_https_result = run_async(async {
            reqwest::Client::new()
                .get("https://google.com")
                .timeout(std::time::Duration::from_secs(10))
                .send()
                .await
        });
        match google_https_result
        {
            Ok(response) => {
                log::info!("Google HTTPS test: SUCCESS - Status: {}", response.status());
                log::error!("CONCLUSION: HTTPS works, issue is specific to Bing endpoint or headers");
            }
            Err(e) => {
                log::error!("Google HTTPS test: FAILED - {}", e);
                log::error!("CONCLUSION: HTTPS/TLS issue on Android");
            }
        }
        
        log::info!("Testing Bing endpoint with proper headers...");
        let bing_base_result = run_async(async {
            reqwest::Client::new()
                .get("https://www.bing.com/")
                .timeout(std::time::Duration::from_secs(10))
                .header("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")
                .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
                .send()
                .await
        });
        match bing_base_result
        {
            Ok(response) => {
                log::info!("Bing base URL test: SUCCESS - Status: {}", response.status());
                log::info!("CONCLUSION: Bing accepts requests with proper headers");
            }
            Err(e) => {
                log::error!("Bing base URL test: FAILED - {}", e);
                log::error!("CONCLUSION: Bing endpoint completely blocked");
            }
        }
        
        log::info!("Testing Bing API endpoint directly (HTTPS)...");
        let test_api_url = format!("https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=1&mkt={}", "en-US");
        let bing_api_https_result = run_async(async move {
            reqwest::Client::new()
                .get(&test_api_url)
                .timeout(std::time::Duration::from_secs(10))
                .header("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")
                .header("Accept", "application/json, text/plain, */*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Cache-Control", "no-cache")
                .header("Referer", "https://www.bing.com/")
                .send()
                .await
        });
        match bing_api_https_result
        {
            Ok(response) => {
                log::info!("Bing API HTTPS test: SUCCESS - Status: {}", response.status());
                if response.status().is_success() {
                    log::info!("CONCLUSION: Bing API endpoint is working correctly!");
                } else {
                    log::error!("CONCLUSION: Bing API endpoint returned error status: {}", response.status());
                    match run_async(async { response.text().await }) {
                        Ok(body) => log::error!("Response body: {}", body),
                        Err(_) => log::error!("Could not read response body"),
                    }
                }
            }
            Err(e) => {
                log::error!("Bing API HTTPS test: FAILED - {}", e);
                log::error!("CONCLUSION: Bing API endpoint has HTTPS connection issues");
            }
        }
        
        log::info!("Testing Bing API endpoint directly (HTTP fallback)...");
        let test_api_url_http = format!("http://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=1&mkt={}", "en-US");
        match run_async(async move {
            reqwest::Client::new()
                .get(&test_api_url_http)
                .timeout(std::time::Duration::from_secs(10))
                .header("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")
                .header("Accept", "application/json, text/plain, */*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .send()
                .await
        })
        {
            Ok(response) => {
                log::info!("Bing API HTTP test: SUCCESS - Status: {}", response.status());
                if response.status().is_success() {
                    log::info!("CONCLUSION: Bing API works with HTTP! HTTPS is the problem.");
                } else {
                    log::info!("Bing API HTTP returned status: {}", response.status());
                }
            }
            Err(e) => {
                log::error!("Bing API HTTP test: FAILED - {}", e);
                log::error!("Both HTTP and HTTPS failed for Bing API");
            }
        }
        
        log::error!("=== END DIAGNOSTIC TEST ===");
    }
    
    // All retries failed, return the last error
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All {} attempts failed", max_retries)))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn download_image(image: &BingImage, target_dir: &Path, config: &Config) -> Result<PathBuf> {
    let url = if image.url.starts_with("http") {
        image.url.clone()
    } else {
        format!("https://bing.com{}", image.url)
    };

    // image.url looks like this "/th?id=OHR.TemplePhilae_EN-US5062419351_1920x1080.jpg&rf=LaDigue_1920x1080.jpg&pid=hp"
    // please extract "OHR.TemplePhilae" part and set it to display_name
    let display_name = image.url
        .split("th?id=")
        .nth(1)
        .and_then(|s| s.split('_').next())
        .unwrap_or(&image.title)
        .to_string();
    let filename = format!("{}.jpg", sanitize_filename(&display_name));
    let filepath = target_dir.join(&filename);
    
    // Check if file exists in keepfavorite folder
    let keepfavorite_path = target_dir.parent()
        .map(|parent| parent.join("keepfavorite").join(&filename));
    
    if let Some(keepfavorite_file) = keepfavorite_path {
        if keepfavorite_file.exists() {
            // File already exists in keepfavorite, skip download
            return Ok(filepath);
        }
    }
    
    if !filepath.exists() {
        let response = run_async(async move {
            reqwest::Client::new()
                .get(&url)
                .timeout(std::time::Duration::from_secs(30))
                .header("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")
                .header("Accept", "image/webp,image/apng,image/*,*/*;q=0.8")
                .header("Referer", "https://www.bing.com/")
                .send()
                .await
        })?;
        let bytes = run_async(async { response.bytes().await })?;
        fs::write(&filepath, bytes)?;
    }
    
    // Save metadata if available
    if let (Some(copyright), Some(copyrightlink)) = (&image.copyright, &image.copyrightlink) {
        save_image_metadata(config, &sanitize_filename(&display_name), copyright, copyrightlink)?;
    }
    
    Ok(filepath)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn download_thumbnail_image(image: &BingImage, config: &Config) -> Result<PathBuf> {
    // Convert the URL to a thumbnail URL with 320x240 dimensions
    let base_url = if image.url.starts_with("http") {
        image.url.clone()
    } else {
        format!("https://bing.com{}", image.url)
    };
    
    // Create thumbnail URL by adding w=320&h=240 parameters
    let thumbnail_url = if base_url.contains('?') {
        format!("{}&w=320&h=240", base_url)
    } else {
        format!("{}?w=320&h=240", base_url)
    };

    // image.url looks like this "/th?id=OHR.TemplePhilae_EN-US5062419351_1920x1080.jpg&rf=LaDigue_1920x1080.jpg&pid=hp"
    // please extract "OHR.TemplePhilae" part and set it to display_name
    let display_name = image.url
        .split("th?id=")
        .nth(1)
        .and_then(|s| s.split('_').next())
        .unwrap_or(&image.title)
        .to_string();
    let filename = format!("{}_thumb.jpg", sanitize_filename(&display_name));
    let filepath = config.cached_dir.join(&filename);
    
    // Check if file exists in keepfavorite folder
    let keepfavorite_path = config.keepfavorite_dir.join(&filename);
    
    if keepfavorite_path.exists() {
        // File already exists in keepfavorite, skip download
        return Ok(filepath);
    }
    
    if !filepath.exists() {
        let response = run_async(async move {
            reqwest::Client::new()
                .get(&thumbnail_url)
                .timeout(std::time::Duration::from_secs(30))
                .header("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")
                .header("Accept", "image/webp,image/apng,image/*,*/*;q=0.8")
                .header("Referer", "https://www.bing.com/")
                .send()
                .await
        })?;
        let bytes = run_async(async { response.bytes().await })?;
        fs::write(&filepath, bytes)?;
    }
    
    // Save metadata if available
    if let (Some(copyright), Some(copyrightlink)) = (&image.copyright, &image.copyrightlink) {
        save_image_metadata(config, &sanitize_filename(&display_name), copyright, copyrightlink)?;
    }
    
    Ok(filepath)
}

pub fn sanitize_filename(filename: &str) -> String {
    let sanitized = filename
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_string();
    
    // Limit filename length to avoid filesystem issues
    // Keep it reasonable while preserving readability
    if sanitized.len() > 100 {
        sanitized.chars().take(100).collect()
    } else {
        sanitized
    }
}

pub fn get_next_image(config: &Config) -> Result<Option<PathBuf>> {
    loop {
        let entries = fs::read_dir(&config.unprocessed_dir)?;
        let images: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_lowercase() == "jpg")
                    .unwrap_or(false)
            })
            .collect();
        
        if images.is_empty() {
            return Ok(None);
        }
        
        // Use a simple pseudo-random selection based on current time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as usize;
        let index = now % images.len();
        let selected_image = &images[index];
        
        // // Check if the image is blacklisted
        // if let Some(filename) = selected_image.file_stem().and_then(|s| s.to_str()) {
        //     if is_blacklisted(config, filename)? {
        //         // Remove the blacklisted file and continue searching
        //         if let Err(e) = fs::remove_file(selected_image) {
        //             eprintln!("Warning: Failed to remove blacklisted file {}: {}", 
        //                      selected_image.display(), e);
        //         }
        //         continue; // Try again with remaining files
        //     }
        // }
        
        return Ok(Some(selected_image.clone()));
    }
}

pub fn move_to_keepfavorite(config: &Config, image_path: &Path) -> Result<()> {
    if let Some(filename) = image_path.file_name() {
        let target_path = config.keepfavorite_dir.join(filename);
        fs::rename(image_path, target_path)?;
    }
    Ok(())
}

pub fn blacklist_image(config: &Config, image_path: &Path) -> Result<()> {
    // Extract hash from filename
    if let Some(filename) = image_path.file_stem().and_then(|s| s.to_str()) {
        // Add the full filename to blacklist
        let mut blacklist = fs::read_to_string(&config.blacklist_file).unwrap_or_default();
        blacklist.push_str(&format!("{}\n", filename));
        fs::write(&config.blacklist_file, blacklist)?;
    }
    
    // Remove the file
    fs::remove_file(image_path)?;
    Ok(())
}

pub fn is_blacklisted(config: &Config, filename: &str) -> Result<bool> {
    let blacklist = fs::read_to_string(&config.blacklist_file).unwrap_or_default();
    println!("Checking if {} is blacklisted : {}", filename, blacklist.lines().any(|line| line.trim() == filename));
    Ok(blacklist.lines().any(|line| line.trim() == filename))
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub fn get_desktop_environment() -> String {
    if let Ok(desktop_session) = std::env::var("DESKTOP_SESSION") {
        let session = desktop_session.to_lowercase();
        if ["gnome", "unity", "cinnamon", "mate", "xfce4", "lxde", "fluxbox", 
            "blackbox", "openbox", "icewm", "jwm", "afterstep", "trinity", "kde"].contains(&session.as_str()) {
            return session;
        }
        
        if session.contains("xfce") || session.starts_with("xubuntu") {
            return "xfce4".to_string();
        } else if session.starts_with("ubuntustudio") {
            return "kde".to_string();
        } else if session.starts_with("ubuntu") {
            return "gnome".to_string();
        } else if session.starts_with("lubuntu") {
            return "lxde".to_string();
        } else if session.starts_with("kubuntu") {
            return "kde".to_string();
        }
    }
    
    if std::env::var("KDE_FULL_SESSION").unwrap_or_default() == "true" {
        return "kde".to_string();
    }
    
    if std::env::var("GNOME_DESKTOP_SESSION_ID").is_ok() {
        return "gnome".to_string();
    }
    
    "unknown".to_string()
}

pub fn set_wallpaper(file_path: &Path) -> Result<bool> {
    let file_loc = file_path.to_string_lossy();
    
    // Android-specific wallpaper setting
    #[cfg(target_os = "android")]
    {
        // Read the image file and use set_wallpaper_from_bytes
        match std::fs::read(file_path) {
            Ok(_image_bytes) => {
                // This function should be provided by the mobile crate
                // For now, we'll return false as it requires mobile integration
                eprintln!("Android wallpaper setting requires mobile crate integration");
                return Ok(false);
            }
            Err(e) => {
                eprintln!("Failed to read image file for Android wallpaper: {}", e);
                return Ok(false);
            }
        }
    }
    
    // Use wallpaper crate for cross-platform wallpaper setting (non-Android, non-WASM)
    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    {
        match wallpaper::set_from_path(&file_loc) {
            Ok(_) => {
                println!("Wallpaper set successfully to: {}", file_loc);
                Ok(true)
            }
            Err(e) => {
                eprintln!("Failed to set wallpaper: {}", e);
                
                // Fallback to platform-specific methods for Linux if wallpaper crate fails
                return set_wallpaper_linux_fallback(file_path);
            }
        }
    }
    
    // WASM fallback - wallpaper setting not supported
    #[cfg(target_arch = "wasm32")]
    {
        eprintln!("Wallpaper setting not supported on WASM");
        Ok(false)
    }
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn set_wallpaper_linux_fallback(file_path: &Path) -> Result<bool> {
    let file_loc = file_path.to_string_lossy();
    let desktop_env = get_desktop_environment();
    
    match desktop_env.as_str() {
        "gnome" | "unity" | "cinnamon" => {
            let uri = format!("file://{}", file_loc);
            let output = Command::new("gsettings")
                .args(&["set", "org.gnome.desktop.background", "picture-uri", &uri])
                .output()?;
            Ok(output.status.success())
        }
        "mate" => {
            let output = Command::new("gsettings")
                .args(&["set", "org.mate.background", "picture-filename", &file_loc])
                .output()?;
            Ok(output.status.success())
        }
        "xfce4" => {
            // Get all monitor paths that contain "workspace0/last-image"
            let list_output = Command::new("xfconf-query")
                .args(&["-c", "xfce4-desktop", "-l"])
                .output()?;
            
            if list_output.status.success() {
                let paths = String::from_utf8_lossy(&list_output.stdout);
                let monitor_paths: Vec<&str> = paths
                    .lines()
                    .filter(|line| line.contains("workspace0/last-image"))
                    .collect();
                
                // Set wallpaper for each monitor
                for path in monitor_paths {
                    if !path.trim().is_empty() {
                        Command::new("xfconf-query")
                            .args(&["-c", "xfce4-desktop", "-p", path.trim(), "-s", &file_loc])
                            .output()?;
                    }
                }
            }
            
            // Set default properties for the primary monitor as fallback
            Command::new("xfconf-query")
                .args(&["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/image-path", "-s", &file_loc])
                .output()?;
            Command::new("xfconf-query")
                .args(&["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/image-style", "-s", "3"])
                .output()?;
            Command::new("xfconf-query")
                .args(&["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/image-show", "-s", "true"])
                .output()?;
            
            let output = Command::new("xfdesktop")
                .args(&["--reload"])
                .output()?;
            Ok(output.status.success())
        }
        "lxde" => {
            let cmd = format!("pcmanfm --set-wallpaper {} --wallpaper-mode=scaled", file_loc);
            let output = Command::new("sh")
                .args(&["-c", &cmd])
                .output()?;
            Ok(output.status.success())
        }
        "fluxbox" | "jwm" | "openbox" | "afterstep" => {
            let output = Command::new("fbsetbg")
                .arg(file_loc.as_ref())
                .output()?;
            Ok(output.status.success())
        }
        "icewm" => {
            let output = Command::new("icewmbg")
                .arg(file_loc.as_ref())
                .output()?;
            Ok(output.status.success())
        }
        "blackbox" => {
            let output = Command::new("bsetbg")
                .args(&["-full", &file_loc])
                .output()?;
            Ok(output.status.success())
        }
        _ => {
            eprintln!("Desktop environment '{}' not supported", desktop_env);
            Ok(false)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn download_images_for_market(config: &Config, market_code: &str, thumb_mode: bool) -> Result<(usize, Vec<BingImage>)> {
    let images = get_bing_images(market_code)?;
    let mut downloaded_count = 0;
    let mut downloaded_images = Vec::new();
    
    for (_, image) in images.iter().enumerate() {
        let mut display_name = image.url
            .split("th?id=")
            .nth(1)
            .and_then(|s| s.split('_').next())
            .unwrap_or(&image.title)
            .to_string();
        display_name = sanitize_filename(&display_name);

        let filename_suffix = if thumb_mode { "_thumb.jpg" } else { ".jpg" };
        let target_dir = if thumb_mode { &config.cached_dir } else { &config.unprocessed_dir };
        
        let target_path = target_dir.join(format!("{}{}", display_name, filename_suffix));
        let keepfavorite_path = config.keepfavorite_dir.join(format!("{}{}", display_name, filename_suffix));
        
        if !target_path.exists() && !keepfavorite_path.exists() && !is_blacklisted(config, &display_name)? {
            
            let download_result = if thumb_mode {
                download_thumbnail_image(&image, config)
            } else {
                download_image(&image, &config.unprocessed_dir, config)
            };
            
            match download_result {
                Ok(filepath) => {
                    let download_type = if thumb_mode { "thumbnail" } else { "image" };
                    println!("Downloaded {}: {}", download_type, filepath.display());
                    downloaded_count += 1;
                    downloaded_images.push((*image).clone());
                }
                Err(e) => {
                    let download_type = if thumb_mode { "thumbnail" } else { "image" };
                    eprintln!("Failed to download {} {}: {}", download_type, display_name, e);
                }
            }
        } else {
            let download_type = if thumb_mode { "thumbnail" } else { "image" };
            println!("Skipping already downloaded or blacklisted {}: {}", download_type, display_name);
        }
    }
    
    Ok((downloaded_count, downloaded_images))
}

pub fn save_image_metadata(config: &Config, filename: &str, copyright: &str, copyrightlink: &str) -> Result<()> {
    let metadata = fs::read_to_string(&config.metadata_file).unwrap_or_default();
    
    // Extract text in parentheses from copyright
    let copyright_text = if let Some(start) = copyright.find('(') {
        if let Some(end) = copyright.find(')') {
            if end > start {
                copyright[start+1..end].to_string()
            } else {
                copyright.to_string()
            }
        } else {
            copyright.to_string()
        }
    } else {
        copyright.to_string()
    };
    
    // Check if entry already exists and replace it
    let mut lines: Vec<String> = metadata.lines().map(|s| s.to_string()).collect();
    let mut found = false;
    
    for line in &mut lines {
        if line.starts_with(&format!("{}|", filename)) {
            *line = format!("{}|{}|{}", filename, copyright_text, copyrightlink);
            found = true;
            break;
        }
    }
    
    if !found {
        lines.push(format!("{}|{}|{}", filename, copyright_text, copyrightlink));
    }
    
    let content = lines.join("\n");
    if !content.is_empty() {
        fs::write(&config.metadata_file, content + "\n")?;
    }
    
    Ok(())
}

pub fn get_image_metadata(config: &Config, filename: &str) -> Option<(String, String)> {
    // First try regular metadata.conf
    let metadata = fs::read_to_string(&config.metadata_file).unwrap_or_default();
    for line in metadata.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 3 && parts[0] == filename {
            return Some((parts[1].to_string(), parts[2].to_string()));
        }
    }
    
    // If not found, try historical.metadata.conf
    let historical_metadata_file = config.config_dir.join("historical.metadata.conf");
    if let Ok(historical_metadata) = fs::read_to_string(&historical_metadata_file) {
        for line in historical_metadata.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 3 && parts[0] == filename {
                return Some((parts[1].to_string(), parts[2].to_string()));
            }
        }
    }
    
    None
}

#[cfg(not(target_os = "android"))]
pub fn open_config_directory(config: &Config) -> Result<()> {
    let config_path = &config.config_dir;
    
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(config_path)
            .spawn()?;
    }
    
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(config_path)
            .spawn()?;
    }
    
    #[cfg(target_os = "linux")]
    {
        // Try different file managers in order of preference
        let file_managers = ["xdg-open", "nautilus", "dolphin", "thunar", "pcmanfm", "nemo"];
        let mut opened = false;
        
        for fm in &file_managers {
            if let Ok(_child) = Command::new(fm)
                .arg(config_path)
                .spawn() 
            {
                opened = true;
                break;
            }
        }
        
        if !opened {
            eprintln!("Could not find a suitable file manager to open {}", config_path.display());
        }
    }
    
    Ok(())
}

pub fn need_more_images(config: &Config) -> Result<bool> {
    let unprocessed_count = fs::read_dir(&config.unprocessed_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_lowercase() == "jpg")
                .unwrap_or(false)
        })
        .count();
    
    Ok(unprocessed_count == 0)
}

pub fn get_old_market_codes(market_codes: &HashMap<String, i64>) -> Vec<String> {
    let now = Utc::now().timestamp();
    let seven_days_ago = now - (7 * 24 * 60 * 60);
    
    market_codes
        .iter()
        .filter(|(_, &timestamp)| timestamp < seven_days_ago)
        .map(|(code, _)| code.clone())
        .collect()
}

/// Download and parse historical data from GitHub repository
#[cfg(not(target_arch = "wasm32"))]
pub fn download_historical_data(config: &Config, _starting_index: usize) -> Result<Vec<HistoricalImage>> {
    // Check if historical metadata conf exists, if so, load and return first 8 images
    if config.historical_metadata_file.exists() {
        let (_, images) = load_historical_metadata(config)?;
        // Return only first 8 records of historical_images from the end (most recent)
        return Ok(images.into_iter().rev().take(8).collect());
    }

    // parse all data when first download historical data
    let url = "https://raw.githubusercontent.com/v5tech/bing-wallpaper/refs/heads/main/bing-wallpaper.md";
    let response = run_async(async move {
        reqwest::Client::new()
            .get(url)
            .timeout(std::time::Duration::from_secs(30))
            .header("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")
            .send()
            .await
    })?;
    let content = run_async(async { response.text().await })?;
    
    let lines: Vec<&str> = content.lines().collect();
    let mut historical_images = Vec::new();
    
    // Parse all historical data at once on first download
    for line in lines.iter() {
        if let Some(historical_image) = parse_historical_line(line)? {
            historical_images.push(historical_image);
        }
    }

    if historical_images.is_empty() {
        return Ok(Vec::new());
    }

    // save all historical images to historial.metadata.conf file
    // Set current_page = 0 since we're returning the first page of data (page 0)
    let current_page = 0;
    let mut metadata_content = format!("{}\n", current_page);
    for image in &historical_images {
        #[cfg(feature = "serde")]
        {
            metadata_content.push_str(&format!("{}\n", serde_json::to_string(image)?));
        }
        #[cfg(not(feature = "serde"))]
        {
            metadata_content.push_str(&format!("{:?}\n", image));
        }
    }
    fs::write(&config.historical_metadata_file, metadata_content)?;

    // return only first 8 records of historical_images from last
    let historical_images = historical_images.into_iter().rev().take(8).collect();

    Ok(historical_images)
}

/// Parse a single line from the historical data markdown
fn parse_historical_line(line: &str) -> Result<Option<HistoricalImage>> {
    // Example line: "2025-08-04 | [Sunflowers in a field in summer (© Arsgera/Shutterstock)](https://cn.bing.com/th?id=OHR.HappySunflower_EN-US8791544241_UHD.jpg)"
    
    if !line.contains(" | [") || !line.contains("](") {
        return Ok(None);
    }
    
    let parts: Vec<&str> = line.split(" | ").collect();
    if parts.len() != 2 {
        return Ok(None);
    }
    
    let date_str = parts[0].trim();
    let bracket_content = parts[1];
    
    // Extract title and copyright from bracket content
    if let Some(start) = bracket_content.find('[') {
        if let Some(end) = bracket_content.find("](") {
            let title_and_copyright = &bracket_content[start + 1..end];
            if let Some(url_start) = bracket_content.find("](") {
                if let Some(url_end) = bracket_content.rfind(')') {
                    let full_url = &bracket_content[url_start + 2..url_end];
                    
                    // Extract title and copyright
                    let (title, copyright) = if let Some(copyright_start) = title_and_copyright.rfind(" (") {
                        let title = title_and_copyright[..copyright_start].trim();
                        let copyright = title_and_copyright[copyright_start + 2..].trim_end_matches(')');
                        (title, copyright)
                    } else {
                        (title_and_copyright, "")
                    };
                    
                    // Extract display_name and imagecode from URL
                    // URL example: https://cn.bing.com/th?id=OHR.HappySunflower_EN-US8791544241_UHD.jpg
                    let display_name = if let Some(id_part) = full_url.split("id=").nth(1) {
                        if let Some(name_part) = id_part.split('_').next() {
                            name_part.to_string()
                        } else {
                            "OHR.Unknown".to_string()
                        }
                    } else {
                        "OHR.Unknown".to_string()
                    };
                    
                    let imagecode = if let Some(id_part) = full_url.split("id=").nth(1) {
                        if let Some(code_part) = id_part.split('_').nth(1) {
                            if let Some(code) = code_part.split('_').next() {
                                code.to_string()
                            } else {
                                "EN-US0000000000".to_string()
                            }
                        } else {
                            "EN-US0000000000".to_string()
                        }
                    } else {
                        "EN-US0000000000".to_string()
                    };
                    
                    // Parse date
                    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                        .context("Failed to parse date")?;
                    
                    let startdate = date.format("%Y%m%d").to_string();
                    let fullstartdate = format!("{}0300", startdate);
                    let next_date = date + Duration::days(1);
                    let _enddate = next_date.format("%Y%m%d").to_string();
                    
                    // Generate URLs
                    let url = format!("/th?id={}_{}_1920x1080.jpg&pid=hp", display_name, imagecode);
                    let _urlbase = format!("/th?id={}", display_name);
                    
                    // Generate copyright link
                    let title_query = title.to_lowercase().replace(' ', "+");
                    let copyrightlink = format!(
                        "https://www.bing.com/search?q={}&form=hpcapt&filters=HpDate%3A%22{}_0700%22",
                        title_query, startdate
                    );
                    
                    return Ok(Some(HistoricalImage {
                        fullstartdate,
                        url,
                        copyright: format!("{}", copyright),
                        copyrightlink,
                        title: title.to_string(),
                    }));
                }
            }
        }
    }
    
    Ok(None)
}

/// Load historical metadata from file
pub fn load_historical_metadata(config: &Config) -> Result<(usize, Vec<HistoricalImage>)> {
    let content = fs::read_to_string(&config.historical_metadata_file)?;
    let lines: Vec<&str> = content.lines().collect();
    
    let current_page = if lines.is_empty() {
        0
    } else {
        lines[0].parse::<usize>().unwrap_or(0)
    };
    
    let mut historical_images = Vec::new();
    for line in lines.iter().skip(1) {
        if !line.trim().is_empty() {
            #[cfg(feature = "serde")]
            {
                if let Ok(image) = serde_json::from_str::<HistoricalImage>(line) {
                    historical_images.push(image);
                }
            }
            #[cfg(not(feature = "serde"))]
            {
                // Without serde, we can't parse stored historical data
                // This is expected when serde feature is disabled
            }
        }
    }
    
    Ok((current_page, historical_images))
}

/// Get next historical page data
pub fn get_next_historical_page(config: &Config, thumb_mode: bool) -> Result<Option<Vec<HistoricalImage>>> {
    let (current_page, existing_images) = load_historical_metadata(config)?;
    
    // Calculate total pages from existing data
    let total_pages = existing_images.len() / 8 + if existing_images.len() % 8 > 0 { 1 } else { 0 };
    
    // Next page to show
    let next_page = current_page + 1;
    
    // Check if we have more pages available
    if next_page >= total_pages {
        return Ok(None);
    }
    
    // Get next page data from existing images (0-based indexing)
    let start_index = next_page * 8;
    let end_index = (start_index + 8).min(existing_images.len());
    
    if start_index >= existing_images.len() {
        return Ok(None);
    }
    
    let page_images = &existing_images[start_index..end_index];
    let mut downloaded_images = Vec::new();
    
    // Download images for this page
    for new_image in page_images {
        let mut display_name = new_image.url
            .split("th?id=")
            .nth(1)
            .and_then(|s| s.split('_').next())
            .unwrap_or(&new_image.title)
            .to_string();
        display_name = sanitize_filename(&display_name);

        let filename_suffix = if thumb_mode { "_thumb.jpg" } else { ".jpg" };
        let target_dir = if thumb_mode { &config.cached_dir } else { &config.unprocessed_dir };
        
        let target_path = target_dir.join(format!("{}{}", display_name, filename_suffix));
        let keepfavorite_path = config.keepfavorite_dir.join(format!("{}{}", display_name, filename_suffix));
        
        if !target_path.exists() && !keepfavorite_path.exists() && !is_blacklisted(config, &display_name)? {
            // Convert HistoricalImage to BingImage for download
            let bing_image = BingImage {
                url: new_image.url.clone(),
                title: new_image.title.clone(),
                copyright: Some(new_image.copyright.clone()),
                copyrightlink: Some(new_image.copyrightlink.clone()),
            };
            
            let download_result = if thumb_mode {
                download_thumbnail_image(&bing_image, config)
            } else {
                download_image(&bing_image, &config.unprocessed_dir, config)
            };
            
            match download_result {
                Ok(filepath) => {
                    let download_type = if thumb_mode { "historical thumbnail" } else { "historical image" };
                    println!("Downloaded {}: {}", download_type, filepath.display());
                }
                Err(e) => {
                    let download_type = if thumb_mode { "historical thumbnail" } else { "historical image" };
                    eprintln!("Failed to download {} {}: {}", download_type, display_name, e);
                }
            }
        } else {
            let download_type = if thumb_mode { "historical thumbnail" } else { "historical image" };
            println!("Skipping already downloaded or blacklisted {}: {}", download_type, display_name);
        }
        
        downloaded_images.push(new_image.clone());

        // Save metadata for this image
        let sanitized_name = sanitize_filename(&display_name);
        if let Err(e) = save_image_metadata(config, &sanitized_name, &new_image.copyright, &new_image.copyrightlink) {
            eprintln!("Failed to save metadata for {}: {}", sanitized_name, e);
        }
    }

    // Update current page to next page in the metadata file
    let mut metadata_content = format!("{}\n", next_page);
    for image in &existing_images {
        #[cfg(feature = "serde")]
        {
            metadata_content.push_str(&format!("{}\n", serde_json::to_string(image)?));
        }
        #[cfg(not(feature = "serde"))]
        {
            metadata_content.push_str(&format!("{:?}\n", image));
        }
    }
    fs::write(&config.historical_metadata_file, metadata_content)?;

    Ok(Some(downloaded_images))
}

/// Download more historical data when current data is exhausted
#[cfg(not(target_arch = "wasm32"))]
pub fn download_more_historical_data(config: &Config) -> Result<Vec<HistoricalImage>> {
    // Load current metadata to check existing images count
    let (current_page, existing_images) = load_historical_metadata(config)?;
    let total_existing = existing_images.len();
    
    // If we already have a lot of images but they're exhausted, try to fetch newer data
    if total_existing > 0 {
        info!("Attempting to fetch additional historical data beyond {} existing images", total_existing);
        
        // Try to fetch from the GitHub repository again to get any new data
        let url = "https://raw.githubusercontent.com/v5tech/bing-wallpaper/refs/heads/main/bing-wallpaper.md";
        let response = run_async(async move {
            reqwest::Client::new()
                .get(url)
                .timeout(std::time::Duration::from_secs(30))
                .header("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")
                .send()
                .await
        })?;
        let content = run_async(async { response.text().await })?;
        
        let lines: Vec<&str> = content.lines().collect();
        let mut new_historical_images = Vec::new();
        
        // Parse all historical data
        for line in lines.iter() {
            if let Some(historical_image) = parse_historical_line(line)? {
                new_historical_images.push(historical_image);
            }
        }
        
        // Filter out images we already have
        let existing_urls: std::collections::HashSet<String> = existing_images.iter().map(|img| img.url.clone()).collect();
        let truly_new_images: Vec<HistoricalImage> = new_historical_images.into_iter()
            .filter(|img| !existing_urls.contains(&img.url))
            .collect();
        
        if !truly_new_images.is_empty() {
            // Add new images to the existing set
            let mut all_images = existing_images;
            all_images.extend(truly_new_images.clone());
            
            // Update metadata file with all images
            let mut metadata_content = format!("{}\n", current_page);
            for image in &all_images {
                #[cfg(feature = "serde")]
        {
            metadata_content.push_str(&format!("{}\n", serde_json::to_string(image)?));
        }
        #[cfg(not(feature = "serde"))]
        {
            metadata_content.push_str(&format!("{:?}\n", image));
        }
            }
            fs::write(&config.historical_metadata_file, metadata_content)?;
            
            info!("Added {} new historical images", truly_new_images.len());
            return Ok(truly_new_images.into_iter().rev().take(8).collect());
        } else {
            // Try to return older images if no new ones are available
            if total_existing > current_page * 8 {
                let start_index = current_page * 8;
                let end_index = (start_index + 8).min(total_existing);
                let older_images = existing_images[start_index..end_index].to_vec();
                
                // Update page counter
                let new_page = current_page + 1;
                let mut metadata_content = format!("{}\n", new_page);
                for image in &existing_images {
                    #[cfg(feature = "serde")]
        {
            metadata_content.push_str(&format!("{}\n", serde_json::to_string(image)?));
        }
        #[cfg(not(feature = "serde"))]
        {
            metadata_content.push_str(&format!("{:?}\n", image));
        }
                }
                fs::write(&config.historical_metadata_file, metadata_content)?;
                
                info!("Serving {} older historical images from local cache", older_images.len());
                return Ok(older_images);
            }
        }
    }
    
    Err(std::io::Error::new(std::io::ErrorKind::NotFound, "No more historical data available").into())
}

/// Get historical data page count information
pub fn get_historical_page_info(config: &Config) -> Result<(usize, usize)> {
    // if no historical metadata file, run download_historical_image
    if !config.historical_metadata_file.exists() {
        return Ok((0, 0));
    }
    
    let (current_page, existing_images) = load_historical_metadata(config)?;
    let total_pages = existing_images.len() / 8 + if existing_images.len() % 8 > 0 { 1 } else { 0 };
    
    Ok((current_page, total_pages))
}





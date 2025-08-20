use anyhow::{Context, Result};
use chrono::{Utc, NaiveDate, Duration};
use std::collections::HashMap;
use std::fs;
#[cfg(feature = "serde")]
use serde_json;
#[cfg(not(target_arch = "wasm32"))]
use log::{info, error};

use crate::core::storage::Config;
use crate::core::request::{BingImage, HistoricalImage};
#[cfg(not(target_arch = "wasm32"))]
use crate::core::request::get_market_codes;
#[cfg(not(target_arch = "wasm32"))]
use crate::run_async;

#[cfg(not(target_arch = "wasm32"))]
pub fn load_market_codes(config: &Config) -> Result<HashMap<String, i64>> {
    info!("load_market_codes: Checking if marketcodes file exists: {:?}", config.marketcodes_file);
    if !config.marketcodes_file.exists() {
        info!("load_market_codes: marketcodes.conf doesn't exist, fetching from web");
        let codes = get_market_codes()?;
        info!("load_market_codes: Got {} market codes from web", codes.len());
        let mut market_map = HashMap::new();
        for code in codes {
            market_map.insert(code, 0);
        }
        info!("load_market_codes: Saving {} market codes to file", market_map.len());
        match save_market_codes(config, &market_map) {
            Ok(()) => info!("load_market_codes: Successfully saved market codes"),
            Err(e) => error!("load_market_codes: Failed to save market codes: {}", e),
        }
        return Ok(market_map);
    }
    
    info!("load_market_codes: marketcodes.conf exists, reading content");
    let content = fs::read_to_string(&config.marketcodes_file)?;
    info!("load_market_codes: Read {} bytes from marketcodes.conf", content.len());
    let mut market_map = HashMap::new();
    
    for line in content.lines() {
        if let Some((code, timestamp)) = line.split_once('|') {
            if let Ok(ts) = timestamp.parse::<i64>() {
                market_map.insert(code.to_string(), ts);
            }
        }
    }
    
    info!("load_market_codes: Loaded {} market codes from file", market_map.len());
    Ok(market_map)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_market_codes(config: &Config, market_codes: &HashMap<String, i64>) -> Result<()> {
    info!("save_market_codes: Saving {} market codes to {:?}", market_codes.len(), config.marketcodes_file);
    let mut content = String::new();
    for (code, timestamp) in market_codes {
        content.push_str(&format!("{}|{}\n", code, timestamp));
    }
    match fs::write(&config.marketcodes_file, &content) {
        Ok(()) => {
            info!("save_market_codes: Successfully wrote {} bytes to marketcodes.conf", content.len());
            Ok(())
        },
        Err(e) => {
            error!("save_market_codes: Failed to write to marketcodes.conf: {}", e);
            Err(e.into())
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn save_market_codes(_config: &Config, _market_codes: &HashMap<String, i64>) -> Result<()> {
    // In WASM, market codes are saved via SqliteDb in the wasm module
    Ok(())
}

pub fn is_blacklisted(config: &Config, filename: &str) -> Result<bool> {
    let blacklist = fs::read_to_string(&config.blacklist_file).unwrap_or_default();
    println!("Checking if {} is blacklisted : {}", filename, blacklist.lines().any(|line| line.trim() == filename));
    Ok(blacklist.lines().any(|line| line.trim() == filename))
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
    let content = run_async(async { response?.text().await })??;
    
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
    
    #[allow(unused_mut)]
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
#[cfg(not(target_arch = "wasm32"))]
pub fn get_next_historical_page(config: &Config, thumb_mode: bool) -> Result<Option<Vec<HistoricalImage>>> {
    use crate::core::storage::{download_image, download_thumbnail_image, sanitize_filename};
    
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

// WASM functions for SQLite and HTTP operations
#[cfg(target_arch = "wasm32")]
pub fn load_market_codes(_config: &Config) -> Result<HashMap<String, i64>> {
    // In WASM, market codes are loaded via SqliteDb in the wasm module
    // This function is kept for compatibility but should use wasm::SqliteDb
    Ok(HashMap::new())
}

#[cfg(target_arch = "wasm32")]
pub fn download_images_for_market(_config: &Config, _market_code: &str, _thumb_mode: bool) -> Result<(usize, Vec<BingImage>)> {
    // In WASM, images are downloaded via HttpClient in the wasm module
    // This function is kept for compatibility but should use wasm::HttpClient
    Ok((0, Vec::new()))
}

#[cfg(target_arch = "wasm32")]
pub fn download_historical_data(_config: &Config, _starting_index: usize) -> Result<Vec<HistoricalImage>> {
    // In WASM, use HttpClient::download_historical_data instead
    Ok(Vec::new())
}

#[cfg(target_arch = "wasm32")]
pub fn download_image(_image: &BingImage, _target_dir: &std::path::Path, _config: &Config) -> Result<std::path::PathBuf> {
    // In WASM, images are not downloaded to filesystem but handled via HttpClient::download_image_bytes
    use std::path::PathBuf;
    Ok(PathBuf::from("/tmp/placeholder.jpg"))
}

#[cfg(target_arch = "wasm32")]
pub fn download_thumbnail_image(_image: &BingImage, _config: &Config) -> Result<std::path::PathBuf> {
    // In WASM, thumbnails are not downloaded to filesystem but handled via HttpClient::download_thumbnail_bytes
    use std::path::PathBuf;
    Ok(PathBuf::from("/tmp/placeholder_thumb.jpg"))
}

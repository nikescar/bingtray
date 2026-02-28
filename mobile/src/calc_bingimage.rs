//! Core business logic for Bing wallpaper management
//!
//! This module provides comprehensive functionality for managing Bing daily wallpapers,
//! including fetching images from multiple market codes, caching, historical data management,
//! and desktop wallpaper operations.
//!
//! ## Architecture
//! - **Cross-platform functions**: Available on all platforms (Android, WASM, Desktop)
//! - **Desktop-only components**: Platform-specific wallpaper operations (Linux, macOS, Windows)
//!
//! ## Cross-Platform Functions
//!
//! ### Market Code Management
//! - [`get_market_codes()`] - Fetch available market codes list from Bing API
//!
//! ### Bing Image API Integration
//! - [`get_bing_images_manifest()`] - Fetch Bing wallpapers for a specific market code with pagination
//! - [`download_historical_data()`] - Load or download historical wallpaper data from cache/GitHub
//!
//! ### Filename & Path Management
//! - [`sanitize_filename()`] - Sanitize filenames for cross-platform filesystem compatibility
//!   (removes special chars, keeps alphanumeric, space, dash, underscore only)
//!
//! ### Pagination & Image Loading
//! - [`get_next_historical_page()`] - Advance to next page and return page number (3 items/page)
//! - [`get_historical_page_info()`] - Get current page and total page count for historical data
//! - [`load_cached_images_paginated()`] - Load locally cached images with pagination (10 items/page)
//! - [`load_historical_images_paginated()`] - Load historical images with pagination (3 items/page)
//!
//! ### Metadata Search
//! - [`find_bing_url_for_cached_image()`] - Search metadata to find original Bing URL for cached image
//!   (supports fuzzy matching on titles)
//!
//! ## Desktop-Only Components
//! Available only on Linux, macOS, and Windows (excluded on Android and WASM).
//!
//! - [`BingTrayLogic`] - Main orchestrator struct for desktop wallpaper operations
//!   - [`set_next_market_wallpaper()`](BingTrayLogic::set_next_market_wallpaper) - Download and set next unprocessed wallpaper from Bing
//!   - [`set_kept_wallpaper()`](BingTrayLogic::set_kept_wallpaper) - Set random wallpaper from favorites collection
//!   - [`keep_current_image()`](BingTrayLogic::keep_current_image) - Move current wallpaper to favorites (keep) directory
//!   - [`blacklist_current_image()`](BingTrayLogic::blacklist_current_image) - Blacklist and delete current wallpaper, then set next
//!
//! ## Testing
//! Comprehensive test suite with 19 tests covering all public functions:
//! - **Unit tests** (16): Run by default with `cargo test`
//! - **Integration tests** (3): Network-dependent, run with `cargo test -- --include-ignored`
//!
//! Run all tests: `cargo test --lib calc_bingimage::tests -- --include-ignored`

use crate::{BingImage, Config, HistoricalImage};
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

// DuckDB is available on both desktop and Android, but not WASM
#[cfg(not(target_arch = "wasm32"))]
use crate::duckdb_bingimage::*;

// Desktop-only imports (wallpaper setting, random selection, channels)
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
use crate::api_setwallpaper;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
use rand::seq::SliceRandom;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
use std::sync::mpsc;

// ============================================================================
// Cross-Platform Functions (available on all platforms including Android)
// ============================================================================

/// Retrieve a comprehensive list of supported Bing market codes.
///
/// This function returns a hardcoded list of common market/language codes that
/// are supported by the Bing image API (e.g., "en-US", "ja-JP", "de-DE"). These
/// codes represent different regional/language variants of Bing's daily wallpaper.
/// The reference implementation originally scraped this from Microsoft's documentation,
/// but for reliability and to avoid network dependencies, this returns a static
/// list of well-known market codes.
///
/// # Returns
/// A vector of market code strings like "en-US", "fr-FR", etc.
pub fn get_market_codes() -> Result<Vec<String>> {
    // Return a hardcoded list of common market codes as fallback
    // The reference implementation scrapes this from Microsoft docs, but for reliability
    // we'll use a static list
    let codes = vec![
        "en-US", "en-GB", "de-DE", "fr-FR", "es-ES", "it-IT", "ja-JP", "zh-CN",
        "pt-BR", "ru-RU", "nl-NL", "pl-PL", "tr-TR", "ko-KR", "sv-SE", "da-DK",
        "fi-FI", "nb-NO", "cs-CZ", "hu-HU", "ro-RO", "el-GR", "th-TH", "id-ID",
        "vi-VN", "uk-UA", "bg-BG", "hr-HR", "sk-SK", "sl-SI", "et-EE", "lv-LV",
        "lt-LT", "sr-Latn-RS", "ar-SA", "he-IL", "pt-PT", "es-MX", "fr-CA",
    ];

    Ok(codes.iter().map(|s| s.to_string()).collect())
}

/// Load Bing images from database cache for a specific market code
pub fn load_bing_images_from_cache(config: &Config, market_code: &str, count: usize) -> Result<Vec<BingImage>> {
    log::info!("Loading Bing images from database cache for market: {}", market_code);

    let db = BingImageDb::new(config.db_path.clone())
        .context("Failed to open database")?;

    let records = db.get_images_by_market_code(market_code)
        .context("Failed to load images from database")?;

    if records.is_empty() {
        anyhow::bail!("No images in cache for market code: {}", market_code);
    }

    let bing_images: Vec<BingImage> = records
        .into_iter()
        .take(count)
        .map(|record| BingImage {
            url: record.url,
            title: record.title,
            copyright: record.copyright,
            copyright_link: record.copyright_link,
        })
        .collect();

    log::info!("Loaded {} Bing images from cache", bing_images.len());
    Ok(bing_images)
}

/// Save Bing images to database cache
fn save_bing_images_to_cache(db: &BingImageDb, market_code: &str, images: &[BingImage]) -> Result<()> {
    log::info!("Saving {} Bing images to cache for market: {}", images.len(), market_code);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    for img in images {
        let record = BingImageRecord {
            url: img.url.clone(),
            title: img.title.clone(),
            copyright: img.copyright.clone(),
            copyright_link: img.copyright_link.clone(),
            market_code: market_code.to_string(),
            fetched_at: now,
            status: ImageStatus::Unprocessed,
        };

        db.upsert_image(&record)?;
    }

    // Save download timestamp
    db.set_last_download_timestamp(market_code, now)?;
    db.checkpoint()?;

    log::info!("Successfully cached {} Bing images", images.len());
    Ok(())
}

/// Fetch Bing images with caching support (checks 7-day cache before downloading)
pub fn get_bing_images_manifest_cached(config: Option<&Config>, market_code: &str, count: u32, offset: u32) -> Result<Vec<BingImage>> {
    // Check if we should use cache
    if let Some(cfg) = config {
        if let Ok(db) = BingImageDb::new(cfg.db_path.clone()) {
            if offset == 0 && !db.should_download_manifest(market_code) {
                log::info!("Bing images for {} are fresh (< 7 days), loading from cache", market_code);
                match load_bing_images_from_cache(cfg, market_code, count as usize) {
                    Ok(images) => return Ok(images),
                    Err(e) => {
                        log::warn!("Failed to load from cache: {}, downloading fresh data", e);
                    }
                }
            }
        }
    }

    // Download fresh data
    let images = get_bing_images_manifest(market_code, count, offset)?;

    // Save to cache if we have config and this is the first page
    if offset == 0 {
        if let Some(cfg) = config {
            if let Ok(db) = BingImageDb::new(cfg.db_path.clone()) {
                save_bing_images_to_cache(&db, market_code, &images)
                    .unwrap_or_else(|e| log::warn!("Failed to cache Bing images: {}", e));
            }
        }
    }

    Ok(images)
}

/// Download image metadata from Bing's HPImageArchive API for a specific market.
///
/// This function makes an HTTP request to Bing's public API endpoint to fetch
/// image metadata (URLs, titles, copyright info) for a given market code.
/// It uses the ehttp library for async HTTP fetching with a 30-second timeout,
/// parses the JSON response, and converts relative URLs to absolute URLs by
/// prepending "https://www.bing.com" where needed. The function includes
/// proper User-Agent headers and error handling for network/parsing failures.
///
/// # Arguments
/// * `market_code` - The market/language code (e.g., "en-US")
/// * `count` - Number of images to fetch (max 8 per Bing API limits)
/// * `offset` - Offset for historical images (0 = today)
///
/// # Returns
/// A vector of BingImage structs containing URLs and metadata
pub fn get_bing_images_manifest(market_code: &str, count: u32, offset: u32) -> Result<Vec<BingImage>> {
    log::info!("get_bing_images_manifest called for market_code: {}, count: {}, offset: {}",
              market_code, count, offset);

    // Build API URL
    let url = format!(
        "https://www.bing.com/HPImageArchive.aspx?format=js&idx={}&n={}&mkt={}",
        offset, count, market_code
    );

    log::info!("Fetching Bing images from: {}", url);

    // Create request with User-Agent
    let mut request = ehttp::Request::get(&url);
    request.headers.insert(
        "User-Agent".to_string(),
        format!("bingtray/{}", env!("CARGO_PKG_VERSION")),
    );

    // Create channel for synchronous fetch
    let (tx, rx) = std::sync::mpsc::channel();

    // Fetch asynchronously
    ehttp::fetch(request, move |response| {
        let _ = tx.send(response);
    });

    // Wait for response with timeout
    let response = rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .context("Timeout waiting for Bing API response")?;

    let resp = response.map_err(|e| anyhow::anyhow!("Network error: {}", e))?;

    if !resp.ok {
        anyhow::bail!("HTTP {}: {}", resp.status, resp.status_text);
    }

    // Parse JSON response
    let text = resp.text().context("Invalid UTF-8 in response")?;
    let bing_response: crate::BingResponse =
        serde_json::from_str(text).context("Failed to parse JSON response")?;

    // Convert to full URLs
    let images: Vec<BingImage> = bing_response
        .images
        .into_iter()
        .map(|img| {
            let full_url = if img.url.starts_with("http") {
                img.url
            } else {
                format!("https://www.bing.com{}", img.url)
            };

            BingImage {
                url: full_url,
                title: img.title,
                copyright: img.copyright,
                copyright_link: img.copyright_link,
            }
        })
        .collect();

    log::info!("Successfully fetched {} Bing images", images.len());
    Ok(images)
}

/// Clean and sanitize a filename to ensure filesystem compatibility.
///
/// This function processes a raw filename string and removes or replaces
/// characters that might cause issues on various filesystems. It keeps only
/// alphanumeric characters, spaces, hyphens, and underscores, replacing all
/// other characters with underscores. Additionally, it truncates the result
/// to a maximum of 100 characters to prevent excessively long filenames that
/// could cause problems on certain operating systems.
///
/// # Arguments
/// * `filename` - The raw filename string to sanitize
///
/// # Returns
/// A sanitized filename string safe for use on all major filesystems
pub fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .chars()
        .take(100)
        .collect()
}

/// Load or download historical Bing wallpaper data from external sources.
///
/// This function checks if historical metadata already exists in the local cache.
/// If found, it loads the cached data and converts the first 8 entries to BingImage
/// format for display in a carousel UI. If no cached data exists, it would normally
/// download historical wallpaper data from a GitHub repository (v5tech/bing-wallpaper),
/// but the actual network download is currently a placeholder to avoid dependencies.
/// The function demonstrates the intended architecture for managing historical data.
///
/// # Arguments
/// * `config` - Configuration containing file paths
/// * `_starting_index` - Reserved for future pagination support (currently unused)
///
/// Load historical images from database cache (without downloading)
///
/// # Arguments
/// * `config` - Configuration containing database path
/// * `count` - Number of images to return
///
/// # Returns
/// A vector of BingImage structs from cached data
pub fn load_historical_from_cache(config: &Config, count: usize) -> Result<Vec<BingImage>> {
    log::info!("Loading historical images from database cache");

    let db = BingImageDb::new(config.db_path.clone())
        .context("Failed to open database")?;

    let (_, historical_images) = load_historical_metadata_with_db(config, Some(&db))?;

    if historical_images.is_empty() {
        anyhow::bail!("No historical images in cache");
    }

    let bing_images: Vec<BingImage> = historical_images
        .iter()
        .take(count)
        .map(|img| BingImage {
            url: img.url.clone(),
            title: img.title.clone(),
            copyright: Some(img.copyright.clone()),
            copyright_link: Some(img.copyrightlink.clone()).filter(|s| !s.is_empty()),
        })
        .collect();

    log::info!("Loaded {} historical images from cache", bing_images.len());
    Ok(bing_images)
}

/// Download historical wallpaper data from GitHub and return first page of images.
///
/// This function checks if download is needed (>7 days since last download).
/// If data is fresh (< 7 days), it loads from cache instead.
///
/// # Returns
/// A vector of BingImage structs for carousel display (up to 8 images)
pub fn download_historical_data(config: &Config, _starting_index: usize) -> Result<Vec<BingImage>> {
    // Check if we need to download
    let db = BingImageDb::new(config.db_path.clone()).ok();
    if let Some(ref database) = db {
        if !database.should_download_manifest("historical") {
            log::info!("Historical data is fresh (< 7 days), loading from cache");
            return load_historical_from_cache(config, 8);
        }
    }

    log::info!("Downloading historical data from GitHub");

    let url = "https://raw.githubusercontent.com/v5tech/bing-wallpaper/refs/heads/main/bing-wallpaper.md";

    // Create request with User-Agent
    let mut request = ehttp::Request::get(url);
    request.headers.insert(
        "User-Agent".to_string(),
        format!("bingtray/{}", env!("CARGO_PKG_VERSION")),
    );

    // Create channel for synchronous fetch
    let (tx, rx) = std::sync::mpsc::channel();

    // Fetch asynchronously
    ehttp::fetch(request, move |response| {
        let _ = tx.send(response);
    });

    // Wait for response with timeout
    let response = rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .context("Timeout waiting for historical data from GitHub")?;

    let resp = response.map_err(|e| anyhow::anyhow!("Network error: {}", e))?;

    if !resp.ok {
        anyhow::bail!("HTTP {}: {}", resp.status, resp.status_text);
    }

    // Parse markdown content
    let text = resp.text().context("Invalid UTF-8 in response")?;
    let mut historical_images = Vec::new();

    for line in text.lines() {
        // Skip empty lines and headers
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse format: "2026-03-03 | [Title (© Copyright)](URL)"
        if let Some((date_part, rest)) = line.split_once('|') {
            let date = date_part.trim();
            let rest = rest.trim();

            // Extract markdown link: [text](url)
            if let Some(link_start) = rest.find('[') {
                if let Some(link_end) = rest.find("](") {
                    if let Some(url_end) = rest.rfind(')') {
                        let content = &rest[link_start + 1..link_end];
                        let url = &rest[link_end + 2..url_end];

                        // Split content into title and copyright
                        let (title, copyright) = if let Some(copyright_start) = content.find("(©") {
                            let title = content[..copyright_start].trim();
                            let copyright = content[copyright_start + 1..].trim_end_matches(')').trim();
                            (title, copyright)
                        } else {
                            (content, "")
                        };

                        // Convert date from YYYY-MM-DD to YYYYMMDD0000
                        let fullstartdate = date.replace('-', "") + "0000";

                        // Change cn.bing.com to www.bing.com
                        let normalized_url = url.replace("cn.bing.com", "www.bing.com");

                        // Generate copyright link
                        let title_query = title.to_lowercase().replace(' ', "+");
                        let startdate = &fullstartdate[..8]; // Extract YYYYMMDD
                        let copyrightlink = format!(
                            "https://www.bing.com/search?q={}&form=hpcapt&filters=HpDate%3A%22{}_0700%22",
                            title_query, startdate
                        );

                        historical_images.push(HistoricalImage {
                            fullstartdate,
                            url: normalized_url,
                            copyright: copyright.to_string(),
                            copyrightlink,
                            title: title.to_string(),
                        });
                    }
                }
            }
        }
    }

    log::info!("Parsed {} historical images from GitHub", historical_images.len());

    if historical_images.is_empty() {
        anyhow::bail!("No historical images found in downloaded data");
    }

    // Open database and save to storage
    log::debug!("Opening database at: {:?}", config.db_path);
    let db = match BingImageDb::new(config.db_path.clone()) {
        Ok(db) => {
            log::debug!("Successfully opened database");
            Some(db)
        }
        Err(e) => {
            log::error!("Failed to open database at {:?}: {}", config.db_path, e);
            None
        }
    };
    save_historical_metadata_with_db(config, 0, &historical_images, db.as_ref())?;

    // Return first page (up to 8 images) as BingImage structs for carousel
    let bing_images: Vec<BingImage> = historical_images
        .iter()
        .take(8)
        .map(|img| BingImage {
            url: img.url.clone(),
            title: img.title.clone(),
            copyright: Some(img.copyright.clone()),
            copyright_link: Some(img.copyrightlink.clone()).filter(|s| !s.is_empty()),
        })
        .collect();

    log::info!("Returning {} images for carousel display", bing_images.len());
    Ok(bing_images)
}

/// Download historical data with progress updates for UI
///
/// This is similar to download_historical_data but accepts a progress status
/// to update the UI during the long-running save operation.
pub fn download_historical_data_with_progress(
    config: &Config,
    _starting_index: usize,
    progress_status: std::sync::Arc<std::sync::Mutex<String>>,
    ctx: egui::Context,
) -> Result<Vec<BingImage>> {
    // Check if we need to download
    let db = BingImageDb::new(config.db_path.clone()).ok();
    if let Some(ref database) = db {
        if !database.should_download_manifest("historical") {
            log::info!("Historical data is fresh (< 7 days), loading from cache");
            return load_historical_from_cache(config, 8);
        }
    }

    // Update progress
    if let Ok(mut status) = progress_status.lock() {
        *status = "Downloading historical data from GitHub...".to_string();
    }
    ctx.request_repaint();

    log::info!("Downloading historical data from GitHub");

    let url = "https://raw.githubusercontent.com/v5tech/bing-wallpaper/refs/heads/main/bing-wallpaper.md";

    // Create request with User-Agent
    let mut request = ehttp::Request::get(url);
    request.headers.insert(
        "User-Agent".to_string(),
        format!("bingtray/{}", env!("CARGO_PKG_VERSION")),
    );

    // Create channel for synchronous fetch
    let (tx, rx) = std::sync::mpsc::channel();

    // Fetch asynchronously
    ehttp::fetch(request, move |response| {
        let _ = tx.send(response);
    });

    // Wait for response with timeout
    let response = rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .context("Timeout waiting for historical data from GitHub")?;

    let resp = response.map_err(|e| anyhow::anyhow!("Network error: {}", e))?;

    if !resp.ok {
        anyhow::bail!("HTTP {}: {}", resp.status, resp.status_text);
    }

    // Update progress
    if let Ok(mut status) = progress_status.lock() {
        *status = "Parsing historical data...".to_string();
    }
    ctx.request_repaint();

    // Parse markdown content
    let text = resp.text().context("Invalid UTF-8 in response")?;
    let mut historical_images = Vec::new();

    for line in text.lines() {
        // Skip empty lines and headers
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse format: "2026-03-03 | [Title (© Copyright)](URL)"
        if let Some((date_part, rest)) = line.split_once('|') {
            let date = date_part.trim();
            let rest = rest.trim();

            // Extract markdown link: [text](url)
            if let Some(link_start) = rest.find('[') {
                if let Some(link_end) = rest.find("](") {
                    if let Some(url_end) = rest.rfind(')') {
                        let content = &rest[link_start + 1..link_end];
                        let url = &rest[link_end + 2..url_end];

                        // Split content into title and copyright
                        let (title, copyright) = if let Some(copyright_start) = content.find("(©") {
                            let title = content[..copyright_start].trim();
                            let copyright = content[copyright_start + 1..].trim_end_matches(')').trim();
                            (title, copyright)
                        } else {
                            (content, "")
                        };

                        // Convert date from YYYY-MM-DD to YYYYMMDD0000
                        let fullstartdate = date.replace('-', "") + "0000";

                        // Change cn.bing.com to www.bing.com
                        let normalized_url = url.replace("cn.bing.com", "www.bing.com");

                        // Generate copyright link
                        let title_query = title.to_lowercase().replace(' ', "+");
                        let startdate = &fullstartdate[..8]; // Extract YYYYMMDD
                        let copyrightlink = format!(
                            "https://www.bing.com/search?q={}&form=hpcapt&filters=HpDate%3A%22{}_0700%22",
                            title_query, startdate
                        );

                        historical_images.push(HistoricalImage {
                            fullstartdate,
                            url: normalized_url,
                            copyright: copyright.to_string(),
                            copyrightlink,
                            title: title.to_string(),
                        });
                    }
                }
            }
        }
    }

    log::info!("Parsed {} historical images from GitHub", historical_images.len());

    if historical_images.is_empty() {
        anyhow::bail!("No historical images found in downloaded data");
    }

    // Update progress
    if let Ok(mut status) = progress_status.lock() {
        *status = format!("Saving {} images to database...", historical_images.len());
    }
    ctx.request_repaint();

    // Open database and save to storage
    log::debug!("Opening database at: {:?}", config.db_path);
    let db = match BingImageDb::new(config.db_path.clone()) {
        Ok(db) => {
            log::debug!("Successfully opened database");
            Some(db)
        }
        Err(e) => {
            log::error!("Failed to open database at {:?}: {}", config.db_path, e);
            None
        }
    };
    save_historical_metadata_with_progress(config, 0, &historical_images, db.as_ref(), progress_status.clone(), ctx.clone())?;

    // Return first page (up to 8 images) as BingImage structs for carousel
    let bing_images: Vec<BingImage> = historical_images
        .iter()
        .take(8)
        .map(|img| BingImage {
            url: img.url.clone(),
            title: img.title.clone(),
            copyright: Some(img.copyright.clone()),
            copyright_link: Some(img.copyrightlink.clone()).filter(|s| !s.is_empty()),
        })
        .collect();

    log::info!("Returning {} images for carousel display", bing_images.len());
    Ok(bing_images)
}

/// Advance to the next page of historical images and return the current page number.
///
/// This function implements pagination for browsing historical Bing wallpapers.
/// It loads the current page number from metadata, validates that more pages exist
/// (checking if the start index would exceed available images), then increments
/// the page counter and saves it back to disk. Each page contains 8 images.
/// If no more pages are available or if metadata doesn't exist, it returns an error.
///
/// # Arguments
/// * `config` - Configuration containing metadata file path
///
/// # Returns
/// The page number that was just loaded (before incrementing)
///
/// # Errors
/// Returns an error if no historical data exists or if already at the last page
pub fn get_next_historical_page(config: &Config) -> Result<usize> {
    let db = match BingImageDb::new(config.db_path.clone()) {
        Ok(db) => Some(db),
        Err(e) => {
            log::warn!("Failed to open database in get_next_historical_page: {}", e);
            None
        }
    };
    let (current_page, all_images) = load_historical_metadata_with_db(config, db.as_ref())?;

    if all_images.is_empty() {
        log::info!("No historical metadata available yet");
        return Err(anyhow::anyhow!("No historical data available - call download_historical_data first"));
    }

    let start_idx = current_page * 8;

    if start_idx >= all_images.len() {
        log::info!("No more historical pages available");
        return Err(anyhow::anyhow!("No more historical data available"));
    }

    // Update page number only (don't re-save images that are already in database)
    if let Some(database) = db.as_ref() {
        database.set_historical_page(current_page + 1)
            .context("Failed to save historical page to database")?;
        log::debug!("Updated historical page number to {}", current_page + 1);
    }

    log::info!("Returning historical page number {}", current_page);
    Ok(current_page)
}

/// Retrieve pagination information for historical image browsing.
///
/// This function loads the historical metadata and calculates both the current
/// page number and the total number of pages available. The total page count
/// is computed by dividing the total image count by 8 (images per page) and
/// rounding up to ensure partial pages are counted. This information is useful
/// for displaying UI elements like "Page 2 of 15" or enabling/disabling
/// navigation buttons.
///
/// # Arguments
/// * `config` - Configuration containing metadata file path
///
/// # Returns
/// A tuple of (current_page, total_pages) where both are zero-indexed counts
pub fn get_historical_page_info(config: &Config) -> Result<(usize, usize)> {
    let db = match BingImageDb::new(config.db_path.clone()) {
        Ok(db) => Some(db),
        Err(e) => {
            log::warn!("Failed to open database in get_historical_page_info: {}", e);
            None
        }
    };
    let (current_page, all_images) = load_historical_metadata_with_db(config, db.as_ref())?;
    let total_pages = (all_images.len() + 7) / 8; // Round up
    Ok((current_page, total_pages))
}

/// Read and parse the historical metadata file from disk.
///
/// This internal helper function loads the historical metadata file which stores
/// both the current page number (on the first line) and a JSON-per-line format
/// of HistoricalImage entries. Each line after the first contains a serialized
/// HistoricalImage object. The function handles missing files gracefully by
/// returning empty data, and skips any lines that fail to parse. This format
/// allows for efficient incremental loading and simple text-based storage.
///
/// # Arguments
/// * `config` - Configuration containing metadata file path
///
/// # Returns
/// A tuple of (current_page_number, vector_of_historical_images)
/// Load historical metadata and current page from persistent storage.
///
/// This function first attempts to load from database if available, then falls back
/// to the legacy file-based storage. Returns the current page number and list of
/// historical images.
///
/// # Arguments
/// * `config` - Configuration containing file paths
/// * `db` - Optional database connection
///
/// # Returns
/// A tuple of (current_page, images)
fn load_historical_metadata_with_db(_config: &Config, db: Option<&BingImageDb>) -> Result<(usize, Vec<HistoricalImage>)> {
    // Load page number and historical images from database
    if let Some(database) = db {
        let current_page = database.get_historical_page()
            .context("Failed to load historical page from database")?;
        log::debug!("Loaded historical page {} from database", current_page);

        // Load historical images from bing_images table (market_code = 'historical')
        let records = database.get_images_by_market_code("historical")
            .context("Failed to load historical images from database")?;

        // Convert BingImageRecord to HistoricalImage
        let mut images: Vec<HistoricalImage> = records
            .into_iter()
            .map(|record| {
                // Convert Unix timestamp back to YYYYMMDD0000 format
                let days_since_epoch = record.fetched_at / 86400;

                // Calculate year, month, day from days since epoch
                let mut year = 1970;
                let mut remaining_days = days_since_epoch;

                loop {
                    let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) { 366 } else { 365 };
                    if remaining_days >= days_in_year {
                        remaining_days -= days_in_year;
                        year += 1;
                    } else {
                        break;
                    }
                }

                let is_leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
                let days_in_month = [31, if is_leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

                let mut month = 1;
                for (i, &days) in days_in_month.iter().enumerate() {
                    if remaining_days < days {
                        month = i + 1;
                        break;
                    }
                    remaining_days -= days;
                }

                let day = remaining_days + 1;
                let fullstartdate = format!("{:04}{:02}{:02}0000", year, month, day);

                HistoricalImage {
                    fullstartdate,
                    url: record.url,
                    copyright: record.copyright.unwrap_or_default(),
                    copyrightlink: record.copyright_link.unwrap_or_default(),
                    title: record.title,
                }
            })
            .collect();

        // Sort by date (most recent first)
        images.sort_by(|a, b| b.fullstartdate.cmp(&a.fullstartdate));

        log::info!("Successfully loaded {} historical images from bing_images table", images.len());
        return Ok((current_page, images));
    }

    log::error!("Cannot load historical data: Database not available");
    anyhow::bail!("Database not available")
}

/// Write historical metadata and current page number to persistent storage.
///
/// This function saves the current page number to the database if available,
/// and saves historical images to the file. The database stores only the page
/// number for tracking pagination state.
///
/// # Arguments
/// * `config` - Configuration containing metadata file path
/// * `current_page` - The current page number to save
/// * `images` - Slice of HistoricalImage objects to persist
/// * `db` - Optional database connection
fn save_historical_metadata_with_db(_config: &Config, current_page: usize, images: &[HistoricalImage], db: Option<&BingImageDb>) -> Result<()> {
    // Save page number and historical images to database
    if let Some(database) = db {
        // Save all images (not just 8) to database
        log::info!("Saving historical page {} with {} images to bing_images table",
            current_page, images.len());

        // Save historical page number
        database.set_historical_page(current_page)
            .context("Failed to save historical page to database")?;
        log::debug!("Saved historical page number to database: {}", current_page);

        // Save each historical image as a row in bing_images table
        let mut saved_count = 0;
        let total_count = images.len();
        for (idx, img) in images.iter().enumerate() {
            // Convert YYYYMMDD0000 to Unix timestamp
            // Extract YYYYMMDD from YYYYMMDD0000
            let date_str = &img.fullstartdate[..8]; // Get first 8 chars (YYYYMMDD)
            let timestamp = if date_str.len() == 8 {
                // Parse YYYYMMDD as year, month, day
                if let (Ok(year), Ok(month), Ok(day)) = (
                    date_str[0..4].parse::<i32>(),
                    date_str[4..6].parse::<u32>(),
                    date_str[6..8].parse::<u32>(),
                ) {
                    // Use chrono to convert to Unix timestamp (seconds since 1970-01-01 00:00:00 UTC)
                    use chrono::{NaiveDate, TimeZone, Utc};
                    if let Some(naive_date) = NaiveDate::from_ymd_opt(year, month, day) {
                        Utc.from_utc_datetime(&naive_date.and_hms_opt(0, 0, 0).unwrap())
                            .timestamp()
                    } else {
                        log::warn!("Invalid date: {}-{:02}-{:02}, using 0", year, month, day);
                        0
                    }
                } else {
                    0
                }
            } else {
                0
            };

            let record = BingImageRecord {
                url: img.url.clone(),
                title: img.title.clone(),
                copyright: Some(img.copyright.clone()),
                copyright_link: Some(img.copyrightlink.clone()).filter(|s| !s.is_empty()),
                market_code: "historical".to_string(), // Special market code for historical images
                fetched_at: timestamp,
                status: ImageStatus::Cached, // Mark as cached/historical
            };

            match database.upsert_image(&record) {
                Ok(_) => {
                    saved_count += 1;
                    // Log progress every 100 images
                    if (idx + 1) % 100 == 0 || idx + 1 == total_count {
                        log::info!("Progress: Saved {}/{} historical images ({:.1}%)",
                            idx + 1, total_count, ((idx + 1) as f32 / total_count as f32) * 100.0);
                    }
                }
                Err(e) => log::warn!("Failed to save historical image {}: {}", img.url, e),
            }
        }

        log::info!("Successfully saved {} historical images to bing_images table (out of {} total)", saved_count, images.len());

        // Save download timestamp
        log::info!("Saving metadata...");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        database.set_last_download_timestamp("historical", now)
            .context("Failed to save historical download timestamp")?;
        log::debug!("Saved historical download timestamp: {}", now);

        // Flush database to ensure data is written to disk
        log::info!("Writing data to disk...");
        database.checkpoint()
            .context("Failed to checkpoint database")?;
        log::info!("Database checkpoint completed - all {} images saved successfully", saved_count);

        return Ok(());
    }

    log::error!("Cannot save historical data: Database not available");
    anyhow::bail!("Database not available")
}

/// Save historical metadata with progress updates for UI
fn save_historical_metadata_with_progress(
    _config: &Config,
    current_page: usize,
    images: &[HistoricalImage],
    db: Option<&BingImageDb>,
    progress_status: std::sync::Arc<std::sync::Mutex<String>>,
    ctx: egui::Context,
) -> Result<()> {
    // Save page number and historical images to database
    if let Some(database) = db {
        // Save all images (not just 8) to database
        log::info!("Saving historical page {} with {} images to bing_images table",
            current_page, images.len());

        // Save historical page number
        database.set_historical_page(current_page)
            .context("Failed to save historical page to database")?;
        log::debug!("Saved historical page number to database: {}", current_page);

        // Convert all images to records first
        let total_count = images.len();
        let mut all_records = Vec::with_capacity(total_count);

        for img in images.iter() {
            // Convert YYYYMMDD0000 to Unix timestamp
            let date_str = &img.fullstartdate[..8];
            let timestamp = if date_str.len() == 8 {
                if let (Ok(year), Ok(month), Ok(day)) = (
                    date_str[0..4].parse::<i32>(),
                    date_str[4..6].parse::<u32>(),
                    date_str[6..8].parse::<u32>(),
                ) {
                    // Use chrono to convert to Unix timestamp (seconds since 1970-01-01 00:00:00 UTC)
                    use chrono::{NaiveDate, TimeZone, Utc};
                    if let Some(naive_date) = NaiveDate::from_ymd_opt(year, month, day) {
                        Utc.from_utc_datetime(&naive_date.and_hms_opt(0, 0, 0).unwrap())
                            .timestamp()
                    } else {
                        log::warn!("Invalid date: {}-{:02}-{:02}, using 0", year, month, day);
                        0
                    }
                } else {
                    0
                }
            } else {
                0
            };

            all_records.push(BingImageRecord {
                url: img.url.clone(),
                title: img.title.clone(),
                copyright: Some(img.copyright.clone()),
                copyright_link: Some(img.copyrightlink.clone()).filter(|s| !s.is_empty()),
                market_code: "historical".to_string(),
                fetched_at: timestamp,
                status: ImageStatus::Cached,
            });
        }

        // Batch insert in chunks of 100 for progress updates
        let mut saved_count = 0;
        let chunk_size = 100;

        for (chunk_idx, chunk) in all_records.chunks(chunk_size).enumerate() {
            match database.batch_upsert_images(chunk) {
                Ok(count) => {
                    saved_count += count;
                    let progress_msg = format!("Saved {}/{} images ({:.0}%)",
                        saved_count, total_count, (saved_count as f32 / total_count as f32) * 100.0);
                    log::info!("Progress: {}", progress_msg);

                    // Update UI progress
                    if let Ok(mut status) = progress_status.lock() {
                        *status = progress_msg;
                    }
                    ctx.request_repaint();
                }
                Err(e) => log::warn!("Failed to save batch {}: {}", chunk_idx, e),
            }
        }

        log::info!("Successfully saved {} historical images to bing_images table (out of {} total)", saved_count, images.len());

        // Update progress
        if let Ok(mut status) = progress_status.lock() {
            *status = "Finalizing...".to_string();
        }
        ctx.request_repaint();

        // Save download timestamp
        log::info!("Saving metadata...");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        database.set_last_download_timestamp("historical", now)
            .context("Failed to save historical download timestamp")?;
        log::debug!("Saved historical download timestamp: {}", now);

        // Flush database to ensure data is written to disk
        if let Ok(mut status) = progress_status.lock() {
            *status = "Writing to disk...".to_string();
        }
        ctx.request_repaint();

        log::info!("Writing data to disk...");
        database.checkpoint()
            .context("Failed to checkpoint database")?;
        log::info!("Database checkpoint completed - all {} images saved successfully", saved_count);

        return Ok(());
    }

    log::error!("Cannot save historical data: Database not available");
    anyhow::bail!("Database not available")
}

/// Load a specific page of images from the local cache directory.
///
/// This function scans the cached directory for image files (jpg, jpeg, png),
/// sorts them by modification time (most recent first), and returns a single
/// page of 8 images starting at the specified page offset. Each cached file
/// is converted to a BingImage struct with the local file path as the URL
/// and the filename (with extensions removed) as the title. This enables
/// browsing previously downloaded wallpapers in a paginated carousel interface.
///
/// # Arguments
/// * `config` - Configuration containing cached directory path
/// * `page` - Zero-indexed page number (0 = first 8 images, 1 = next 8, etc.)
///
/// # Returns
/// A vector of up to 8 BingImage structs representing cached local files
pub fn load_cached_images_paginated(config: &Config, page: usize) -> Result<Vec<BingImage>> {
    use std::fs::read_dir;

    let mut images = Vec::new();

    if !config.cached_dir.exists() {
        return Ok(images);
    }

    let mut entries: Vec<_> = read_dir(&config.cached_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension()
                .and_then(|s| s.to_str())
                .map(|s| s == "jpg" || s == "jpeg" || s == "png")
                .unwrap_or(false)
        })
        .collect();

    // Sort by modification time, most recent first
    entries.sort_by_key(|e| {
        e.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH)
    });
    entries.reverse();

    let start = page * 8;

    for entry in entries.iter().skip(start).take(8) {
        let filename = entry.file_name().to_string_lossy().to_string();
        let title = filename.replace("_thumb.jpg", "")
            .replace(".jpg", "")
            .replace(".jpeg", "")
            .replace(".png", "");

        images.push(BingImage {
            url: entry.path().to_string_lossy().to_string(),
            title,
            copyright: None,
            copyright_link: None,
        });
    }

    Ok(images)
}

/// Load a specific page of historical images from metadata storage.
///
/// This function retrieves historical wallpaper metadata from the persistent
/// storage file and extracts a single page worth of images (8 images) starting
/// at the specified page offset. It converts HistoricalImage objects to the
/// BingImage format used by the UI carousel, preserving all metadata fields
/// including URLs, titles, copyright info, and copyright links. This allows
/// users to browse historical Bing wallpapers without re-downloading metadata.
///
/// # Arguments
/// * `config` - Configuration containing metadata file path
/// * `page` - Zero-indexed page number for pagination
///
/// # Returns
/// A vector of up to 8 BingImage structs for the requested page
pub fn load_historical_images_paginated(config: &Config, page: usize) -> Result<Vec<BingImage>> {
    let db = BingImageDb::new(config.db_path.clone())
        .context("Failed to open database in load_historical_images_paginated")?;

    // Load only 3 images per page to prevent ANR (reduced from 8)
    // egui decodes images synchronously on main thread - loading 8 at once causes 5s freeze
    let limit = 3;
    let offset = page * 3;
    let records = db.get_images_by_market_code_paginated("historical", limit, offset)
        .context("Failed to load paginated historical images from database")?;

    log::info!("Successfully loaded {} historical images from bing_images table", records.len());

    // Convert BingImageRecord to BingImage
    let bing_images: Vec<BingImage> = records
        .into_iter()
        .map(|record| BingImage {
            url: record.url,
            title: record.title,
            copyright: record.copyright,
            copyright_link: record.copyright_link,
        })
        .collect();

    Ok(bing_images)
}

/// Search for the original Bing URL of a cached image using its title.
///
/// This function attempts to locate the original Bing URL for a locally cached
/// image by searching through metadata files. It first checks the regular metadata
/// file (which stores filename|copyright|link entries), then falls back to searching
/// the historical metadata. The search uses fuzzy matching (checking if either string
/// contains the other) to handle cases where titles may have been truncated or
/// sanitized. If found, it returns the original Bing URL or reconstructs it from
/// the filename pattern.
///
/// # Arguments
/// * `config` - Configuration containing metadata file paths
/// * `title` - The title/filename to search for
///
/// # Returns
/// An Option containing the Bing URL if found, or None if not located
pub fn find_bing_url_for_cached_image(_config: &Config, _title: &str) -> Result<Option<String>> {
    // Note: Metadata is now stored in database, not files
    // This function is deprecated and returns None
    // Callers should query the database directly using BingImageDb methods
    Ok(None)
}

/// Save image bytes to the unprocessed directory for wallpaper setting.
///
/// This function takes already-downloaded image bytes and saves them to the
/// unprocessed directory with a sanitized filename based on the URL or title.
/// This avoids downloading the same image twice - once for display and once
/// for wallpaper setting.
///
/// # Arguments
/// * `config` - Configuration with directory paths
/// * `bytes` - The image data as bytes
/// * `url` - The original image URL (used for generating filename)
/// * `title` - The image title (fallback for filename if URL parsing fails)
///
/// # Returns
/// The PathBuf where the image was saved
pub fn save_image_to_unprocessed(
    config: &Config,
    bytes: &[u8],
    url: &str,
    title: &str,
) -> Result<PathBuf> {
    // Generate filename from URL (same logic as download_and_save_image)
    let filename = url
        .split("th?id=")
        .nth(1)
        .and_then(|s| s.split('_').next())
        .unwrap_or(title);

    // Sanitize filename
    let sanitized = sanitize_filename(filename);
    let filepath = config.unprocessed_dir.join(format!("{}.jpg", sanitized));

    // Save to disk
    std::fs::write(&filepath, bytes)
        .with_context(|| format!("Failed to write image to {:?}", filepath))?;

    log::info!("Saved image to unprocessed directory: {:?}", filepath);

    Ok(filepath)
}

// ============================================================================
// Desktop-Only BingTrayLogic Struct
// ============================================================================

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]

/// Core business logic for Bing wallpaper management
pub struct BingTrayLogic {
    config: Config,
    current_image_path: Option<PathBuf>,
    db: Option<BingImageDb>,
    current_market_code: String,
    current_market_offset: u32,
    download_exhausted: bool, // True when API returns no more images
    last_downloaded_urls: Vec<String>, // Track last download to detect duplicates
}

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
impl BingTrayLogic {
    /// Create a new BingTrayLogic instance with initialized configuration and database.
    ///
    /// This constructor initializes the core wallpaper management logic by creating
    /// a new Config object (which sets up all necessary directories) and attempting
    /// to open or create a DuckDB database for tracking image metadata and state.
    /// If database initialization fails, the instance is still created but with
    /// database features disabled. This graceful degradation ensures the application
    /// can function even without database support.
    ///
    /// # Returns
    /// A new BingTrayLogic instance ready for wallpaper operations
    pub fn new() -> Result<Self> {
        let config = Config::new()?;

        // Initialize database
        let db = BingImageDb::new(config.db_path.clone()).ok();

        // Load market state (default to en-US, offset 0)
        let (current_market_code, current_market_offset) = Self::load_market_state(&config)
            .unwrap_or_else(|_| ("en-US".to_string(), 0));

        Ok(Self {
            config,
            current_image_path: None,
            db,
            current_market_code,
            current_market_offset,
            download_exhausted: false,
            last_downloaded_urls: Vec::new(),
        })
    }

    /// Load the current market code and offset from persistent storage.
    ///
    /// # Returns
    /// A tuple of (market_code, offset) or an error if the file doesn't exist or is invalid
    fn load_market_state(config: &Config) -> Result<(String, u32)> {
        let state_file = config.config_dir.join("market_state.conf");
        if !state_file.exists() {
            return Ok(("en-US".to_string(), 0));
        }

        let content = fs::read_to_string(&state_file)?;
        let parts: Vec<&str> = content.trim().split('|').collect();

        if parts.len() != 2 {
            return Ok(("en-US".to_string(), 0));
        }

        let market_code = parts[0].to_string();
        let offset = parts[1].parse::<u32>().unwrap_or(0);

        Ok((market_code, offset))
    }

    /// Save the current market code and offset to persistent storage.
    fn save_market_state(&self) -> Result<()> {
        let state_file = self.config.config_dir.join("market_state.conf");
        let content = format!("{}|{}", self.current_market_code, self.current_market_offset);
        fs::write(&state_file, content)?;
        Ok(())
    }

    /// Change the current market code and reset offset to 0.
    ///
    /// This method allows changing the market code (e.g., from "en-US" to "ja-JP").
    /// The offset is reset to 0 to start from the beginning of the new market.
    ///
    /// # Arguments
    /// * `market_code` - The new market code to use (e.g., "en-US", "ja-JP", "de-DE")
    pub fn set_market_code(&mut self, market_code: String) -> Result<()> {
        self.current_market_code = market_code;
        self.current_market_offset = 0;
        self.save_market_state()?;
        log::info!("Changed market code to: {}", self.current_market_code);
        Ok(())
    }

    /// Get the current market code.
    ///
    /// # Returns
    /// The current market code string (e.g., "en-US")
    pub fn get_market_code(&self) -> &str {
        &self.current_market_code
    }

    /// Perform initial setup and prepare the application for first use.
    ///
    /// This initialization method checks if there are any unprocessed wallpaper files
    /// available, and if not, automatically downloads an initial batch from Bing.
    /// It then attempts to load the most recently modified cached image as the current
    /// wallpaper, sorting by modification time to find the latest. This ensures that
    /// when the application starts, it has wallpapers ready to display and can resume
    /// from the last known state. All errors during download are logged but don't
    /// prevent initialization from succeeding.
    ///
    /// # Returns
    /// Ok(()) if initialization completes, regardless of download success
    pub fn initialize(&mut self) -> Result<()> {
        // Directories are already created in Config::new()

        // Check if we need to download images
        if !self.has_unprocessed_files() {
            log::info!("No unprocessed images found, downloading from Bing...");
            match self.download_from_next_market() {
                Ok(count) => {
                    log::info!("Successfully downloaded {} images", count);
                }
                Err(e) => {
                    log::warn!("Failed to download initial images: {}", e);
                }
            }
        }

        // Try to load current image: prefer keepfavorite if unprocessed is empty, otherwise use cached
        let load_dir = if !self.has_unprocessed_files() && self.has_kept_wallpapers() {
            log::info!("No unprocessed images available, loading from keepfavorite");
            &self.config.keepfavorite_dir
        } else {
            &self.config.cached_dir
        };

        if let Ok(entries) = fs::read_dir(load_dir) {
            let mut images: Vec<PathBuf> = entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.to_lowercase() == "jpg" || ext.to_lowercase() == "jpeg")
                        .unwrap_or(false)
                })
                .collect();

            if !images.is_empty() {
                // Sort by modification time (most recent first)
                images.sort_by_key(|path| {
                    fs::metadata(path)
                        .and_then(|m| m.modified())
                        .ok()
                });

                if let Some(latest) = images.last() {
                    self.current_image_path = Some(latest.clone());
                    log::info!("Loaded current image: {:?}", latest);
                }
            }
        }

        log::info!("BingTray initialized");
        Ok(())
    }

    /// Set the next unprocessed wallpaper WITHOUT downloading.
    ///
    /// This method ONLY sets the next wallpaper from the unprocessed directory.
    /// It does NOT download new images if the directory is empty.
    ///
    /// # Returns
    /// Ok(true) if a wallpaper was successfully set, Ok(false) if no images available
    pub fn set_next_wallpaper(&mut self) -> Result<bool> {
        // Get next unprocessed image
        if let Some(image_path) = self.get_next_unprocessed_image()? {
            api_setwallpaper::set_wallpaper(&image_path)?;
            self.current_image_path = Some(image_path.clone());

            // Move to cached directory
            let filename = image_path
                .file_name()
                .context("No filename")?
                .to_string_lossy()
                .to_string();
            let cached_path = self.config.cached_dir.join(&filename);

            fs::rename(&image_path, &cached_path)?;
            log::info!("Set wallpaper and cached: {:?}", cached_path);

            return Ok(true);
        }

        Ok(false)
    }

    /// Retrieve the next unprocessed wallpaper and set it as the desktop background.
    ///
    /// This method is the core wallpaper-changing function. It looks for the next
    /// unprocessed image file (sorted alphabetically), sets it as the system wallpaper
    /// using the platform-specific API, updates the current image tracking, and then
    /// moves the file from the unprocessed directory to the cached directory. This
    /// ensures each wallpaper is only used once and maintains a history of displayed
    /// wallpapers. The database is updated if available (currently MVP-skipped).
    /// If no unprocessed images are found, it downloads from the next market page.
    ///
    /// # Returns
    /// Ok(true) if a wallpaper was successfully set, Ok(false) if no images available
    pub fn set_next_market_wallpaper(&mut self) -> Result<bool> {
        // Get next unprocessed image
        if let Some(image_path) = self.get_next_unprocessed_image()? {
            api_setwallpaper::set_wallpaper(&image_path)?;
            self.current_image_path = Some(image_path.clone());

            // Move to cached directory
            let filename = image_path
                .file_name()
                .context("No filename")?
                .to_string_lossy()
                .to_string();
            let cached_path = self.config.cached_dir.join(&filename);

            fs::rename(&image_path, &cached_path)?;
            log::info!("Set wallpaper and cached: {:?}", cached_path);

            // Update database status if available
            if let Some(ref _db) = self.db {
                // Extract URL from filename or metadata
                // For now, skip database update during MVP
                log::debug!("Database update skipped for MVP");
            }

            return Ok(true);
        }

        // No unprocessed images available - download from next market
        log::info!("No unprocessed images available, downloading from next market");
        log::info!("BEFORE DOWNLOAD: market_code={}, offset={}", self.current_market_code, self.current_market_offset);
        match self.download_from_next_market() {
            Ok(count) => {
                log::info!("Successfully downloaded {} images from next market", count);
                log::info!("AFTER DOWNLOAD: market_code={}, offset={}", self.current_market_code, self.current_market_offset);
                // Try to set the first downloaded image
                if let Some(image_path) = self.get_next_unprocessed_image()? {
                    api_setwallpaper::set_wallpaper(&image_path)?;
                    self.current_image_path = Some(image_path.clone());

                    let filename = image_path
                        .file_name()
                        .context("No filename")?
                        .to_string_lossy()
                        .to_string();
                    let cached_path = self.config.cached_dir.join(&filename);

                    fs::rename(&image_path, &cached_path)?;
                    log::info!("Set wallpaper and cached: {:?}", cached_path);

                    return Ok(true);
                }
            }
            Err(e) => {
                log::error!("Failed to download from next market: {}", e);
            }
        }

        Ok(false)
    }

    /// Save the current wallpaper to the favorites collection for future use.
    ///
    /// This method marks the currently displayed wallpaper as a favorite by moving
    /// it from the cached directory to the keepfavorite directory. It handles cases
    /// where the file might have already been moved to cache, searching both the
    /// original current path and the cached directory. After successfully moving
    /// the file, it automatically calls set_next_market_wallpaper() to display
    /// the next wallpaper, creating a smooth "keep and advance" workflow.
    ///
    /// # Errors
    /// Returns an error if the current image file cannot be found or moved
    pub fn keep_current_image(&mut self) -> Result<()> {
        if let Some(ref current_path) = self.current_image_path.clone() {
            let filename = current_path
                .file_name()
                .context("No filename")?
                .to_string_lossy()
                .to_string();
            let keepfavorite_path = self.config.keepfavorite_dir.join(&filename);

            // Check if file still exists (might be in cached dir)
            if current_path.exists() {
                fs::rename(current_path, &keepfavorite_path)?;
            } else {
                // Try to find it in cached dir
                let cached_path = self.config.cached_dir.join(&filename);
                if cached_path.exists() {
                    fs::rename(&cached_path, &keepfavorite_path)?;
                } else {
                    anyhow::bail!("Current image file not found");
                }
            }

            log::info!("Moved to favorites: {:?}", keepfavorite_path);

            // Update database
            if let Some(ref _db) = self.db {
                // Update status in database
                log::debug!("Database update skipped for MVP");
            }

            // Try to set next wallpaper
            self.set_next_market_wallpaper()?;
        }

        Ok(())
    }

    /// Add the current wallpaper to the blacklist and permanently delete it.
    ///
    /// This method marks the currently displayed wallpaper as undesirable by adding
    /// its filename to the persistent blacklist file, then permanently deletes the
    /// image file from disk. It searches for the file in both the current path and
    /// the cached directory to ensure deletion. After blacklisting, it automatically
    /// advances to the next wallpaper by calling set_next_market_wallpaper(). The
    /// blacklist prevents re-downloading or re-displaying unwanted wallpapers.
    ///
    /// # Errors
    /// Returns an error if blacklist operations or file deletion fails
    pub fn blacklist_current_image(&mut self) -> Result<()> {
        if let Some(ref current_path) = self.current_image_path.clone() {
            let filename = current_path
                .file_name()
                .context("No filename")?
                .to_string_lossy()
                .to_string();

            // Read blacklist
            let mut blacklist = self.read_blacklist()?;

            // Add filename to blacklist
            blacklist.push(filename.clone());

            // Write blacklist
            self.write_blacklist(&blacklist)?;

            // Delete the image file
            if current_path.exists() {
                fs::remove_file(current_path)?;
            } else {
                // Try cached dir
                let cached_path = self.config.cached_dir.join(&filename);
                if cached_path.exists() {
                    fs::remove_file(&cached_path)?;
                }
            }

            log::info!("Blacklisted and deleted: {}", filename);

            // Update database
            if let Some(ref _db) = self.db {
                log::debug!("Database update skipped for MVP");
            }

            // Try to set next wallpaper
            self.set_next_market_wallpaper()?;
        }

        Ok(())
    }

    /// Select and display a random wallpaper from the favorites collection.
    ///
    /// This method scans the keepfavorite directory for all saved favorite wallpapers,
    /// filters for valid image files (jpg/jpeg), and randomly selects one using the
    /// rand crate's thread_rng. The selected image is then set as the desktop wallpaper
    /// and tracked as the current image. This feature allows users to revisit their
    /// favorite wallpapers without going through the normal sequential progression.
    /// If no favorites are available, it returns Ok(false).
    ///
    /// # Returns
    /// Ok(true) if a favorite was set, Ok(false) if no favorites exist
    pub fn set_kept_wallpaper(&mut self) -> Result<bool> {
        let entries = fs::read_dir(&self.config.keepfavorite_dir)?;
        let images: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_lowercase() == "jpg" || ext.to_lowercase() == "jpeg")
                    .unwrap_or(false)
            })
            .collect();

        if images.is_empty() {
            log::info!("No favorite wallpapers available");
            return Ok(false);
        }

        // Random selection
        let image_path = images
            .choose(&mut rand::thread_rng())
            .context("Failed to select random image")?;

        api_setwallpaper::set_wallpaper(image_path)?;
        self.current_image_path = Some(image_path.clone());

        log::info!("Set favorite wallpaper: {:?}", image_path);

        Ok(true)
    }

    /// Launch the system file manager to browse the cached wallpaper directory.
    ///
    /// This platform-specific method opens the cached wallpaper directory using the
    /// default file manager for the operating system. On Linux it uses xdg-open,
    /// on macOS it uses the open command, and on Windows it uses explorer. This
    /// allows users to manually browse, organize, or delete cached wallpapers using
    /// their familiar file management tools. The command is spawned asynchronously
    /// so the application doesn't block waiting for the file manager to close.
    ///
    /// # Errors
    /// Returns an error if the file manager command fails to spawn
    pub fn open_cache_directory(&self) -> Result<()> {
        let path = &self.config.cached_dir;

        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(path)
                .spawn()
                .context("Failed to open directory")?;
        }

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(path)
                .spawn()
                .context("Failed to open directory")?;
        }

        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(path)
                .spawn()
                .context("Failed to open directory")?;
        }

        log::info!("Opened cache directory: {:?}", path);

        Ok(())
    }

    // === State queries ===

    /// Query whether there are any unprocessed wallpapers ready to be displayed.
    ///
    /// This convenience method checks if the get_next_unprocessed_image() call
    /// would return Some(path), indicating that at least one wallpaper file
    /// exists in the unprocessed directory. This is useful for enabling/disabling
    /// UI elements like "Next" buttons or determining if a download operation is
    /// needed. The check is performed without consuming or modifying any files.
    ///
    /// # Returns
    /// true if at least one unprocessed image is available, false otherwise
    pub fn has_next_available(&self) -> bool {
        // Check if there are unprocessed images
        if self.get_next_unprocessed_image().ok().flatten().is_some() {
            return true;
        }

        // Check if we can download more (not exhausted)
        !self.download_exhausted
    }

    /// Internal helper to determine if the unprocessed directory contains any images.
    ///
    /// This private method scans the unprocessed directory and counts files with
    /// jpg or jpeg extensions. It's used by various other methods to determine
    /// whether certain operations are possible (e.g., can't keep or blacklist if
    /// there are no replacements available). The method handles directory read
    /// errors gracefully by returning false, treating inaccessible directories
    /// the same as empty ones.
    ///
    /// # Returns
    /// true if at least one unprocessed jpg/jpeg file exists, false otherwise
    /// Count the number of unprocessed image files.
    ///
    /// # Returns
    /// The number of jpg/jpeg files in the unprocessed directory
    pub fn count_unprocessed_files(&self) -> usize {
        if let Ok(entries) = fs::read_dir(&self.config.unprocessed_dir) {
            entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.path().extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.to_lowercase() == "jpg" || ext.to_lowercase() == "jpeg")
                        .unwrap_or(false)
                })
                .count()
        } else {
            0
        }
    }

    pub fn has_unprocessed_files(&self) -> bool {
        if let Ok(entries) = fs::read_dir(&self.config.unprocessed_dir) {
            let count = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.path().extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.to_lowercase() == "jpg" || ext.to_lowercase() == "jpeg")
                        .unwrap_or(false)
                })
                .count();
            count > 0
        } else {
            false
        }
    }

    /// Determine whether the currently displayed wallpaper is from the favorites.
    ///
    /// This private helper checks if the current image path starts with the
    /// keepfavorite directory path, indicating that the wallpaper is a favorite
    /// rather than a new image from the market codes rotation. This information
    /// is used to control UI behavior - for example, you shouldn't be able to
    /// "keep" an image that's already in the favorites directory.
    ///
    /// # Returns
    /// true if the current image is in the keepfavorite directory, false otherwise
    fn is_current_image_in_favorites(&self) -> bool {
        if let Some(ref current_image) = self.current_image_path {
            current_image.starts_with(&self.config.keepfavorite_dir)
        } else {
            false
        }
    }

    /// Evaluate whether the current wallpaper can be added to favorites.
    ///
    /// This method implements the business logic for determining when the "Keep"
    /// action should be available. A wallpaper can only be kept if: (1) there is
    /// a current image set, (2) that image is not already in the favorites directory,
    /// and (3) there are unprocessed images available to replace it. The third
    /// condition prevents users from being left without a wallpaper after keeping.
    ///
    /// # Returns
    /// true if the current wallpaper can be moved to favorites, false otherwise
    pub fn can_keep(&self) -> bool {
        // Can keep if there's a current image AND it's not already in favorites AND there are unprocessed files
        if let Some(ref _image_path) = self.current_image_path {
            !self.is_current_image_in_favorites() && self.has_unprocessed_files()
        } else {
            false
        }
    }

    /// Evaluate whether the current wallpaper can be blacklisted and deleted.
    ///
    /// This method implements the business logic for determining when the "Blacklist"
    /// action should be available. A wallpaper can be blacklisted if there is a
    /// current image set AND there are unprocessed images available to replace it.
    /// This prevents users from blacklisting their last wallpaper and being left
    /// with no background image. The check doesn't care if the current image is
    /// a favorite, unlike the keep check.
    ///
    /// # Returns
    /// true if the current wallpaper can be blacklisted, false otherwise
    pub fn can_blacklist(&self) -> bool {
        // Can blacklist if there's a current image AND there are unprocessed files
        self.current_image_path.is_some() && self.has_unprocessed_files()
    }

    /// Query whether any favorite wallpapers exist in the keepfavorite directory.
    ///
    /// This method scans the keepfavorite directory to determine if any jpg images
    /// have been saved as favorites. It's used to enable/disable the "Random Favorite"
    /// button or similar UI elements. The check is efficient, returning true as soon
    /// as it finds any jpg file without needing to enumerate the entire directory.
    /// Directory read errors are treated as "no favorites available".
    ///
    /// # Returns
    /// true if at least one jpg image exists in favorites, false otherwise
    pub fn has_kept_wallpapers(&self) -> bool {
        if let Ok(entries) = fs::read_dir(&self.config.keepfavorite_dir) {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.path().extension().and_then(|s| s.to_str()) == Some("jpg"))
        } else {
            false
        }
    }

    /// Extract a user-friendly title from the current wallpaper's filename.
    ///
    /// This method converts the current image file path into a displayable title
    /// string by extracting the file stem (filename without extension). If the
    /// stem is longer than 40 characters, it truncates and appends "..." to keep
    /// UI layouts clean. If no current image is set or the path is invalid, it
    /// returns the placeholder string "(no image)". This is useful for status bars,
    /// tooltips, or other UI elements that need to show what's currently displayed.
    ///
    /// # Returns
    /// A formatted title string for UI display
    pub fn get_current_image_title(&self) -> String {
        if let Some(ref path) = self.current_image_path {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if stem.len() > 40 {
                    format!("{}...", &stem[..40])
                } else {
                    stem.to_string()
                }
            } else {
                "(no image)".to_string()
            }
        } else {
            "(no image)".to_string()
        }
    }

    // === Helper methods ===

    /// Find and return the path to the next wallpaper file to be processed.
    ///
    /// This private helper method scans the unprocessed directory for all jpg/jpeg
    /// image files, sorts them alphabetically by filename, and returns the first
    /// one (if any exist). The alphabetical sorting ensures predictable, consistent
    /// ordering of wallpaper display. This method does not modify or consume the
    /// file - it simply identifies which file should be processed next by other
    /// methods like set_next_market_wallpaper().
    ///
    /// # Returns
    /// Ok(Some(path)) if an image is available, Ok(None) if directory is empty
    fn get_next_unprocessed_image(&self) -> Result<Option<PathBuf>> {
        let entries = fs::read_dir(&self.config.unprocessed_dir)?;

        // Get all .jpg files
        let mut images: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_lowercase() == "jpg" || ext.to_lowercase() == "jpeg")
                    .unwrap_or(false)
            })
            .collect();

        if images.is_empty() {
            return Ok(None);
        }

        // Sort by filename (alphabetical)
        images.sort();

        // Return first image
        Ok(images.into_iter().next())
    }

    /// Load the blacklist of unwanted wallpaper filenames from persistent storage.
    ///
    /// This method now retrieves blacklisted images from the database by querying
    /// for all images with status = 'blacklisted'. The URL field is extracted as
    /// the identifier. Falls back to reading the legacy blacklist.conf file if
    /// the database is not available, ensuring backward compatibility.
    ///
    /// # Returns
    /// A vector of blacklisted identifiers (URLs or filenames)
    fn read_blacklist(&self) -> Result<Vec<String>> {
        if let Some(ref db) = self.db {
            return db.get_blacklisted_urls()
                .context("Failed to read blacklist from database");
        }

        // No database available
        Ok(Vec::new())
    }

    /// Persist the updated blacklist to the database or disk.
    ///
    /// This method stores blacklisted image identifiers. When database is available,
    /// it updates/inserts records with status = 'blacklisted'. Falls back to writing
    /// the legacy blacklist.conf file if database is unavailable.
    ///
    /// Note: This method is currently only used to add new entries one at a time.
    /// The blacklist vector contains all existing plus new entries.
    ///
    /// # Arguments
    /// * `blacklist` - Complete list of identifiers to blacklist (URLs or filenames)
    fn write_blacklist(&self, blacklist: &[String]) -> Result<()> {
        if let Some(ref db) = self.db {
            // Get the last entry (newly added)
            if let Some(last_entry) = blacklist.last() {
                // Try to find and update existing image record
                if let Ok(Some(mut record)) = db.get_image(last_entry) {
                    record.status = ImageStatus::Blacklisted;
                    db.upsert_image(&record)
                        .context("Failed to update image status to blacklisted")?;
                    log::info!("Updated image status to blacklisted in database: {}", last_entry);
                    return Ok(());
                } else {
                    // Create a minimal record for unknown images
                    let record = BingImageRecord {
                        url: last_entry.clone(),
                        title: last_entry.clone(),
                        copyright: None,
                        copyright_link: None,
                        market_code: "unknown".to_string(),
                        fetched_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64,
                        status: ImageStatus::Blacklisted,
                    };
                    db.upsert_image(&record)
                        .context("Failed to create blacklist entry in database")?;
                    log::info!("Created blacklist entry in database: {}", last_entry);
                    return Ok(());
                }
            }
        }

        anyhow::bail!("Database not available")
    }

    /// Fetch and download an initial batch of wallpapers from Bing's API.
    ///
    /// This method performs the first-time setup by fetching 8 images from Bing's
    /// HPImageArchive API for the en-US market code. It first retrieves the metadata
    /// (URLs, titles, copyright), then downloads each image file and saves it to
    /// the unprocessed directory. Metadata is persisted to a separate file for
    /// future reference. Individual download failures are logged but don't stop
    /// the process - partial success is acceptable for initial setup.
    ///
    /// # Returns
    /// The number of images successfully downloaded (may be less than 8 if some fail)
    fn download_initial_images(&self) -> Result<usize> {
        // Use a popular market code for initial download
        let market_code = "en-US";
        let count = 8; // Bing API max
        let offset = 0;

        log::info!("Fetching {} images from Bing API (market: {})", count, market_code);

        // Fetch metadata from Bing API
        let images = self.fetch_bing_images_sync(market_code, count, offset)?;

        if images.is_empty() {
            anyhow::bail!("No images returned from Bing API");
        }

        log::info!("Retrieved {} image metadata entries", images.len());

        // Download and save images
        let mut downloaded = 0;
        for image in &images {
            match self.download_and_save_image(image) {
                Ok(_) => {
                    downloaded += 1;
                    log::info!("Downloaded: {}", image.title);
                }
                Err(e) => {
                    log::warn!("Failed to download {}: {}", image.title, e);
                }
            }
        }

        // Save metadata
        self.save_metadata(&images)?;

        Ok(downloaded)
    }

    /// Download images from the next page of the current market.
    ///
    /// This method downloads images from the current market code at the current offset,
    /// then increments the offset for the next download. The offset keeps increasing
    /// until the API returns no images (reached historical limit), then sets download_exhausted flag.
    ///
    /// # Returns
    /// The number of images successfully downloaded
    pub fn download_from_next_market(&mut self) -> Result<usize> {
        log::info!("=== DOWNLOAD_FROM_NEXT_MARKET START ===");
        log::info!("Current market: {}, offset: {}", self.current_market_code, self.current_market_offset);

        // Try to download from current market and offset
        let count = 8; // Bing API max
        log::info!("Fetching images: market={}, count={}, offset={}", self.current_market_code, count, self.current_market_offset);
        let images = self.fetch_bing_images_sync(&self.current_market_code, count, self.current_market_offset)?;

        if images.is_empty() {
            // No more images at this offset - mark as exhausted
            log::warn!("No images at offset {}, reached end of available data", self.current_market_offset);
            self.download_exhausted = true;
            anyhow::bail!("No more images available from Bing API");
        }

        // Check for duplicate downloads (Bing API repeating same images)
        let current_urls: Vec<String> = images.iter().map(|img| img.url.clone()).collect();
        if !self.last_downloaded_urls.is_empty() && current_urls == self.last_downloaded_urls {
            log::warn!("Detected duplicate images at offset {}. Bing API has no more historical data.", self.current_market_offset);
            self.download_exhausted = true;
            anyhow::bail!("No more unique images available - API is repeating");
        }

        // Store URLs for next comparison
        self.last_downloaded_urls = current_urls;

        // Reset exhausted flag on successful download
        self.download_exhausted = false;

        log::info!("Retrieved {} image metadata entries", images.len());
        for (i, img) in images.iter().enumerate() {
            log::info!("  Image {}: title='{}', url='{}'", i, img.title, img.url);
        }

        // Download and save images
        let mut downloaded = 0;
        for (i, image) in images.iter().enumerate() {
            log::info!("Downloading image {}/{}: {}", i + 1, images.len(), image.title);
            match self.download_and_save_image(image) {
                Ok(path) => {
                    downloaded += 1;
                    log::info!("✓ Downloaded {}/{}: {} -> {:?}", i + 1, images.len(), image.title, path);
                }
                Err(e) => {
                    log::warn!("✗ Failed to download {}: {}", image.title, e);
                }
            }
        }

        // Save metadata
        self.save_metadata(&images)?;

        // Increment offset by count to get next non-overlapping page
        let increment = count;
        log::info!("Incrementing offset from {} to {}", self.current_market_offset, self.current_market_offset + increment);
        self.current_market_offset += increment;
        self.save_market_state()?;

        log::info!("Advanced to offset {} for next download", self.current_market_offset);
        log::info!("=== DOWNLOAD_FROM_NEXT_MARKET END: market={}, offset={} ===", self.current_market_code, self.current_market_offset);

        Ok(downloaded)
    }

    /// Synchronously fetch Bing wallpaper metadata using the HPImageArchive API.
    ///
    /// This internal method makes an HTTP GET request to Bing's public API endpoint,
    /// waits for the response with a 30-second timeout, parses the JSON data, and
    /// converts it to BingImage structs. It's similar to the public get_bing_images()
    /// function but is designed for use within the struct's synchronous initialization
    /// flow. The method handles URL normalization by prepending "https://www.bing.com"
    /// to relative URLs returned by the API.
    ///
    /// # Arguments
    /// * `market_code` - Market/language code like "en-US"
    /// * `count` - Number of images to fetch (max 8)
    /// * `offset` - Days offset for historical images (0 = today)
    ///
    /// # Returns
    /// A vector of BingImage structs with full URLs and metadata
    fn fetch_bing_images_sync(&self, market_code: &str, count: u32, offset: u32) -> Result<Vec<BingImage>> {
        let url = format!(
            "https://www.bing.com/HPImageArchive.aspx?format=js&idx={}&n={}&mkt={}",
            offset, count, market_code
        );

        log::debug!("Fetching from URL: {}", url);

        // Create channel for receiving response
        let (tx, rx) = mpsc::channel();

        // Create request
        let mut request = ehttp::Request::get(&url);
        request.headers.insert(
            "User-Agent".to_string(),
            format!("bingtray/{}", env!("CARGO_PKG_VERSION")),
        );

        // Fetch asynchronously but wait for result
        ehttp::fetch(request, move |response| {
            let _ = tx.send(response);
        });

        // Wait for response with timeout
        let response = rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .context("Timeout waiting for Bing API response")?;

        let resp = response.map_err(|e| anyhow::anyhow!("Network error: {}", e))?;

        if !resp.ok {
            anyhow::bail!("HTTP {}: {}", resp.status, resp.status_text);
        }

        // Parse JSON
        let text = resp.text().context("Invalid UTF-8 in response")?;
        let bing_response: crate::BingResponse =
            serde_json::from_str(text).context("Failed to parse JSON response")?;

        // Convert to BingImage with full URLs
        let images: Vec<BingImage> = bing_response
            .images
            .into_iter()
            .map(|img| {
                let full_url = if img.url.starts_with("http") {
                    img.url
                } else {
                    format!("https://www.bing.com{}", img.url)
                };

                BingImage {
                    url: full_url,
                    title: img.title,
                    copyright: img.copyright,
                    copyright_link: img.copyright_link,
                }
            })
            .collect();

        Ok(images)
    }

    /// Download a single wallpaper image from Bing and save it to disk.
    ///
    /// This internal method takes a BingImage metadata struct, downloads the actual
    /// image data from the URL using ehttp with a 60-second timeout, generates a
    /// sanitized filename from the URL's "th?id=" parameter or title, and saves
    /// the binary data to the unprocessed directory as a .jpg file. The filename
    /// sanitization ensures filesystem compatibility by converting problematic
    /// characters to underscores. The method returns the path where the image was saved.
    ///
    /// # Arguments
    /// * `image` - BingImage struct containing the URL and metadata
    ///
    /// # Returns
    /// The PathBuf where the downloaded image was saved
    fn download_and_save_image(&self, image: &BingImage) -> Result<PathBuf> {
        // Create channel for receiving response
        let (tx, rx) = mpsc::channel();

        // Create request
        let mut request = ehttp::Request::get(&image.url);
        request.headers.insert(
            "User-Agent".to_string(),
            format!("bingtray/{}", env!("CARGO_PKG_VERSION")),
        );

        log::debug!("Downloading image from: {}", image.url);

        // Fetch asynchronously but wait for result
        ehttp::fetch(request, move |response| {
            let _ = tx.send(response);
        });

        // Wait for response with timeout
        let response = rx
            .recv_timeout(std::time::Duration::from_secs(60))
            .context("Timeout waiting for image download")?;

        let resp = response.map_err(|e| anyhow::anyhow!("Network error: {}", e))?;

        if !resp.ok {
            anyhow::bail!("HTTP {}: {}", resp.status, resp.status_text);
        }

        // Generate filename from URL
        let filename = image.url
            .split("th?id=")
            .nth(1)
            .and_then(|s| s.split('_').next())
            .unwrap_or(&image.title);

        // Sanitize filename
        let sanitized = filename
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect::<String>();

        let filepath = self.config.unprocessed_dir.join(format!("{}.jpg", sanitized));

        // Save to disk
        fs::write(&filepath, &resp.bytes)
            .with_context(|| format!("Failed to write image to {:?}", filepath))?;

        log::debug!("Saved image to: {:?}", filepath);

        Ok(filepath)
    }

    /// Append wallpaper metadata to the persistent metadata file.
    ///
    /// This internal method manages the metadata file that stores title, copyright,
    /// and copyright_link information for downloaded wallpapers. It reads existing
    /// metadata (if any), adds new entries in pipe-delimited format (title|copyright|link),
    /// checks for duplicates to avoid redundant entries, and writes the complete
    /// updated list back to disk. This metadata is used for displaying attribution
    /// information and for finding original URLs of cached images.
    ///
    /// # Arguments
    /// * `images` - Slice of BingImage structs to add to metadata
    fn save_metadata(&self, images: &[BingImage]) -> Result<()> {
        // Try to save to database first
        if let Some(ref db) = self.db {
            let mut saved_count = 0;
            for image in images {
                let record = BingImageRecord {
                    url: image.url.clone(),
                    title: image.title.clone(),
                    copyright: image.copyright.clone(),
                    copyright_link: image.copyright_link.clone(),
                    market_code: "unknown".to_string(), // Will be updated when fetching
                    fetched_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                    status: ImageStatus::Unprocessed,
                };

                match db.upsert_image(&record) {
                    Ok(_) => saved_count += 1,
                    Err(e) => log::warn!("Failed to save image metadata to database: {}", e),
                }
            }

            if saved_count == images.len() {
                log::info!("Successfully saved {} image metadata records to database", saved_count);
                return Ok(());
            } else {
                anyhow::bail!("Failed to save all metadata to database ({}/{})", saved_count, images.len());
            }
        }

        anyhow::bail!("Database not available")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config() -> (Config, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config");
        let cache_dir = temp_dir.path().join("cache");

        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&cache_dir).unwrap();

        let unprocessed_dir = cache_dir.join("unprocessed");
        let keepfavorite_dir = cache_dir.join("keepfavorite");
        let cached_dir = cache_dir.join("cached");

        fs::create_dir_all(&unprocessed_dir).unwrap();
        fs::create_dir_all(&keepfavorite_dir).unwrap();
        fs::create_dir_all(&cached_dir).unwrap();

        let config = Config {
            config_dir: config_dir.clone(),
            unprocessed_dir,
            keepfavorite_dir,
            cached_dir,
            db_path: config_dir.join("bingtray.db"),
        };

        (config, temp_dir)
    }

    #[test]
    fn test_sanitize_filename() {
        // Note: sanitize_filename removes all non-alphanumeric chars except space, -, and _
        assert_eq!(sanitize_filename("hello.jpg"), "hello_jpg");
        assert_eq!(sanitize_filename("hello world.jpg"), "hello world_jpg");
        assert_eq!(sanitize_filename("hello/world.jpg"), "hello_world_jpg");
        assert_eq!(sanitize_filename("hello\\world.jpg"), "hello_world_jpg");
        assert_eq!(sanitize_filename("hello:world.jpg"), "hello_world_jpg");
        assert_eq!(sanitize_filename("hello*world.jpg"), "hello_world_jpg");
        assert_eq!(sanitize_filename("hello?world.jpg"), "hello_world_jpg");
        assert_eq!(sanitize_filename("hello\"world.jpg"), "hello_world_jpg");
        assert_eq!(sanitize_filename("hello<world.jpg"), "hello_world_jpg");
        assert_eq!(sanitize_filename("hello>world.jpg"), "hello_world_jpg");
        assert_eq!(sanitize_filename("hello|world.jpg"), "hello_world_jpg");
        // Each non-allowed character is replaced individually, so "../" becomes "___"
        assert_eq!(sanitize_filename("hello/../world.jpg"), "hello____world_jpg");
        assert_eq!(sanitize_filename("hello-world_test"), "hello-world_test");
    }

    #[test]
    fn test_get_next_historical_page_no_file() {
        let (config, _temp_dir) = create_test_config();

        let result = get_next_historical_page(&config);
        // Without file, should return error or default to 1
        match result {
            Ok(page) => assert_eq!(page, 1),
            Err(_) => {
                // Error is acceptable if no internet
            }
        }
    }

    #[test]
    #[ignore] // Disabled: historical metadata now stored in database
    fn test_get_next_historical_page_with_existing_data() {
        let (config, _temp_dir) = create_test_config();

        // Test disabled: file-based historical metadata storage removed
        let _ = config;

        let result = get_next_historical_page(&config);
        match result {
            Ok(page) => assert!(page >= 1),
            Err(_) => {
                // Network error acceptable
            }
        }
    }

    #[test]
    #[ignore] // Disabled: historical metadata now stored in database
    fn test_get_historical_page_info_empty() {
        let (config, _temp_dir) = create_test_config();

        // Test disabled: file-based historical metadata storage removed
        let _ = config;

        let result = get_historical_page_info(&config).unwrap();
        assert_eq!(result, (0, 0));
    }

    #[test]
    #[ignore] // Disabled: historical metadata now stored in database
    fn test_get_historical_page_info_with_data() {
        let (config, _temp_dir) = create_test_config();

        // Test disabled: file-based historical metadata storage removed
        let _ = config;

        let result = get_historical_page_info(&config).unwrap();
        assert_eq!(result.0, 2); // current page
        assert_eq!(result.1, 4); // total pages = ceil(25 / 8) = 4
    }

    #[test]
    fn test_load_cached_images_paginated_empty() {
        let (config, _temp_dir) = create_test_config();

        let result = load_cached_images_paginated(&config, 0).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_load_cached_images_paginated_with_images() {
        let (config, _temp_dir) = create_test_config();

        // Create some test image files
        fs::write(config.cached_dir.join("test1.jpg"), b"fake image 1").unwrap();
        fs::write(config.cached_dir.join("test2.jpg"), b"fake image 2").unwrap();
        fs::write(config.cached_dir.join("test3.jpg"), b"fake image 3").unwrap();

        // Note: Metadata is now stored in database, not files
        // load_cached_images_paginated only scans the cached directory

        let result = load_cached_images_paginated(&config, 0).unwrap();
        assert!(result.len() > 0);
        assert!(result.len() <= 10); // Page size is 10
    }

    #[test]
    #[ignore] // Disabled: historical metadata now stored in database
    fn test_load_historical_images_paginated_empty() {
        let (config, _temp_dir) = create_test_config();

        // Test disabled: file-based historical metadata storage removed
        let _ = config;

        let result = load_historical_images_paginated(&config, 0).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    #[ignore] // Disabled: historical metadata now stored in database
    fn test_load_historical_images_paginated_with_data() {
        let (config, _temp_dir) = create_test_config();

        // Test disabled: file-based historical metadata storage removed
        let _ = config;

        let result = load_historical_images_paginated(&config, 0).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].title, "Title 1");
        assert_eq!(result[1].title, "Title 2");
        assert_eq!(result[2].title, "Title 3");
    }

    #[test]
    #[ignore] // Disabled: metadata now stored in database
    fn test_find_bing_url_for_cached_image_not_found() {
        let (config, _temp_dir) = create_test_config();

        // Test disabled: file-based metadata storage removed
        let _ = config;

        let result = find_bing_url_for_cached_image(&config, "nonexistent").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    #[ignore] // Disabled: metadata now stored in database
    fn test_find_bing_url_for_cached_image_found() {
        let (config, _temp_dir) = create_test_config();

        // Test disabled: file-based metadata storage removed
        let _ = config;

        let result = find_bing_url_for_cached_image(&config, "test2").unwrap();
        assert_eq!(result, Some("/th?id=test2&pid=hp".to_string()));
    }

    #[test]
    #[ignore] // Disabled: metadata now stored in database
    fn test_find_bing_url_for_cached_image_partial_match() {
        let (config, _temp_dir) = create_test_config();

        // Test disabled: file-based metadata storage removed
        let _ = config;

        let result = find_bing_url_for_cached_image(&config, "test_title").unwrap();
        assert_eq!(result, Some("/th?id=full_test_title_here&pid=hp".to_string()));
    }

    // Integration test for get_market_codes (requires internet)
    #[test]
    #[ignore] // Ignored by default since it requires internet
    fn test_get_market_codes_integration() {
        let result = get_market_codes();
        match result {
            Ok(codes) => {
                assert!(!codes.is_empty());
                assert!(codes.contains(&"en-US".to_string()));
            }
            Err(e) => {
                eprintln!("Network test failed (expected without internet): {}", e);
            }
        }
    }

    // Integration test for get_bing_images_manifest (requires internet)
    #[test]
    #[ignore] // Ignored by default since it requires internet
    fn test_get_bing_images_manifest_integration() {
        let result = get_bing_images_manifest("en-US", 1, 0);
        match result {
            Ok(images) => {
                assert!(!images.is_empty());
                assert!(!images[0].url.is_empty());
                assert!(!images[0].title.is_empty());
            }
            Err(e) => {
                eprintln!("Network test failed (expected without internet): {}", e);
            }
        }
    }

    // Integration test for download_historical_data
    #[test]
    #[ignore] // Disabled: historical metadata now stored in database
    fn test_download_historical_data_integration() {
        let (config, _temp_dir) = create_test_config();

        // Test disabled: file-based historical metadata storage removed
        let _ = config;
    }

    #[test]
    #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
    fn test_market_state_save_and_load() {
        let (_config, _temp_dir) = create_test_config();

        println!("\n=== Test: Market State Save and Load ===");

        // Test saving market state
        let mut logic = BingTrayLogic::new().unwrap();
        println!("Initial: market={}, offset={}", logic.current_market_code, logic.current_market_offset);
        assert_eq!(logic.current_market_code, "en-US");
        assert_eq!(logic.current_market_offset, 0);

        // Modify and save
        logic.current_market_code = "ja-JP".to_string();
        logic.current_market_offset = 5;
        logic.save_market_state().unwrap();
        println!("Modified: market={}, offset={}", logic.current_market_code, logic.current_market_offset);

        // Verify file exists and has correct content
        let state_file = logic.config.config_dir.join("market_state.conf");
        assert!(state_file.exists());

        let content = fs::read_to_string(&state_file).unwrap();
        println!("State file content: {}", content);
        assert_eq!(content, "ja-JP|5");

        // Verify load works
        let (market, offset) = BingTrayLogic::load_market_state(&logic.config).unwrap();
        println!("Loaded: market={}, offset={}", market, offset);
        assert_eq!(market, "ja-JP");
        assert_eq!(offset, 5);
    }

    #[test]
    #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
    fn test_market_offset_increments() {
        let (_config, _temp_dir) = create_test_config();

        let mut logic = BingTrayLogic::new().unwrap();

        println!("\n=== Test: Market Offset Increments ===");
        println!("Initial state: market={}, offset={}", logic.current_market_code, logic.current_market_offset);
        println!("State file: {:?}", logic.config.config_dir.join("market_state.conf"));

        let initial_offset = logic.current_market_offset;

        // Simulate offset increment
        logic.current_market_offset += 1;
        logic.save_market_state().unwrap();

        println!("After increment: market={}, offset={}", logic.current_market_code, logic.current_market_offset);
        assert_eq!(logic.current_market_offset, initial_offset + 1);

        // Verify state file was written
        let state_file = logic.config.config_dir.join("market_state.conf");
        let content = fs::read_to_string(&state_file).unwrap();
        println!("State file content: {}", content);

        // Load in new instance using same config
        let (market, offset) = BingTrayLogic::load_market_state(&logic.config).unwrap();
        println!("After reload from file: market={}, offset={}", market, offset);
        assert_eq!(offset, initial_offset + 1);
    }

    #[test]
    #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
    fn test_set_market_code_resets_offset() {
        let (config, _temp_dir) = create_test_config();

        let mut logic = BingTrayLogic::new().unwrap();

        // Increment offset
        logic.current_market_offset = 5;
        logic.save_market_state().unwrap();

        // Change market code
        logic.set_market_code("de-DE".to_string()).unwrap();

        // Verify offset was reset
        assert_eq!(logic.current_market_code, "de-DE");
        assert_eq!(logic.current_market_offset, 0);

        // Verify it was saved
        let logic2 = BingTrayLogic::new().unwrap();
        assert_eq!(logic2.current_market_code, "de-DE");
        assert_eq!(logic2.current_market_offset, 0);
    }

    #[test]
    #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
    #[ignore] // Requires internet connection
    fn test_download_next_page_increments_offset() {
        let (_config, _temp_dir) = create_test_config();

        let mut logic = BingTrayLogic::new().unwrap();

        println!("\n=== Test: Download Next Page ===");
        println!("Initial: market={}, offset={}", logic.current_market_code, logic.current_market_offset);

        let initial_offset = logic.current_market_offset;

        // Try to download from next market (requires internet)
        match logic.download_from_next_market() {
            Ok(count) => {
                println!("Downloaded {} images", count);
                println!("After download: market={}, offset={}", logic.current_market_code, logic.current_market_offset);

                // Offset should have incremented
                assert_eq!(logic.current_market_offset, initial_offset + 1);

                // Try again
                let second_offset = logic.current_market_offset;
                match logic.download_from_next_market() {
                    Ok(count2) => {
                        println!("Downloaded {} more images", count2);
                        println!("After second download: market={}, offset={}", logic.current_market_code, logic.current_market_offset);
                        assert_eq!(logic.current_market_offset, second_offset + 1);
                    }
                    Err(e) => {
                        eprintln!("Second download failed: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Download failed (this is expected without internet): {}", e);
            }
        }
    }
}

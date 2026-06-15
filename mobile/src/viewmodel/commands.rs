use diesel::prelude::*;
use anyhow::{Result, Context};
use crate::db::ImageStatus;
use crate::db::models::NewBingImage;
use std::path::PathBuf;

/// Download images for a market code (stub for now)
pub fn download_images_sync(_conn: &mut SqliteConnection, _market_code: &str) -> Result<usize> {
    // TODO: Implement actual download logic using api_bingimage.rs
    // For now, return 0 to make compilation work
    log::info!("download_images_sync called (stub)");
    Ok(0)
}

/// Set wallpaper from URL (stub for now)
pub fn set_wallpaper_sync(_conn: &mut SqliteConnection, url: &str) -> Result<bool> {
    // TODO: Implement actual wallpaper setting using api_setwallpaper.rs
    log::info!("set_wallpaper_sync called for: {}", url);
    Ok(true)
}

/// Toggle favorite status for an image
pub fn toggle_favorite_sync(conn: &mut SqliteConnection, url: &str) -> Result<()> {
    use crate::db::operations;

    // Get current image
    let img = operations::get_image(conn, url)?;

    if let Some(image) = img {
        let current_status = crate::db::ImageStatus::from_str(&image.status)
            .unwrap_or(ImageStatus::Unprocessed);

        let new_status = match current_status {
            ImageStatus::KeepFavorite => ImageStatus::Unprocessed,
            _ => ImageStatus::KeepFavorite,
        };

        operations::update_image_status(conn, url, new_status)?;
    }

    Ok(())
}

/// Blacklist an image
pub fn blacklist_image_sync(conn: &mut SqliteConnection, url: &str) -> Result<()> {
    use crate::db::operations;
    operations::update_image_status(conn, url, ImageStatus::Blacklisted)?;
    Ok(())
}

// ============================================================================
// Image Cache Helpers
// ============================================================================

/// Get the cache directory path for storing downloaded images
fn get_cache_dir() -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine cache directory"))?
        .join("bingtray")
        .join("images");

    // Create cache directory if it doesn't exist
    std::fs::create_dir_all(&cache_dir)?;

    Ok(cache_dir)
}

/// Generate a cache filename from a URL
fn get_cache_filename(url: &str) -> String {
    // Extract filename from URL (e.g., OHR.SomeImage_EN-US123_UHD.jpg)
    // URLs look like: https://www.bing.com/th?id=OHR.Hnausapollur_EN-US2080493040_1920x1080.jpg&rf=...
    // We want to extract: OHR_Hnausapollur (the first two parts before market code)

    let filename = url
        .split("th?id=")
        .nth(1)
        .and_then(|s| {
            // Split on & to remove query params
            let clean = s.split('&').next().unwrap_or(s);
            // Split on _ to get parts: [OHR.Name, EN-US..., 1920x1080.jpg]
            let parts: Vec<&str> = clean.split('_').collect();
            if parts.len() >= 2 {
                // Take first part (OHR.Name) and replace . with _
                Some(parts[0].replace('.', "_"))
            } else {
                Some(parts.first().unwrap_or(&"unknown").to_string())
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    // Sanitize filename
    let sanitized = filename
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();

    format!("{}.jpg", sanitized)
}

/// Load image bytes from cache if available
/// Returns Some(bytes) if cached, None if not found
fn load_image_from_cache(url: &str) -> Result<Option<Vec<u8>>> {
    let cache_dir = get_cache_dir()?;
    let filename = get_cache_filename(url);
    let cache_path = cache_dir.join(&filename);

    if cache_path.exists() {
        log::info!("✓ Cache hit: loading from {:?}", cache_path);
        let bytes = std::fs::read(&cache_path)?;
        Ok(Some(bytes))
    } else {
        log::info!("⚠ Cache miss: will download from network");
        Ok(None)
    }
}

/// Save image bytes to cache
fn save_image_to_cache(url: &str, bytes: &[u8]) -> Result<()> {
    let cache_dir = get_cache_dir()?;
    let filename = get_cache_filename(url);
    let cache_path = cache_dir.join(&filename);

    std::fs::write(&cache_path, bytes)?;
    log::info!("💾 Saved to cache: {:?} ({} bytes)", cache_path, bytes.len());

    Ok(())
}

// ============================================================================
// Market State Helpers
// ============================================================================

/// Get market state from config (market_code, offset)
/// Returns default ("en-US", 0) if not found
pub fn get_market_state_sync(conn: &mut SqliteConnection) -> Result<(String, u32)> {
    use crate::db::operations;
    
    let market_code = operations::get_config(conn, "market_code")?
        .unwrap_or_else(|| "en-US".to_string());
    
    let offset = operations::get_config(conn, "offset")?
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    
    Ok((market_code, offset))
}

/// Save market state to config (market_code, offset)
pub fn save_market_state_sync(conn: &mut SqliteConnection, market_code: &str, offset: u32) -> Result<()> {
    use crate::db::operations;
    
    operations::set_config(conn, "market_code", market_code)?;
    operations::set_config(conn, "offset", &offset.to_string())?;
    
    Ok(())
}

/// Increment market offset by 8 and save to config
pub fn increment_market_offset_sync(conn: &mut SqliteConnection) -> Result<()> {
    let (market_code, offset) = get_market_state_sync(conn)?;
    let new_offset = offset + 8;
    save_market_state_sync(conn, &market_code, new_offset)?;
    Ok(())
}

// ============================================================================
// CLI-Specific: Desktop Wallpaper Matching
// ============================================================================

/// Get the URL of the current desktop wallpaper by matching it to the database
/// Returns None if no match found (wallpaper not from BingTray or database cleared)
pub fn get_current_desktop_wallpaper_url_sync(conn: &mut SqliteConnection) -> Result<Option<String>> {
    use crate::db::operations;
    use crate::schema::bing_images;

    // First, try to get the tracked wallpaper URL from config (fast path)
    if let Ok(Some(tracked_url)) = operations::get_config(conn, "current_wallpaper_url") {
        log::debug!("Found tracked wallpaper URL: {}", tracked_url);
        // Verify it exists in database
        let images: Vec<crate::db::BingImage> = bing_images::table
            .filter(bing_images::url.eq(&tracked_url))
            .load(conn)?;

        if !images.is_empty() {
            return Ok(Some(tracked_url));
        }
        log::warn!("Tracked wallpaper URL not found in database, falling back to file detection");
    }

    // Fallback: Try to detect from desktop environment with timeout
    // This can be slow on some systems, so we add a timeout
    log::debug!("Attempting desktop environment wallpaper detection with 2s timeout");

    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let result = crate::api_setwallpaper::get_wallpaper();
        let _ = tx.send(result);
    });

    // Wait for result with 2 second timeout
    match rx.recv_timeout(std::time::Duration::from_secs(2)) {
        Ok(Ok(wallpaper_path_str)) => {
            use std::path::Path;
            let wallpaper_path = Path::new(&wallpaper_path_str);

            // Extract filename stem (without extension)
            let filename = wallpaper_path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("Invalid wallpaper path"))?;

            // Extract core identifier (remove OHR_ prefix if present)
            let core_id = filename
                .strip_prefix("OHR_")
                .unwrap_or(filename);

            // Query database for URLs containing this identifier
            let pattern = format!("%{}%", core_id);
            let images: Vec<crate::db::BingImage> = bing_images::table
                .filter(bing_images::url.like(pattern))
                .order(bing_images::fetched_at.desc())
                .load(conn)?;

            // Return first match (most recent if multiple)
            Ok(images.first().map(|img| img.url.clone()))
        }
        Ok(Err(e)) => {
            log::warn!("Desktop environment detection failed: {}", e);
            Ok(None)
        }
        Err(_) => {
            log::warn!("Desktop environment detection timed out after 2s - skipping wallpaper tracking");
            Ok(None)
        }
    }
}

/// Mark current desktop wallpaper as favorite
/// Returns image title if successful, None if no match found
pub fn keep_current_wallpaper_sync(conn: &mut SqliteConnection) -> Result<Option<String>> {
    use crate::db::operations;

    // Get current wallpaper URL
    let url = match get_current_desktop_wallpaper_url_sync(conn)? {
        Some(u) => u,
        None => return Ok(None),
    };

    // Get image to retrieve title
    let image = operations::get_image(conn, &url)?
        .ok_or_else(|| anyhow::anyhow!("Image not found in database"))?;

    let title = image.title.clone();

    // Update status to keepfavorite
    operations::update_image_status(conn, &url, ImageStatus::KeepFavorite)?;

    // Auto-advance: set next wallpaper (with rotation and auto-download)
    log::info!("Auto-advancing to next wallpaper after keep");
    if let Err(e) = download_and_set_next_wallpaper_sync(conn) {
        log::warn!("Failed to auto-advance to next wallpaper: {}", e);
    }

    Ok(Some(title))
}

/// Mark current desktop wallpaper as blacklisted
/// Returns image title if successful, None if no match found
pub fn blacklist_current_wallpaper_sync(conn: &mut SqliteConnection) -> Result<Option<String>> {
    use crate::db::operations;

    // Get current wallpaper URL
    let url = match get_current_desktop_wallpaper_url_sync(conn)? {
        Some(u) => u,
        None => return Ok(None),
    };

    // Get image to retrieve title
    let image = operations::get_image(conn, &url)?
        .ok_or_else(|| anyhow::anyhow!("Image not found in database"))?;

    let title = image.title.clone();

    // Update status to blacklisted
    operations::update_image_status(conn, &url, ImageStatus::Blacklisted)?;

    // Auto-advance: set next wallpaper (with rotation and auto-download)
    log::info!("Auto-advancing to next wallpaper after blacklist");
    if let Err(e) = download_and_set_next_wallpaper_sync(conn) {
        log::warn!("Failed to auto-advance to next wallpaper: {}", e);
    }

    Ok(Some(title))
}

/// Set a random favorite as desktop wallpaper
/// Returns image title if successful, None if no favorites available
pub fn set_random_favorite_wallpaper_sync(conn: &mut SqliteConnection) -> Result<Option<String>> {
    use crate::db::operations;
    use rand::seq::SliceRandom;

    // Query all favorites
    let favorites = operations::get_images_by_status(conn, ImageStatus::KeepFavorite)?;

    if favorites.is_empty() {
        return Ok(None);
    }

    // Pick one randomly
    let mut rng = rand::thread_rng();
    let image = favorites.choose(&mut rng)
        .ok_or_else(|| anyhow::anyhow!("Failed to pick random favorite"))?;

    // Try to load from cache first
    let bytes = if let Some(cached_bytes) = load_image_from_cache(&image.url)? {
        log::info!("Using cached image for: {}", image.title);
        cached_bytes
    } else {
        log::info!("Cache miss, downloading image for: {}", image.title);
        // Download image bytes on-demand via ehttp (synchronous)
        let (tx, rx) = std::sync::mpsc::channel();
        ehttp::fetch(ehttp::Request::get(&image.url), move |response| {
            let _ = tx.send(response);
        });

        let response = rx.recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| anyhow::anyhow!("Image download timeout"))?;

        let downloaded_bytes = response
            .map_err(|e| anyhow::anyhow!("Image download failed: {}", e))?
            .bytes;

        // Save to cache for future use
        save_image_to_cache(&image.url, &downloaded_bytes)?;

        downloaded_bytes
    };

    // Set wallpaper
    crate::api_setwallpaper::set_wallpaper_from_bytes(&bytes)?;

    // Track current wallpaper URL in config for detection
    operations::set_config(conn, "current_wallpaper_url", &image.url)?;
    log::debug!("Tracked current wallpaper URL: {}", image.url);

    Ok(Some(image.title.clone()))
}

// ============================================================================
// CLI-Specific: Download and Set Next Wallpaper
// ============================================================================

/// Download next wallpaper if needed, then set it as desktop wallpaper
/// Returns WallpaperSetResult with title and URL
pub fn download_and_set_next_wallpaper_sync(conn: &mut SqliteConnection) -> Result<crate::viewmodel::WallpaperSetResult> {
    use crate::db::operations;
    use crate::schema::bing_images;

    // Step 1: Check count of unprocessed images
    let unprocessed_count = operations::count_by_status(conn, crate::db::ImageStatus::Unprocessed)?;
    log::info!("Unprocessed images count: {}", unprocessed_count);

    // Step 2: If count < 7, download new page
    if unprocessed_count < 7 {
        log::info!("Unprocessed count ({}) < 7, downloading new page", unprocessed_count);

        let (market_code_str, _offset) = get_market_state_sync(conn)?;

        // Get all existing URLs to avoid re-downloading
        let existing_urls: Vec<String> = {
            use crate::schema::bing_images::dsl::*;
            bing_images
                .select(url)
                .load(conn)?
        };
        log::info!("Found {} existing URLs in database", existing_urls.len());

        // Use ImageSource to fetch from both Bing API and GitHub archive
        // Pass existing URLs so it can skip them and return next batch
        let sources = crate::viewmodel::sources::ImageSource::new(None);
        let images = sources.fetch_images(20, &existing_urls)
            .context("Failed to fetch images from sources")?;

        if images.is_empty() {
            anyhow::bail!("No images returned from sources");
        }

        let fetched_count = images.len();
        log::info!("Fetched {} images from dual sources (Bing + GitHub)", fetched_count);

        // Filter out already-downloaded images
        let new_images: Vec<_> = images.into_iter()
            .filter(|img| !existing_urls.contains(&img.url))
            .collect();

        if !new_images.is_empty() {
            log::info!("Found {} new images (filtered {} duplicates)",
                new_images.len(),
                fetched_count - new_images.len()
            );

            // Insert new images into database with status='unprocessed'
            use std::time::{SystemTime, UNIX_EPOCH};
            let current_timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            for img in &new_images {
                let new_img = NewBingImage {
                    url: &img.url,
                    title: &img.title,
                    copyright: img.copyright.as_deref(),
                    copyright_link: img.copyright_link.as_deref(),
                    market_code: &market_code_str,
                    status: "unprocessed",
                    fetched_at: current_timestamp,
                    created_at: current_timestamp,
                    updated_at: current_timestamp,
                };

                operations::upsert_image(conn, &new_img)?;
            }

            log::info!("Inserted {} new images into database", new_images.len());
        } else {
            log::warn!("All fetched images already exist in database - no new images to add");
        }
    }

    // Step 3: Get next unprocessed image (with rotation if at end)
    let unprocessed_list = bing_images::table
        .filter(bing_images::status.eq("unprocessed"))
        .order(bing_images::fetched_at.desc())
        .load::<crate::db::BingImage>(conn)?;

    if unprocessed_list.is_empty() {
        anyhow::bail!("No unprocessed images available");
    }

    // Try to find next image after current wallpaper
    let current_url_opt = operations::get_config(conn, "current_wallpaper_url")?;

    let image = if let Some(current_url) = current_url_opt {
        // Find index of current image
        if let Some(current_idx) = unprocessed_list.iter().position(|img| img.url == current_url) {
            // Get next image (or rotate to first if at end)
            if current_idx + 1 < unprocessed_list.len() {
                log::info!("Using next unprocessed image in sequence");
                unprocessed_list[current_idx + 1].clone()
            } else {
                log::info!("At end of unprocessed list, rotating to first image");
                unprocessed_list[0].clone()
            }
        } else {
            // Current wallpaper not in unprocessed list, use first unprocessed
            log::info!("Current wallpaper not in unprocessed list, using first unprocessed");
            unprocessed_list[0].clone()
        }
    } else {
        // No current wallpaper tracked, use first unprocessed
        log::info!("No current wallpaper tracked, using first unprocessed image");
        unprocessed_list[0].clone()
    };
    
    // Step 3: Download image bytes on-demand (with caching)
    let bytes = if let Some(cached_bytes) = load_image_from_cache(&image.url)? {
        log::info!("Using cached image: {}", image.title);
        cached_bytes
    } else {
        log::info!("Downloading image bytes: {}", image.url);
        let (tx, rx) = std::sync::mpsc::channel();
        ehttp::fetch(ehttp::Request::get(&image.url), move |response| {
            let _ = tx.send(response);
        });

        let response = rx.recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|_| anyhow::anyhow!("Image download timeout"))?;

        let downloaded_bytes = response
            .map_err(|e| anyhow::anyhow!("Image download failed: {}", e))?
            .bytes;

        // Save to cache for future use
        save_image_to_cache(&image.url, &downloaded_bytes)?;

        downloaded_bytes
    };
    
    // Step 4: Set wallpaper
    log::info!("Setting wallpaper: {}", image.title);
    crate::api_setwallpaper::set_wallpaper_from_bytes(&bytes)?;
    
    // Step 5: Track current wallpaper URL in config for detection
    operations::set_config(conn, "current_wallpaper_url", &image.url)?;
    log::debug!("Tracked current wallpaper URL: {}", image.url);
    
    // Step 6: Return result
    Ok(crate::viewmodel::WallpaperSetResult {
        title: image.title.clone(),
        url: image.url.clone(),
    })
}

// ============================================================================
// Instant Keep/Blacklist Operations (using cache)
// ============================================================================

use super::cache_manager::CacheManager;
use std::sync::Arc;

/// Keep current wallpaper as favorite, set next wallpaper instantly
pub fn keep_current_wallpaper_instant_sync(
    conn: &mut SqliteConnection,
    cache_mgr: &Arc<CacheManager>,
) -> Result<String> {
    use crate::db::operations;

    // 1. Get current wallpaper URL
    let url = get_current_desktop_wallpaper_url_sync(conn)?
        .ok_or_else(|| anyhow::anyhow!("No current wallpaper"))?;

    log::info!("Keeping current wallpaper: {}", url);

    // 2. Mark as favorite (instant database update)
    operations::update_image_status(conn, &url, ImageStatus::KeepFavorite)?;

    // 3. Get next cached image (pre-downloaded)
    let next_image = cache_mgr.get_next_cached_image()?
        .ok_or_else(|| anyhow::anyhow!("No cached images available"))?;

    log::info!("Setting next wallpaper: {}", next_image.title);

    // 4. Load from local cache (instant, no network)
    let bytes = cache_mgr.load_cached_bytes(&next_image.url)?;

    // 5. Set wallpaper
    crate::api_setwallpaper::set_wallpaper_from_bytes(&bytes)?;

    // 6. Update current wallpaper tracking
    operations::set_config(conn, "current_wallpaper_url", &next_image.url)?;

    // 7. Trigger background cache refill if count < 3
    if cache_mgr.needs_refill()? {
        let cache_clone = cache_mgr.clone();
        std::thread::spawn(move || {
            if let Err(e) = cache_clone.refill_background() {
                log::error!("Background refill failed: {}", e);
            }
        });
    }

    Ok(next_image.title)
}

/// Blacklist current wallpaper, set next wallpaper instantly
pub fn blacklist_current_wallpaper_instant_sync(
    conn: &mut SqliteConnection,
    cache_mgr: &Arc<CacheManager>,
) -> Result<String> {
    use crate::db::operations;

    // 1. Get current wallpaper URL
    let url = get_current_desktop_wallpaper_url_sync(conn)?
        .ok_or_else(|| anyhow::anyhow!("No current wallpaper"))?;

    log::info!("Blacklisting current wallpaper: {}", url);

    // 2. Mark as blacklisted (instant database update)
    operations::update_image_status(conn, &url, ImageStatus::Blacklisted)?;

    // 3. Get next cached image (pre-downloaded)
    let next_image = cache_mgr.get_next_cached_image()?
        .ok_or_else(|| anyhow::anyhow!("No cached images available"))?;

    log::info!("Setting next wallpaper: {}", next_image.title);

    // 4. Load from local cache (instant, no network)
    let bytes = cache_mgr.load_cached_bytes(&next_image.url)?;

    // 5. Set wallpaper
    crate::api_setwallpaper::set_wallpaper_from_bytes(&bytes)?;

    // 6. Update current wallpaper tracking
    operations::set_config(conn, "current_wallpaper_url", &next_image.url)?;

    // 7. Trigger background cache refill if count < 3
    if cache_mgr.needs_refill()? {
        let cache_clone = cache_mgr.clone();
        std::thread::spawn(move || {
            if let Err(e) = cache_clone.refill_background() {
                log::error!("Background refill failed: {}", e);
            }
        });
    }

    Ok(next_image.title)
}

// ============================================================================
// Image Cache Helpers (for carousel/main panel)
// ============================================================================

/// Load image bytes from cache if available
pub fn load_cached_image(url: &str) -> Result<Option<Vec<u8>>> {
    let cache_dir = get_cache_dir()?;
    let filename = get_cache_filename(url);
    let cache_path = cache_dir.join(&filename);

    if cache_path.exists() {
        log::debug!("Cache hit: {:?}", cache_path);
        let bytes = std::fs::read(&cache_path)?;
        Ok(Some(bytes))
    } else {
        log::debug!("Cache miss: {}", url);
        Ok(None)
    }
}

/// Download image from network (blocking)
pub fn download_image(url: &str) -> Result<Vec<u8>> {
    log::info!("Downloading image: {}", url);

    let (tx, rx) = std::sync::mpsc::channel();
    ehttp::fetch(ehttp::Request::get(url), move |response| {
        let _ = tx.send(response);
    });

    let response = rx.recv_timeout(std::time::Duration::from_secs(30))
        .context("Image download timeout")?;

    let resp = response
        .map_err(|e| anyhow::anyhow!("Network error: {}", e))?;

    if !resp.ok {
        anyhow::bail!("HTTP {}: {}", resp.status, resp.status_text);
    }

    Ok(resp.bytes)
}

/// Save image bytes to cache
pub fn save_to_cache(url: &str, bytes: &[u8]) -> Result<()> {
    let cache_dir = get_cache_dir()?;
    let filename = get_cache_filename(url);
    let cache_path = cache_dir.join(&filename);

    std::fs::write(&cache_path, bytes)?;
    log::debug!("Saved to cache: {:?} ({} bytes)", cache_path, bytes.len());

    Ok(())
}

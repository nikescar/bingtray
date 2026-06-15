use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

/// Get cache filename from URL
pub fn get_cache_filename(url: &str) -> String {
    // Extract identifier from Bing URL
    let identifier = url
        .split("th?id=")
        .nth(1)
        .and_then(|s| s.split('_').next())
        .map(|s| s.replace('.', "_"))
        .unwrap_or_else(|| {
            // Fallback: use last part of path
            url.split('/')
                .last()
                .unwrap_or("unknown")
                .split('?')
                .next()
                .unwrap_or("unknown")
                .to_string()
        });

    format!("{}.jpg", identifier)
}

/// Cached image metadata
pub struct CachedImage {
    pub url: String,
    pub title: String,
    pub cached_path: PathBuf,
}

/// Smart pre-download cache manager
pub struct CacheManager {
    cache_dir: PathBuf,
    db_path: PathBuf,
    sources: Option<Arc<super::sources::ImageSource>>,
}

impl CacheManager {
    /// Create new cache manager and ensure cache directory exists
    pub fn new(
        cache_dir: PathBuf,
        db_path: PathBuf,
        sources: Option<Arc<super::sources::ImageSource>>,
    ) -> Self {
        // Create cache directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            log::warn!("Failed to create cache directory: {}", e);
        }

        Self {
            cache_dir,
            db_path,
            sources,
        }
    }

    /// Initialize cache on app startup (download 3 images synchronously)
    pub fn initialize(&self) -> Result<usize> {
        log::info!("Initializing cache (target: 3 images)");

        // Check current cached count
        let cached_count = self.get_cached_count()?;

        if cached_count >= 3 {
            log::info!("Cache already has {} images, skipping initial download", cached_count);
            return Ok(cached_count);
        }

        let needed = 3 - cached_count;
        log::info!("Need to download {} images", needed);

        // Download and cache images
        self.download_and_cache(needed)
    }

    /// Download and cache N images
    fn download_and_cache(&self, count: usize) -> Result<usize> {
        let sources = self.sources.as_ref()
            .context("No image sources available")?;

        let mut conn = crate::db::establish_connection(&self.db_path);

        // Get existing URLs to skip
        use diesel::prelude::*;
        use crate::schema::bing_images;
        let existing_urls: Vec<String> = bing_images::table
            .select(bing_images::url)
            .load(&mut conn)?;

        // Fetch images from sources (fetch extra in case some fail)
        let images = sources.fetch_images(count * 2, &existing_urls)?;

        let mut downloaded = 0;

        for image in images.iter().take(count) {
            // Check if already in database and cached
            if let Ok(Some(_existing)) = crate::db::operations::get_image(&mut conn, &image.url) {
                // Check if already cached
                use diesel::prelude::*;
                use crate::schema::bing_images;

                let cached_at: Option<i32> = bing_images::table
                    .filter(bing_images::url.eq(&image.url))
                    .select(bing_images::cached_at)
                    .first(&mut conn)
                    .optional()?
                    .flatten();

                if cached_at.is_some() {
                    log::debug!("Image already cached: {}", image.title);
                    downloaded += 1;
                    continue;
                }
            }

            // Download image bytes with retry
            match self.download_with_retry(&image.url, 3) {
                Ok(bytes) => {
                    // Save to cache directory
                    let filename = get_cache_filename(&image.url);
                    let cache_path = self.cache_dir.join(&filename);

                    std::fs::write(&cache_path, &bytes)
                        .with_context(|| format!("Failed to write {}", filename))?;

                    log::info!("Cached image: {} ({} bytes)", image.title, bytes.len());

                    // Update database
                    self.mark_as_cached(&mut conn, &image.url)?;

                    downloaded += 1;
                }
                Err(e) => {
                    log::warn!("Failed to download {}: {}", image.title, e);
                }
            }

            if downloaded >= count {
                break;
            }
        }

        Ok(downloaded)
    }

    /// Download image bytes with retry (exponential backoff)
    fn download_with_retry(&self, url: &str, max_retries: usize) -> Result<Vec<u8>> {
        let mut retry_delay = std::time::Duration::from_secs(1);

        for attempt in 0..max_retries {
            log::debug!("Downloading {} (attempt {}/{})", url, attempt + 1, max_retries);

            let (tx, rx) = std::sync::mpsc::channel();
            ehttp::fetch(ehttp::Request::get(url), move |response| {
                let _ = tx.send(response);
            });

            match rx.recv_timeout(std::time::Duration::from_secs(30)) {
                Ok(Ok(response)) => {
                    if response.ok {
                        return Ok(response.bytes);
                    } else {
                        log::warn!("HTTP {} for {}", response.status, url);
                    }
                }
                Ok(Err(e)) => {
                    log::warn!("Network error: {}", e);
                }
                Err(_) => {
                    log::warn!("Timeout downloading {}", url);
                }
            }

            if attempt + 1 < max_retries {
                std::thread::sleep(retry_delay);
                retry_delay *= 2; // Exponential backoff
            }
        }

        anyhow::bail!("Failed to download after {} retries", max_retries)
    }

    /// Mark image as cached in database
    fn mark_as_cached(&self, conn: &mut diesel::SqliteConnection, url: &str) -> Result<()> {
        use diesel::prelude::*;
        use crate::schema::bing_images;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        diesel::update(bing_images::table)
            .filter(bing_images::url.eq(url))
            .set(bing_images::cached_at.eq(now))
            .execute(conn)?;

        Ok(())
    }

    /// Get count of cached images in database
    pub fn get_cached_count(&self) -> Result<usize> {
        use diesel::prelude::*;
        use crate::schema::bing_images;

        let mut conn = crate::db::establish_connection(&self.db_path);

        let count: i64 = bing_images::table
            .filter(bing_images::status.eq("unprocessed"))
            .filter(bing_images::cached_at.is_not_null())
            .count()
            .get_result(&mut conn)?;

        Ok(count as usize)
    }

    /// Check if cache refill is needed (< 3 cached images)
    pub fn needs_refill(&self) -> Result<bool> {
        Ok(self.get_cached_count()? < 3)
    }

    /// Download 5 more images in background (non-blocking)
    pub fn refill_background(&self) -> Result<usize> {
        log::info!("Background refill: downloading 5 images");
        self.download_and_cache(5)
    }

    /// Get next cached image for instant wallpaper setting
    pub fn get_next_cached_image(&self) -> Result<Option<CachedImage>> {
        use diesel::prelude::*;
        use crate::schema::bing_images;

        let mut conn = crate::db::establish_connection(&self.db_path);

        // Query for next unprocessed image with cached_at set
        let result: Option<crate::db::BingImage> = bing_images::table
            .filter(bing_images::status.eq("unprocessed"))
            .filter(bing_images::cached_at.is_not_null())
            .order(bing_images::fetched_at.desc())
            .first(&mut conn)
            .optional()?;

        if let Some(img) = result {
            let filename = get_cache_filename(&img.url);
            let cached_path = self.cache_dir.join(&filename);

            // Verify file exists
            if !cached_path.exists() {
                log::warn!("Cache file missing: {:?}", cached_path);
                return Ok(None);
            }

            Ok(Some(CachedImage {
                url: img.url,
                title: img.title,
                cached_path,
            }))
        } else {
            Ok(None)
        }
    }

    /// Load image bytes from cache (instant, no network)
    pub fn load_cached_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let filename = get_cache_filename(url);
        let cache_path = self.cache_dir.join(&filename);

        std::fs::read(&cache_path)
            .with_context(|| format!("Failed to read cached file: {:?}", cache_path))
    }
}

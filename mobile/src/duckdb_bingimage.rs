//! DuckDB database for Bing image metadata
//!
//! This module provides database storage for Bing wallpaper images and market codes.
//! - Native: Uses duckdb crate with synchronous queries
//! - WASM: Uses @duckdb/duckdb-wasm via JavaScript FFI (to be implemented)

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[cfg(not(target_arch = "wasm32"))]
use duckdb::{params, Connection};

/// Image status in the database
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageStatus {
    Unprocessed,
    KeepFavorite,
    Blacklisted,
    Cached,
}

impl ImageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageStatus::Unprocessed => "unprocessed",
            ImageStatus::KeepFavorite => "keepfavorite",
            ImageStatus::Blacklisted => "blacklisted",
            ImageStatus::Cached => "cached",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "unprocessed" => Some(ImageStatus::Unprocessed),
            "keepfavorite" => Some(ImageStatus::KeepFavorite),
            "blacklisted" => Some(ImageStatus::Blacklisted),
            "cached" => Some(ImageStatus::Cached),
            _ => None,
        }
    }
}

/// Bing image record stored in database
#[derive(Debug, Clone)]
pub struct BingImageRecord {
    pub url: String,
    pub title: String,
    pub copyright: Option<String>,
    pub copyright_link: Option<String>,
    pub market_code: String,
    pub fetched_at: i64, // Unix timestamp
    pub status: ImageStatus,
}

/// Market code record
#[derive(Debug, Clone)]
pub struct MarketCodeRecord {
    pub code: String,
    pub last_used_at: i64, // Unix timestamp
}

// ============================================================================
// NATIVE IMPLEMENTATION (Desktop & Android)
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
pub struct BingImageDb {
    conn: Arc<Mutex<Connection>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl BingImageDb {
    /// Check if a database file is potentially corrupted
    /// Returns true if the file exists but has suspicious characteristics
    fn is_database_corrupted(db_path: &PathBuf) -> bool {
        use std::fs;

        if !db_path.exists() {
            return false; // New database, not corrupted
        }

        // Check if file size is suspiciously small (< 100 bytes is likely corrupted)
        if let Ok(metadata) = fs::metadata(db_path) {
            if metadata.len() < 100 {
                log::warn!("Database file is suspiciously small ({} bytes), may be corrupted", metadata.len());
                return true;
            }
        }

        // Try to read the first few bytes to check for valid DuckDB magic bytes
        if let Ok(mut file) = fs::File::open(db_path) {
            use std::io::Read;
            let mut magic = [0u8; 8];
            if file.read_exact(&mut magic).is_ok() {
                // DuckDB files typically start with specific magic bytes
                // We're being conservative here - if we can't verify, we'll try to open anyway
            }
        }

        false
    }

    /// Attempt to recover from a corrupted database by deleting and recreating it
    fn recover_corrupted_database(db_path: &PathBuf) -> Result<()> {
        log::warn!("Attempting to recover corrupted database at {:?}", db_path);

        // Create backup of corrupted file
        if db_path.exists() {
            let backup_path = db_path.with_extension("db.corrupted.bak");
            if let Err(e) = std::fs::rename(db_path, &backup_path) {
                log::error!("Failed to backup corrupted database: {}", e);
                // Try to delete if rename fails
                if let Err(e) = std::fs::remove_file(db_path) {
                    return Err(anyhow::anyhow!("Failed to remove corrupted database: {}", e));
                }
            } else {
                log::info!("Backed up corrupted database to {:?}", backup_path);
            }
        }

        // Also remove WAL (Write-Ahead Log) files which can cause corruption
        let wal_path = db_path.with_extension("db.wal");
        if wal_path.exists() {
            if let Err(e) = std::fs::remove_file(&wal_path) {
                log::warn!("Failed to remove WAL file {:?}: {}", wal_path, e);
            } else {
                log::info!("Removed WAL file {:?}", wal_path);
            }
        }

        // Remove temporary files
        let tmp_path = db_path.with_extension("db.tmp");
        if tmp_path.exists() {
            if let Err(e) = std::fs::remove_file(&tmp_path) {
                log::warn!("Failed to remove temp file {:?}: {}", tmp_path, e);
            } else {
                log::info!("Removed temp file {:?}", tmp_path);
            }
        }

        log::info!("Corrupted database removed, will create new database");
        Ok(())
    }

    /// Validate database by attempting a simple query
    fn validate_database_connection(conn: &Connection) -> Result<()> {
        // Try a simple query to verify the database is functional
        match conn.execute("SELECT 1", []) {
            Ok(_) => Ok(()),
            Err(e) => {
                log::error!("Database validation failed: {}", e);
                Err(anyhow::anyhow!("Database validation failed: {}", e))
            }
        }
    }

    /// Create a new database connection or open existing database
    /// Automatically handles corrupted database files by recreating them
    pub fn new(db_path: PathBuf) -> Result<Self> {
        // Check for obvious corruption before attempting to open
        if Self::is_database_corrupted(&db_path) {
            Self::recover_corrupted_database(&db_path)?;
        }

        // Attempt to open the database
        let conn = match Connection::open(&db_path) {
            Ok(conn) => {
                // Validate that the connection actually works
                match Self::validate_database_connection(&conn) {
                    Ok(_) => conn,
                    Err(validation_err) => {
                        log::error!("Database opened but validation failed: {}", validation_err);
                        drop(conn); // Close the connection

                        // Try recovery
                        Self::recover_corrupted_database(&db_path)?;

                        // Retry opening after recovery
                        Connection::open(&db_path)
                            .with_context(|| format!("Failed to open DuckDB after recovery at {:?}", db_path))?
                    }
                }
            }
            Err(open_err) => {
                log::error!("Failed to open database: {}", open_err);

                // If opening fails, try to recover
                Self::recover_corrupted_database(&db_path)?;

                // Retry opening after recovery
                Connection::open(&db_path)
                    .with_context(|| format!("Failed to open DuckDB after recovery at {:?}", db_path))?
            }
        };

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        // Initialize schema - this will also serve as a final validation
        match db.init_schema() {
            Ok(_) => Ok(db),
            Err(schema_err) => {
                log::error!("Failed to initialize schema: {}", schema_err);
                drop(db); // Close the connection

                // Final recovery attempt
                Self::recover_corrupted_database(&db_path)?;

                // Create a new connection and retry schema initialization
                let new_conn = Connection::open(&db_path)
                    .with_context(|| format!("Failed to open DuckDB after schema error at {:?}", db_path))?;

                let new_db = Self {
                    conn: Arc::new(Mutex::new(new_conn)),
                };

                new_db.init_schema()
                    .context("Failed to initialize schema after recovery")?;

                Ok(new_db)
            }
        }
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Create bing_images table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS bing_images (
                url TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                copyright TEXT,
                copyright_link TEXT,
                market_code TEXT NOT NULL,
                fetched_at BIGINT NOT NULL,
                status TEXT NOT NULL
            )",
            [],
        )
        .context("Failed to create bing_images table")?;

        // Create market_codes table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS market_codes (
                code TEXT PRIMARY KEY,
                last_used_at BIGINT NOT NULL DEFAULT 0
            )",
            [],
        )
        .context("Failed to create market_codes table")?;

        // Create config_kv table for storing configuration key-value pairs
        conn.execute(
            "CREATE TABLE IF NOT EXISTS config_kv (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )
        .context("Failed to create config_kv table")?;

        // Create indexes for common queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bing_images_status ON bing_images(status)",
            [],
        )
        .ok();

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bing_images_market_code ON bing_images(market_code)",
            [],
        )
        .ok();

        Ok(())
    }

    /// Insert or update a Bing image record
    pub fn upsert_image(&self, record: &BingImageRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO bing_images (url, title, copyright, copyright_link, market_code, fetched_at, status)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (url) DO UPDATE SET
                title = excluded.title,
                copyright = excluded.copyright,
                copyright_link = excluded.copyright_link,
                market_code = excluded.market_code,
                fetched_at = excluded.fetched_at,
                status = excluded.status",
            params![
                &record.url,
                &record.title,
                &record.copyright,
                &record.copyright_link,
                &record.market_code,
                record.fetched_at,
                record.status.as_str(),
            ],
        )
        .context("Failed to upsert image record")?;

        Ok(())
    }

    /// Batch insert or update multiple image records in a single transaction
    /// This is much faster than individual upserts for bulk operations
    pub fn batch_upsert_images(&self, records: &[BingImageRecord]) -> Result<usize> {
        let conn = self.conn.lock().unwrap();

        // Begin transaction
        conn.execute("BEGIN TRANSACTION", [])
            .context("Failed to begin transaction")?;

        let mut saved_count = 0;
        for record in records {
            match conn.execute(
                "INSERT INTO bing_images (url, title, copyright, copyright_link, market_code, fetched_at, status)
                 VALUES (?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT (url) DO UPDATE SET
                    title = excluded.title,
                    copyright = excluded.copyright,
                    copyright_link = excluded.copyright_link,
                    market_code = excluded.market_code,
                    fetched_at = excluded.fetched_at,
                    status = excluded.status",
                params![
                    &record.url,
                    &record.title,
                    &record.copyright,
                    &record.copyright_link,
                    &record.market_code,
                    &record.fetched_at,
                    &record.status.as_str(),
                ],
            ) {
                Ok(_) => saved_count += 1,
                Err(e) => log::warn!("Failed to upsert image {}: {}", record.url, e),
            }
        }

        // Commit transaction
        conn.execute("COMMIT", [])
            .context("Failed to commit transaction")?;

        Ok(saved_count)
    }

    /// Get an image by URL
    pub fn get_image(&self, url: &str) -> Result<Option<BingImageRecord>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT url, title, copyright, copyright_link, market_code, fetched_at, status FROM bing_images WHERE url = ?")
            .context("Failed to prepare get_image query")?;

        let mut rows = stmt
            .query(params![url])
            .context("Failed to execute get_image query")?;

        if let Some(row) = rows.next().context("Failed to fetch row")? {
            let status_str: String = row.get(6)?;
            let status = ImageStatus::from_str(&status_str)
                .unwrap_or(ImageStatus::Unprocessed);

            Ok(Some(BingImageRecord {
                url: row.get(0)?,
                title: row.get(1)?,
                copyright: row.get(2)?,
                copyright_link: row.get(3)?,
                market_code: row.get(4)?,
                fetched_at: row.get(5)?,
                status,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get all images with a specific status
    pub fn get_images_by_status(&self, status: ImageStatus) -> Result<Vec<BingImageRecord>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT url, title, copyright, copyright_link, market_code, fetched_at, status FROM bing_images WHERE status = ? ORDER BY fetched_at DESC")
            .context("Failed to prepare get_images_by_status query")?;

        let rows = stmt
            .query_map(params![status.as_str()], |row| {
                let status_str: String = row.get(6)?;
                let status = ImageStatus::from_str(&status_str)
                    .unwrap_or(ImageStatus::Unprocessed);

                Ok(BingImageRecord {
                    url: row.get(0)?,
                    title: row.get(1)?,
                    copyright: row.get(2)?,
                    copyright_link: row.get(3)?,
                    market_code: row.get(4)?,
                    fetched_at: row.get(5)?,
                    status,
                })
            })
            .context("Failed to execute get_images_by_status query")?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    /// Get all images with a specific market code
    pub fn get_images_by_market_code(&self, market_code: &str) -> Result<Vec<BingImageRecord>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT url, title, copyright, copyright_link, market_code, fetched_at, status FROM bing_images WHERE market_code = ? ORDER BY fetched_at DESC")
            .context("Failed to prepare get_images_by_market_code query")?;

        let rows = stmt
            .query_map(params![market_code], |row| {
                let status_str: String = row.get(6)?;
                let status = ImageStatus::from_str(&status_str)
                    .unwrap_or(ImageStatus::Unprocessed);

                Ok(BingImageRecord {
                    url: row.get(0)?,
                    title: row.get(1)?,
                    copyright: row.get(2)?,
                    copyright_link: row.get(3)?,
                    market_code: row.get(4)?,
                    fetched_at: row.get(5)?,
                    status,
                })
            })
            .context("Failed to execute get_images_by_market_code query")?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    /// Get images with a specific market code, with pagination support
    pub fn get_images_by_market_code_paginated(&self, market_code: &str, limit: usize, offset: usize) -> Result<Vec<BingImageRecord>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT url, title, copyright, copyright_link, market_code, fetched_at, status FROM bing_images WHERE market_code = ? ORDER BY fetched_at DESC LIMIT ? OFFSET ?")
            .context("Failed to prepare get_images_by_market_code_paginated query")?;

        let rows = stmt
            .query_map(params![market_code, limit, offset], |row| {
                let status_str: String = row.get(6)?;
                let status = ImageStatus::from_str(&status_str)
                    .unwrap_or(ImageStatus::Unprocessed);

                Ok(BingImageRecord {
                    url: row.get(0)?,
                    title: row.get(1)?,
                    copyright: row.get(2)?,
                    copyright_link: row.get(3)?,
                    market_code: row.get(4)?,
                    fetched_at: row.get(5)?,
                    status,
                })
            })
            .context("Failed to execute get_images_by_market_code_paginated query")?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    /// Update image status
    pub fn update_image_status(&self, url: &str, status: ImageStatus) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE bing_images SET status = ? WHERE url = ?",
            params![status.as_str(), url],
        )
        .context("Failed to update image status")?;

        Ok(())
    }

    /// Delete an image record
    pub fn delete_image(&self, url: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute("DELETE FROM bing_images WHERE url = ?", params![url])
            .context("Failed to delete image")?;

        Ok(())
    }

    /// Insert or update a market code
    pub fn upsert_market_code(&self, code: &str, last_used_at: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO market_codes (code, last_used_at)
             VALUES (?, ?)
             ON CONFLICT (code) DO UPDATE SET
                last_used_at = excluded.last_used_at",
            params![code, last_used_at],
        )
        .context("Failed to upsert market code")?;

        Ok(())
    }

    /// Get all market codes ordered by last used time
    pub fn get_market_codes(&self) -> Result<Vec<MarketCodeRecord>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT code, last_used_at FROM market_codes ORDER BY last_used_at DESC")
            .context("Failed to prepare get_market_codes query")?;

        let rows = stmt
            .query_map([], |row| {
                Ok(MarketCodeRecord {
                    code: row.get(0)?,
                    last_used_at: row.get(1)?,
                })
            })
            .context("Failed to execute get_market_codes query")?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }

        Ok(records)
    }

    /// Delete a market code
    pub fn delete_market_code(&self, code: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute("DELETE FROM market_codes WHERE code = ?", params![code])
            .context("Failed to delete market code")?;

        Ok(())
    }

    /// Count images by status
    pub fn count_by_status(&self, status: ImageStatus) -> Result<usize> {
        let conn = self.conn.lock().unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM bing_images WHERE status = ?",
                params![status.as_str()],
                |row| row.get(0),
            )
            .context("Failed to count images by status")?;

        Ok(count as usize)
    }

    /// Get a configuration value by key
    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT value FROM config_kv WHERE key = ?")
            .context("Failed to prepare get_config query")?;

        let mut rows = stmt
            .query(params![key])
            .context("Failed to execute get_config query")?;

        if let Some(row) = rows.next().context("Failed to fetch row")? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    /// Set a configuration value
    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO config_kv (key, value)
             VALUES (?, ?)
             ON CONFLICT (key) DO UPDATE SET
                value = excluded.value",
            params![key, value],
        )
        .context("Failed to set config value")?;

        Ok(())
    }

    /// Delete a configuration value
    pub fn delete_config(&self, key: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute("DELETE FROM config_kv WHERE key = ?", params![key])
            .context("Failed to delete config value")?;

        Ok(())
    }

    /// Get all blacklisted image URLs
    pub fn get_blacklisted_urls(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn
            .prepare("SELECT url FROM bing_images WHERE status = ? ORDER BY fetched_at DESC")
            .context("Failed to prepare get_blacklisted_urls query")?;

        let rows = stmt
            .query_map(params![ImageStatus::Blacklisted.as_str()], |row| {
                row.get(0)
            })
            .context("Failed to execute get_blacklisted_urls query")?;

        let mut urls = Vec::new();
        for row in rows {
            urls.push(row?);
        }

        Ok(urls)
    }

    /// Get historical page number (for pagination tracking)
    pub fn get_historical_page(&self) -> Result<usize> {
        Ok(self
            .get_config("historical_page")?
            .and_then(|v| v.parse().ok())
            .unwrap_or(0))
    }

    /// Set historical page number
    pub fn set_historical_page(&self, page: usize) -> Result<()> {
        self.set_config("historical_page", &page.to_string())
    }

    /// Flush/checkpoint the database to ensure all data is written to disk
    pub fn checkpoint(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("CHECKPOINT", [])
            .context("Failed to checkpoint database")?;
        Ok(())
    }

    /// Get the last download timestamp for a manifest type (unix timestamp)
    pub fn get_last_download_timestamp(&self, manifest_type: &str) -> Result<Option<i64>> {
        let key = format!("last_download_{}", manifest_type);
        Ok(self.get_config(&key)?.and_then(|v| v.parse().ok()))
    }

    /// Set the last download timestamp for a manifest type (unix timestamp)
    pub fn set_last_download_timestamp(&self, manifest_type: &str, timestamp: i64) -> Result<()> {
        let key = format!("last_download_{}", manifest_type);
        self.set_config(&key, &timestamp.to_string())
    }

    /// Check if a manifest needs to be downloaded (>7 days since last download)
    pub fn should_download_manifest(&self, manifest_type: &str) -> bool {
        match self.get_last_download_timestamp(manifest_type) {
            Ok(Some(last_download)) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                let days_elapsed = (now - last_download) / 86400; // seconds in a day
                days_elapsed >= 7
            }
            _ => true, // Download if no timestamp or error
        }
    }
}

// ============================================================================
// WASM IMPLEMENTATION (Browser)
// ============================================================================

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use js_sys;
#[cfg(target_arch = "wasm32")]
use std::sync::RwLock;

// JavaScript FFI bindings to @duckdb/duckdb-wasm
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(module = "@duckdb/duckdb-wasm")]
extern "C" {
    #[wasm_bindgen(js_name = "AsyncDuckDB")]
    pub type JsAsyncDuckDB;

    #[wasm_bindgen(catch, method, js_name = "connectInternal")]
    async fn connect(this: &JsAsyncDuckDB) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, method, js_name = "runQuery")]
    async fn run_query(
        this: &JsAsyncDuckDB,
        conn: u32,
        text: &str,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, method, js_name = "fetchQueryResults")]
    async fn fetch_query_results(this: &JsAsyncDuckDB, conn: u32) -> Result<JsValue, JsValue>;
}

#[cfg(target_arch = "wasm32")]
pub struct BingImageDb {
    db: Arc<RwLock<JsAsyncDuckDB>>,
    conn_id: u32,
}

#[cfg(target_arch = "wasm32")]
impl BingImageDb {
    /// Create a new DuckDB instance for WASM
    ///
    /// Note: This requires @duckdb/duckdb-wasm to be loaded in the browser
    /// and passed to Rust via JavaScript interop
    pub fn new(_db_path: PathBuf) -> Result<Self> {
        // In WASM, the database is created from JavaScript
        // This is a simplified version that logs a warning
        log::warn!("DuckDB WASM: Database should be initialized from JavaScript");
        log::info!("DuckDB WASM: Using in-memory storage (no persistence)");

        // For now, return an error indicating JS initialization is needed
        anyhow::bail!("DuckDB WASM requires JavaScript initialization. Use init_from_js()");
    }

    /// Initialize from JavaScript-created DuckDB instance
    ///
    /// This should be called from JavaScript after creating the DuckDB instance:
    /// ```js
    /// const db = await DuckDB.create();
    /// const connId = await db.connect();
    /// // Pass db and connId to Rust
    /// ```
    #[cfg(target_arch = "wasm32")]
    pub fn init_from_js(db: JsAsyncDuckDB, conn_id: u32) -> Result<Self> {
        log::info!("DuckDB WASM initialized with connection ID: {}", conn_id);

        Ok(Self {
            db: Arc::new(RwLock::new(db)),
            conn_id,
        })
    }

    /// Initialize schema (async in WASM)
    #[cfg(target_arch = "wasm32")]
    pub async fn init_schema_async(&self) -> Result<()> {
        let db = self.db.read().unwrap();

        // Create tables
        let create_images_table = "CREATE TABLE IF NOT EXISTS bing_images (
            url TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            copyright TEXT,
            copyright_link TEXT,
            market_code TEXT NOT NULL,
            fetched_at BIGINT NOT NULL,
            status TEXT NOT NULL
        )";

        let create_market_codes_table = "CREATE TABLE IF NOT EXISTS market_codes (
            code TEXT PRIMARY KEY,
            last_used_at BIGINT NOT NULL DEFAULT 0
        )";

        // Execute queries (simplified - in real implementation would use run_query + fetch_query_results)
        log::info!("DuckDB WASM: Schema initialization (tables created via JS)");

        Ok(())
    }

    // Simplified stubs for WASM (full implementation would use async JS FFI)
    pub fn upsert_image(&self, record: &BingImageRecord) -> Result<()> {
        log::debug!("DuckDB WASM: upsert_image (stub): {}", record.url);
        Ok(())
    }

    pub fn get_image(&self, url: &str) -> Result<Option<BingImageRecord>> {
        log::debug!("DuckDB WASM: get_image (stub): {}", url);
        Ok(None)
    }

    pub fn get_images_by_status(&self, status: ImageStatus) -> Result<Vec<BingImageRecord>> {
        log::debug!("DuckDB WASM: get_images_by_status (stub): {:?}", status);
        Ok(Vec::new())
    }

    pub fn get_images_by_market_code(&self, market_code: &str) -> Result<Vec<BingImageRecord>> {
        log::debug!("DuckDB WASM: get_images_by_market_code (stub): {}", market_code);
        Ok(Vec::new())
    }

    pub fn get_images_by_market_code_paginated(&self, market_code: &str, limit: usize, offset: usize) -> Result<Vec<BingImageRecord>> {
        log::debug!("DuckDB WASM: get_images_by_market_code_paginated (stub): {} limit={} offset={}", market_code, limit, offset);
        Ok(Vec::new())
    }

    pub fn update_image_status(&self, url: &str, status: ImageStatus) -> Result<()> {
        log::debug!(
            "DuckDB WASM: update_image_status (stub): {} -> {:?}",
            url,
            status
        );
        Ok(())
    }

    pub fn delete_image(&self, url: &str) -> Result<()> {
        log::debug!("DuckDB WASM: delete_image (stub): {}", url);
        Ok(())
    }

    pub fn upsert_market_code(&self, code: &str, last_used_at: i64) -> Result<()> {
        log::debug!(
            "DuckDB WASM: upsert_market_code (stub): {} at {}",
            code,
            last_used_at
        );
        Ok(())
    }

    pub fn get_market_codes(&self) -> Result<Vec<MarketCodeRecord>> {
        log::debug!("DuckDB WASM: get_market_codes (stub)");
        Ok(Vec::new())
    }

    pub fn delete_market_code(&self, code: &str) -> Result<()> {
        log::debug!("DuckDB WASM: delete_market_code (stub): {}", code);
        Ok(())
    }

    pub fn count_by_status(&self, status: ImageStatus) -> Result<usize> {
        log::debug!("DuckDB WASM: count_by_status (stub): {:?}", status);
        Ok(0)
    }

    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        log::debug!("DuckDB WASM: get_config (stub): {}", key);
        Ok(None)
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        log::debug!("DuckDB WASM: set_config (stub): {} = {}", key, value);
        Ok(())
    }

    pub fn delete_config(&self, key: &str) -> Result<()> {
        log::debug!("DuckDB WASM: delete_config (stub): {}", key);
        Ok(())
    }

    pub fn get_blacklisted_urls(&self) -> Result<Vec<String>> {
        log::debug!("DuckDB WASM: get_blacklisted_urls (stub)");
        Ok(Vec::new())
    }

    pub fn get_historical_page(&self) -> Result<usize> {
        log::debug!("DuckDB WASM: get_historical_page (stub)");
        Ok(0)
    }

    pub fn set_historical_page(&self, page: usize) -> Result<()> {
        log::debug!("DuckDB WASM: set_historical_page (stub): {}", page);
        Ok(())
    }

    pub fn checkpoint(&self) -> Result<()> {
        log::debug!("DuckDB WASM: checkpoint (stub)");
        Ok(())
    }

    pub fn get_last_download_timestamp(&self, manifest_type: &str) -> Result<Option<i64>> {
        log::debug!("DuckDB WASM: get_last_download_timestamp (stub): {}", manifest_type);
        Ok(None)
    }

    pub fn set_last_download_timestamp(&self, manifest_type: &str, timestamp: i64) -> Result<()> {
        log::debug!("DuckDB WASM: set_last_download_timestamp (stub): {} = {}", manifest_type, timestamp);
        Ok(())
    }

    pub fn should_download_manifest(&self, manifest_type: &str) -> bool {
        log::debug!("DuckDB WASM: should_download_manifest (stub): {}", manifest_type);
        true
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for BingImageDb {
    fn drop(&mut self) {
        log::info!("DuckDB WASM: Dropping database connection");
    }
}

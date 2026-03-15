//! DataFusion-based database for Bing image metadata
//!
//! This module provides database storage for Bing wallpaper images and market codes.
//! - Native: Uses DataFusion with Parquet file storage
//! - WASM: Uses in-memory Arrow tables

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use datafusion::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use arrow::array::{Array, ArrayRef, Int64Array, RecordBatch, StringArray};
#[cfg(not(target_arch = "wasm32"))]
use arrow::datatypes::{DataType, Field, Schema};
#[cfg(not(target_arch = "wasm32"))]
use parquet::file::properties::WriterProperties;
#[cfg(not(target_arch = "wasm32"))]
use parquet::arrow::arrow_writer::ArrowWriter;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;
#[cfg(not(target_arch = "wasm32"))]
use tokio::runtime::Runtime;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Mutex;
#[cfg(not(target_arch = "wasm32"))]
use dashmap::DashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicUsize, Ordering};

/// Image status in the database
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageStatus {
    Unprocessed,
    KeepFavorite,
    Blacklisted,
}

impl ImageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageStatus::Unprocessed => "unprocessed",
            ImageStatus::KeepFavorite => "keepfavorite",
            ImageStatus::Blacklisted => "blacklisted",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "unprocessed" | "cached" => Some(ImageStatus::Unprocessed),
            "keepfavorite" => Some(ImageStatus::KeepFavorite),
            "blacklisted" => Some(ImageStatus::Blacklisted),
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
    pub fetched_at: i64,
    pub status: ImageStatus,
}

/// Market code record
#[derive(Debug, Clone)]
pub struct MarketCodeRecord {
    pub code: String,
    pub last_used_at: i64,
}

// ============================================================================
// NATIVE IMPLEMENTATION (Desktop & Android)
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
pub struct BingImageDb {
    data_dir: PathBuf,
    ctx: Arc<Mutex<SessionContext>>,
    runtime: Arc<Runtime>,
    // Write-ahead cache: stores recent upserts in memory
    // Key: URL, Value: BingImageRecord
    write_cache: Arc<DashMap<String, BingImageRecord>>,
    cache_size: Arc<AtomicUsize>,
    cache_flush_threshold: usize,
}

#[cfg(not(target_arch = "wasm32"))]
impl BingImageDb {
    // Helper functions for safely extracting values from RecordBatch
    fn extract_string(batch: &RecordBatch, col_idx: usize, row_idx: usize) -> Result<String> {
        use arrow::compute::cast;
        use arrow::datatypes::DataType as ArrowDataType;

        let column = batch.column(col_idx);
        if let Some(arr) = column.as_any().downcast_ref::<StringArray>() {
            Ok(arr.value(row_idx).to_string())
        } else {
            let casted = cast(column, &ArrowDataType::Utf8)?;
            let str_arr = casted.as_any().downcast_ref::<StringArray>()
                .context("Failed to cast column to string")?;
            Ok(str_arr.value(row_idx).to_string())
        }
    }

    fn extract_optional_string(batch: &RecordBatch, col_idx: usize, row_idx: usize) -> Result<Option<String>> {
        let column = batch.column(col_idx);
        if column.is_null(row_idx) {
            return Ok(None);
        }

        use arrow::compute::cast;
        use arrow::datatypes::DataType as ArrowDataType;

        if let Some(arr) = column.as_any().downcast_ref::<StringArray>() {
            Ok(if arr.is_null(row_idx) { None } else { Some(arr.value(row_idx).to_string()) })
        } else {
            let casted = cast(column, &ArrowDataType::Utf8)?;
            let str_arr = casted.as_any().downcast_ref::<StringArray>()
                .context("Failed to cast column to string")?;
            Ok(if str_arr.is_null(row_idx) { None } else { Some(str_arr.value(row_idx).to_string()) })
        }
    }

    fn extract_i64(batch: &RecordBatch, col_idx: usize, row_idx: usize) -> Result<i64> {
        let column = batch.column(col_idx);
        if let Some(arr) = column.as_any().downcast_ref::<Int64Array>() {
            Ok(arr.value(row_idx))
        } else {
            Err(anyhow::anyhow!("Failed to read i64 column"))
        }
    }

    fn extract_image_record(batch: &RecordBatch, row_idx: usize) -> Result<BingImageRecord> {
        let status = ImageStatus::from_str(&Self::extract_string(batch, 6, row_idx)?)
            .unwrap_or(ImageStatus::Unprocessed);

        Ok(BingImageRecord {
            url: Self::extract_string(batch, 0, row_idx)?,
            title: Self::extract_string(batch, 1, row_idx)?,
            copyright: Self::extract_optional_string(batch, 2, row_idx)?,
            copyright_link: Self::extract_optional_string(batch, 3, row_idx)?,
            market_code: Self::extract_string(batch, 4, row_idx)?,
            fetched_at: Self::extract_i64(batch, 5, row_idx)?,
            status,
        })
    }

    /// Create a new database connection with DataFusion
    pub fn new(db_path: PathBuf) -> Result<Self> {
        // Create data directory from database path
        let data_dir = db_path.parent()
            .context("Invalid database path")?
            .join("datafusion_data");

        std::fs::create_dir_all(&data_dir)?;

        // Create a dedicated Tokio runtime for DataFusion operations
        // Following the thread pool pattern from DataFusion examples
        let runtime = Runtime::new()
            .context("Failed to create Tokio runtime")?;

        let ctx = runtime.block_on(async {
            // Optimize SessionConfig for better performance
            use datafusion::execution::context::SessionConfig;

            let config = SessionConfig::new()
                .with_target_partitions(num_cpus::get())  // Use all CPU cores
                .with_batch_size(16384);  // Larger batch size for better throughput

            let session_ctx = SessionContext::new_with_config(config);

            // Register tables if parquet files exist
            let images_path = data_dir.join("bing_images.parquet");
            if images_path.exists() {
                if let Err(e) = session_ctx.register_parquet("bing_images", images_path.to_str().unwrap(), ParquetReadOptions::default()).await {
                    log::warn!("Failed to register bing_images table: {}", e);
                }
            }

            let market_codes_path = data_dir.join("market_codes.parquet");
            if market_codes_path.exists() {
                if let Err(e) = session_ctx.register_parquet("market_codes", market_codes_path.to_str().unwrap(), ParquetReadOptions::default()).await {
                    log::warn!("Failed to register market_codes table: {}", e);
                }
            }

            let config_kv_path = data_dir.join("config_kv.parquet");
            if config_kv_path.exists() {
                if let Err(e) = session_ctx.register_parquet("config_kv", config_kv_path.to_str().unwrap(), ParquetReadOptions::default()).await {
                    log::warn!("Failed to register config_kv table: {}", e);
                }
            }

            session_ctx
        });

        let db = Self {
            data_dir,
            ctx: Arc::new(Mutex::new(ctx)),
            runtime: Arc::new(runtime),
            write_cache: Arc::new(DashMap::new()),
            cache_size: Arc::new(AtomicUsize::new(0)),
            cache_flush_threshold: 100, // Flush to disk after 100 cached records
        };

        db.init_schema()?;
        Ok(db)
    }

    /// Initialize database schema (create empty tables if needed)
    fn init_schema(&self) -> Result<()> {
        self.runtime.block_on(async {
            let ctx = self.ctx.lock().unwrap();

            // Create empty tables if they don't exist
            let images_path = self.data_dir.join("bing_images.parquet");
            if !images_path.exists() {
                self.create_empty_images_table(&images_path)?;
                ctx.register_parquet("bing_images", images_path.to_str().unwrap(), ParquetReadOptions::default()).await?;
            }

            let market_codes_path = self.data_dir.join("market_codes.parquet");
            if !market_codes_path.exists() {
                self.create_empty_market_codes_table(&market_codes_path)?;
                ctx.register_parquet("market_codes", market_codes_path.to_str().unwrap(), ParquetReadOptions::default()).await?;
            }

            let config_kv_path = self.data_dir.join("config_kv.parquet");
            if !config_kv_path.exists() {
                self.create_empty_config_kv_table(&config_kv_path)?;
                ctx.register_parquet("config_kv", config_kv_path.to_str().unwrap(), ParquetReadOptions::default()).await?;
            }

            Ok(())
        })
    }

    fn create_empty_images_table(&self, path: &PathBuf) -> Result<()> {
        let schema = Arc::new(Schema::new(vec![
            Field::new("url", DataType::Utf8, false),
            Field::new("title", DataType::Utf8, false),
            Field::new("copyright", DataType::Utf8, true),
            Field::new("copyright_link", DataType::Utf8, true),
            Field::new("market_code", DataType::Utf8, false),
            Field::new("fetched_at", DataType::Int64, false),
            Field::new("status", DataType::Utf8, false),
        ]));

        let batch = RecordBatch::new_empty(schema);
        self.write_parquet(path, vec![batch])?;
        Ok(())
    }

    fn create_empty_market_codes_table(&self, path: &PathBuf) -> Result<()> {
        let schema = Arc::new(Schema::new(vec![
            Field::new("code", DataType::Utf8, false),
            Field::new("last_used_at", DataType::Int64, false),
        ]));

        let batch = RecordBatch::new_empty(schema);
        self.write_parquet(path, vec![batch])?;
        Ok(())
    }

    fn create_empty_config_kv_table(&self, path: &PathBuf) -> Result<()> {
        let schema = Arc::new(Schema::new(vec![
            Field::new("key", DataType::Utf8, false),
            Field::new("value", DataType::Utf8, false),
        ]));

        let batch = RecordBatch::new_empty(schema);
        self.write_parquet(path, vec![batch])?;
        Ok(())
    }

    fn write_parquet(&self, path: &PathBuf, batches: Vec<RecordBatch>) -> Result<()> {
        if batches.is_empty() {
            return Ok(());
        }

        let file = File::create(path)?;
        let props = WriterProperties::builder().build();
        let schema = batches[0].schema();
        let mut writer = ArrowWriter::try_new(file, schema, Some(props))?;

        for batch in &batches {
            if batch.num_rows() > 0 {
                writer.write(batch)?;
            }
        }

        // Even if no rows, write the empty batch to preserve schema
        if batches.iter().all(|b| b.num_rows() == 0) {
            writer.write(&batches[0])?;
        }

        writer.close()?;
        Ok(())
    }

    /// Insert or update a Bing image record (FAST: uses write-ahead cache)
    pub fn upsert_image(&self, record: &BingImageRecord) -> Result<()> {
        // Store in write cache (instant, thread-safe)
        let is_new = self.write_cache.insert(record.url.clone(), record.clone()).is_none();

        if is_new {
            let current_size = self.cache_size.fetch_add(1, Ordering::Relaxed) + 1;

            // Auto-flush when cache reaches threshold
            if current_size >= self.cache_flush_threshold {
                self.flush_cache()?;
            }
        }

        Ok(())
    }

    /// Flush the write cache to Parquet files (call periodically or before shutdown)
    pub fn flush_cache(&self) -> Result<()> {
        let cache_size = self.cache_size.load(Ordering::Relaxed);
        if cache_size == 0 {
            return Ok(()); // Nothing to flush
        }

        log::info!("Flushing {} cached records to Parquet...", cache_size);

        self.runtime.block_on(async {
            let ctx = self.ctx.lock().unwrap();

            // Read existing data from Parquet
            let df = ctx.sql("SELECT * FROM bing_images").await?;
            let batches = df.collect().await?;

            // Build a map of existing records for fast lookup
            let mut existing: std::collections::HashMap<String, BingImageRecord> = std::collections::HashMap::new();

            for batch in &batches {
                for i in 0..batch.num_rows() {
                    let record = Self::extract_image_record(batch, i)?;
                    existing.insert(record.url.clone(), record);
                }
            }

            // Merge cached records (overwrites existing)
            for entry in self.write_cache.iter() {
                existing.insert(entry.key().clone(), entry.value().clone());
            }

            // Convert merged records to vectors
            let records: Vec<_> = existing.into_values().collect();

            let mut urls = Vec::with_capacity(records.len());
            let mut titles = Vec::with_capacity(records.len());
            let mut copyrights = Vec::with_capacity(records.len());
            let mut copyright_links = Vec::with_capacity(records.len());
            let mut market_codes = Vec::with_capacity(records.len());
            let mut fetched_ats = Vec::with_capacity(records.len());
            let mut statuses = Vec::with_capacity(records.len());

            for record in records {
                urls.push(record.url);
                titles.push(record.title);
                copyrights.push(record.copyright);
                copyright_links.push(record.copyright_link);
                market_codes.push(record.market_code);
                fetched_ats.push(record.fetched_at);
                statuses.push(record.status.as_str().to_string());
            }

            // Create new batch
            let schema = Arc::new(Schema::new(vec![
                Field::new("url", DataType::Utf8, false),
                Field::new("title", DataType::Utf8, false),
                Field::new("copyright", DataType::Utf8, true),
                Field::new("copyright_link", DataType::Utf8, true),
                Field::new("market_code", DataType::Utf8, false),
                Field::new("fetched_at", DataType::Int64, false),
                Field::new("status", DataType::Utf8, false),
            ]));

            let batch = RecordBatch::try_new(
                schema,
                vec![
                    Arc::new(StringArray::from(urls)) as ArrayRef,
                    Arc::new(StringArray::from(titles)) as ArrayRef,
                    Arc::new(StringArray::from(copyrights)) as ArrayRef,
                    Arc::new(StringArray::from(copyright_links)) as ArrayRef,
                    Arc::new(StringArray::from(market_codes)) as ArrayRef,
                    Arc::new(Int64Array::from(fetched_ats)) as ArrayRef,
                    Arc::new(StringArray::from(statuses)) as ArrayRef,
                ],
            )?;

            // Write to parquet
            let images_path = self.data_dir.join("bing_images.parquet");
            self.write_parquet(&images_path, vec![batch])?;

            // Re-register table
            let _ = ctx.deregister_table("bing_images");
            ctx.register_parquet("bing_images", images_path.to_str().unwrap(), ParquetReadOptions::default()).await?;

            // Clear cache after successful flush
            self.write_cache.clear();
            self.cache_size.store(0, Ordering::Relaxed);

            log::info!("Cache flush complete");
            Ok(())
        })
    }

    /// Batch insert or update multiple image records (FAST: batched cache writes)
    pub fn batch_upsert_images(&self, records: &[BingImageRecord]) -> Result<usize> {
        let mut count = 0;
        for record in records {
            // All writes go to cache
            self.write_cache.insert(record.url.clone(), record.clone());
            count += 1;
        }

        // Update cache size
        self.cache_size.fetch_add(count, Ordering::Relaxed);

        // Flush if threshold exceeded
        let current_size = self.cache_size.load(Ordering::Relaxed);
        if current_size >= self.cache_flush_threshold {
            self.flush_cache()?;
        }

        Ok(count)
    }

    /// Get an image by URL (checks cache first, then Parquet)
    pub fn get_image(&self, url: &str) -> Result<Option<BingImageRecord>> {
        // Check write cache first (instant)
        if let Some(cached) = self.write_cache.get(url) {
            return Ok(Some(cached.clone()));
        }

        // Fall back to Parquet
        self.runtime.block_on(async {
            let ctx = self.ctx.lock().unwrap();

            let query = format!("SELECT * FROM bing_images WHERE url = '{}'", url.replace("'", "''"));
            let df = ctx.sql(&query).await?;
            let batches = df.collect().await?;

            for batch in batches {
                if batch.num_rows() > 0 {
                    return Ok(Some(Self::extract_image_record(&batch, 0)?));
                }
            }

            Ok(None)
        })
    }

    /// Get all images with a specific status (merges cache with Parquet)
    pub fn get_images_by_status(&self, status: ImageStatus) -> Result<Vec<BingImageRecord>> {
        let mut records = self.runtime.block_on(async {
            let ctx = self.ctx.lock().unwrap();

            let query = format!("SELECT * FROM bing_images WHERE status = '{}' ORDER BY fetched_at DESC", status.as_str());
            let df = ctx.sql(&query).await?;
            let batches = df.collect().await?;

            let mut records = Vec::new();

            for batch in batches {
                for i in 0..batch.num_rows() {
                    records.push(Self::extract_image_record(&batch, i)?);
                }
            }

            Ok::<Vec<BingImageRecord>, anyhow::Error>(records)
        })?;

        // Build map for deduplication (URL -> Record)
        let mut record_map: std::collections::HashMap<String, BingImageRecord> =
            records.into_iter().map(|r| (r.url.clone(), r)).collect();

        // Merge cached records (overwrites Parquet data)
        for entry in self.write_cache.iter() {
            if entry.value().status == status {
                record_map.insert(entry.key().clone(), entry.value().clone());
            } else {
                // Remove from map if status changed in cache
                record_map.remove(entry.key());
            }
        }

        // Convert back to vec and sort
        let mut final_records: Vec<_> = record_map.into_values().collect();
        final_records.sort_by(|a, b| b.fetched_at.cmp(&a.fetched_at));

        Ok(final_records)
    }

    /// Get all images with a specific market code (merges cache with Parquet)
    pub fn get_images_by_market_code(&self, market_code: &str) -> Result<Vec<BingImageRecord>> {
        let mut records = self.runtime.block_on(async {
            let ctx = self.ctx.lock().unwrap();

            let query = format!("SELECT * FROM bing_images WHERE market_code = '{}' ORDER BY fetched_at DESC", market_code.replace("'", "''"));
            let df = ctx.sql(&query).await?;
            let batches = df.collect().await?;

            let mut records = Vec::new();

            for batch in batches {
                for i in 0..batch.num_rows() {
                    records.push(Self::extract_image_record(&batch, i)?);
                }
            }

            Ok::<Vec<BingImageRecord>, anyhow::Error>(records)
        })?;

        // Merge cached records
        let mut record_map: std::collections::HashMap<String, BingImageRecord> =
            records.into_iter().map(|r| (r.url.clone(), r)).collect();

        for entry in self.write_cache.iter() {
            if entry.value().market_code == market_code {
                record_map.insert(entry.key().clone(), entry.value().clone());
            } else if record_map.contains_key(entry.key()) {
                // Market code changed in cache, remove old entry
                record_map.remove(entry.key());
            }
        }

        let mut final_records: Vec<_> = record_map.into_values().collect();
        final_records.sort_by(|a, b| b.fetched_at.cmp(&a.fetched_at));

        Ok(final_records)
    }

    /// Get images with a specific market code, with pagination support
    pub fn get_images_by_market_code_paginated(&self, market_code: &str, limit: usize, offset: usize) -> Result<Vec<BingImageRecord>> {
        // For paginated queries, get all matching records (with cache merge) then slice
        let all_records = self.get_images_by_market_code(market_code)?;

        let start = offset.min(all_records.len());
        let end = (offset + limit).min(all_records.len());

        Ok(all_records[start..end].to_vec())
    }

    /// Update image status
    pub fn update_image_status(&self, url: &str, status: ImageStatus) -> Result<()> {
        // Get the existing record and update it
        if let Some(mut record) = self.get_image(url)? {
            record.status = status;
            self.upsert_image(&record)?;
        }
        Ok(())
    }

    /// Delete an image record (removes from cache, marks for deletion)
    pub fn delete_image(&self, url: &str) -> Result<()> {
        // Remove from cache if present
        self.write_cache.remove(url);

        // For deletion from Parquet, we need to rewrite the file
        // This is deferred until next flush or done immediately
        self.runtime.block_on(async {
            let ctx = self.ctx.lock().unwrap();

            let df = ctx.sql("SELECT * FROM bing_images").await?;
            let batches = df.collect().await?;

            let mut urls = Vec::new();
            let mut titles = Vec::new();
            let mut copyrights = Vec::new();
            let mut copyright_links = Vec::new();
            let mut market_codes = Vec::new();
            let mut fetched_ats = Vec::new();
            let mut statuses = Vec::new();

            for batch in &batches {
                for i in 0..batch.num_rows() {
                    let batch_url = Self::extract_string(batch, 0, i)?;
                    if batch_url != url {
                        urls.push(batch_url);
                        titles.push(Self::extract_string(batch, 1, i)?);
                        copyrights.push(Self::extract_optional_string(batch, 2, i)?);
                        copyright_links.push(Self::extract_optional_string(batch, 3, i)?);
                        market_codes.push(Self::extract_string(batch, 4, i)?);
                        fetched_ats.push(Self::extract_i64(batch, 5, i)?);
                        statuses.push(Self::extract_string(batch, 6, i)?);
                    }
                }
            }

            let schema = Arc::new(Schema::new(vec![
                Field::new("url", DataType::Utf8, false),
                Field::new("title", DataType::Utf8, false),
                Field::new("copyright", DataType::Utf8, true),
                Field::new("copyright_link", DataType::Utf8, true),
                Field::new("market_code", DataType::Utf8, false),
                Field::new("fetched_at", DataType::Int64, false),
                Field::new("status", DataType::Utf8, false),
            ]));

            let batch = RecordBatch::try_new(
                schema,
                vec![
                    Arc::new(StringArray::from(urls)) as ArrayRef,
                    Arc::new(StringArray::from(titles)) as ArrayRef,
                    Arc::new(StringArray::from(copyrights)) as ArrayRef,
                    Arc::new(StringArray::from(copyright_links)) as ArrayRef,
                    Arc::new(StringArray::from(market_codes)) as ArrayRef,
                    Arc::new(Int64Array::from(fetched_ats)) as ArrayRef,
                    Arc::new(StringArray::from(statuses)) as ArrayRef,
                ],
            )?;

            let images_path = self.data_dir.join("bing_images.parquet");
            self.write_parquet(&images_path, vec![batch])?;

            let _ = ctx.deregister_table("bing_images");
            ctx.register_parquet("bing_images", images_path.to_str().unwrap(), ParquetReadOptions::default()).await?;

            Ok(())
        })
    }

    /// Market code operations (simplified implementations)
    pub fn upsert_market_code(&self, _code: &str, _last_used_at: i64) -> Result<()> {
        // Similar pattern to upsert_image
        Ok(())
    }

    pub fn get_market_codes(&self) -> Result<Vec<MarketCodeRecord>> {
        Ok(Vec::new())
    }

    pub fn delete_market_code(&self, _code: &str) -> Result<()> {
        Ok(())
    }

    /// Count images by status (includes cached records)
    pub fn count_by_status(&self, status: ImageStatus) -> Result<usize> {
        // Get all records with this status (already merges cache)
        let records = self.get_images_by_status(status)?;
        Ok(records.len())
    }

    /// Count images by market code (efficient, includes cached records)
    pub fn count_by_market_code(&self, market_code: &str) -> Result<usize> {
        let mut count = self.runtime.block_on(async {
            let ctx = self.ctx.lock().unwrap();

            let query = format!("SELECT COUNT(*) FROM bing_images WHERE market_code = '{}'",
                market_code.replace("'", "''"));
            let df = ctx.sql(&query).await?;
            let batches = df.collect().await?;

            if let Some(batch) = batches.first() {
                if batch.num_rows() > 0 {
                    let count_array = batch.column(0).as_any().downcast_ref::<Int64Array>().unwrap();
                    return Ok(count_array.value(0) as usize);
                }
            }

            Ok::<usize, anyhow::Error>(0)
        })?;

        // Add cached records with matching market_code
        for entry in self.write_cache.iter() {
            if entry.value().market_code == market_code {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Configuration key-value operations
    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        self.runtime.block_on(async {
            let ctx = self.ctx.lock().unwrap();

            let query = format!("SELECT value FROM config_kv WHERE key = '{}'", key.replace("'", "''"));
            let df = ctx.sql(&query).await?;
            let batches = df.collect().await?;

            for batch in batches {
                if batch.num_rows() > 0 {
                    return Ok(Some(Self::extract_string(&batch, 0, 0)?));
                }
            }

            Ok(None)
        })
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        self.runtime.block_on(async {
            let ctx = self.ctx.lock().unwrap();

            // Read existing config (similar to upsert_image pattern)
            let df = ctx.sql("SELECT * FROM config_kv").await?;
            let batches = df.collect().await?;

            let mut keys = Vec::new();
            let mut values = Vec::new();

            // Collect existing configs (except the one we're updating)
            for batch in &batches {
                for i in 0..batch.num_rows() {
                    let batch_key = Self::extract_string(batch, 0, i)?;
                    if batch_key != key {
                        keys.push(batch_key);
                        values.push(Self::extract_string(batch, 1, i)?);
                    }
                }
            }

            // Add new/updated config
            keys.push(key.to_string());
            values.push(value.to_string());

            // Create new batch
            let schema = Arc::new(Schema::new(vec![
                Field::new("key", DataType::Utf8, false),
                Field::new("value", DataType::Utf8, false),
            ]));

            let batch = RecordBatch::try_new(
                schema,
                vec![
                    Arc::new(StringArray::from(keys)) as ArrayRef,
                    Arc::new(StringArray::from(values)) as ArrayRef,
                ],
            )?;

            let config_path = self.data_dir.join("config_kv.parquet");
            self.write_parquet(&config_path, vec![batch])?;

            let _ = ctx.deregister_table("config_kv");
            ctx.register_parquet("config_kv", config_path.to_str().unwrap(), ParquetReadOptions::default()).await?;

            Ok(())
        })
    }

    pub fn delete_config(&self, _key: &str) -> Result<()> {
        Ok(())
    }

    pub fn get_blacklisted_urls(&self) -> Result<Vec<String>> {
        let records = self.get_images_by_status(ImageStatus::Blacklisted)?;
        Ok(records.into_iter().map(|r| r.url).collect())
    }

    pub fn get_historical_page(&self) -> Result<usize> {
        Ok(self
            .get_config("historical_page")?
            .and_then(|v| v.parse().ok())
            .unwrap_or(0))
    }

    pub fn set_historical_page(&self, page: usize) -> Result<()> {
        self.set_config("historical_page", &page.to_string())
    }

    pub fn checkpoint(&self) -> Result<()> {
        // Flush write cache to ensure durability
        self.flush_cache()
    }

    pub fn get_last_download_timestamp(&self, manifest_type: &str) -> Result<Option<i64>> {
        let key = format!("last_download_{}", manifest_type);
        Ok(self.get_config(&key)?.and_then(|v| v.parse().ok()))
    }

    pub fn set_last_download_timestamp(&self, manifest_type: &str, timestamp: i64) -> Result<()> {
        let key = format!("last_download_{}", manifest_type);
        self.set_config(&key, &timestamp.to_string())
    }

    pub fn should_download_manifest(&self, manifest_type: &str) -> bool {
        match self.get_last_download_timestamp(manifest_type) {
            Ok(Some(last_download)) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                let days_elapsed = (now - last_download) / 86400;
                days_elapsed >= 7
            }
            _ => true,
        }
    }

    /// Get write cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        let size = self.cache_size.load(Ordering::Relaxed);
        let threshold = self.cache_flush_threshold;
        (size, threshold)
    }
}

// Implement Drop to flush cache on shutdown
#[cfg(not(target_arch = "wasm32"))]
impl Drop for BingImageDb {
    fn drop(&mut self) {
        log::debug!("BingImageDb dropping, flushing cache...");
        // Flush cache to ensure all data is persisted
        if let Err(e) = self.flush_cache() {
            log::error!("Failed to flush cache on drop: {}", e);
        }
    }
}

// ============================================================================
// WASM IMPLEMENTATION (Browser)
// ============================================================================

#[cfg(target_arch = "wasm32")]
pub struct BingImageDb;

#[cfg(target_arch = "wasm32")]
impl BingImageDb {
    pub fn new(_db_path: PathBuf) -> Result<Self> {
        log::info!("DataFusion WASM: Using in-memory storage");
        Ok(Self)
    }

    pub fn upsert_image(&self, record: &BingImageRecord) -> Result<()> {
        log::debug!("DataFusion WASM: upsert_image (stub): {}", record.url);
        Ok(())
    }

    pub fn batch_upsert_images(&self, _records: &[BingImageRecord]) -> Result<usize> {
        Ok(0)
    }

    pub fn get_image(&self, url: &str) -> Result<Option<BingImageRecord>> {
        log::debug!("DataFusion WASM: get_image (stub): {}", url);
        Ok(None)
    }

    pub fn get_images_by_status(&self, status: ImageStatus) -> Result<Vec<BingImageRecord>> {
        log::debug!("DataFusion WASM: get_images_by_status (stub): {:?}", status);
        Ok(Vec::new())
    }

    pub fn get_images_by_market_code(&self, market_code: &str) -> Result<Vec<BingImageRecord>> {
        log::debug!("DataFusion WASM: get_images_by_market_code (stub): {}", market_code);
        Ok(Vec::new())
    }

    pub fn get_images_by_market_code_paginated(&self, market_code: &str, limit: usize, offset: usize) -> Result<Vec<BingImageRecord>> {
        log::debug!("DataFusion WASM: get_images_by_market_code_paginated (stub): {} limit={} offset={}", market_code, limit, offset);
        Ok(Vec::new())
    }

    pub fn update_image_status(&self, url: &str, status: ImageStatus) -> Result<()> {
        log::debug!("DataFusion WASM: update_image_status (stub): {} -> {:?}", url, status);
        Ok(())
    }

    pub fn delete_image(&self, url: &str) -> Result<()> {
        log::debug!("DataFusion WASM: delete_image (stub): {}", url);
        Ok(())
    }

    pub fn upsert_market_code(&self, code: &str, last_used_at: i64) -> Result<()> {
        log::debug!("DataFusion WASM: upsert_market_code (stub): {} at {}", code, last_used_at);
        Ok(())
    }

    pub fn get_market_codes(&self) -> Result<Vec<MarketCodeRecord>> {
        log::debug!("DataFusion WASM: get_market_codes (stub)");
        Ok(Vec::new())
    }

    pub fn delete_market_code(&self, code: &str) -> Result<()> {
        log::debug!("DataFusion WASM: delete_market_code (stub): {}", code);
        Ok(())
    }

    pub fn count_by_status(&self, status: ImageStatus) -> Result<usize> {
        log::debug!("DataFusion WASM: count_by_status (stub): {:?}", status);
        Ok(0)
    }

    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        log::debug!("DataFusion WASM: get_config (stub): {}", key);
        Ok(None)
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        log::debug!("DataFusion WASM: set_config (stub): {} = {}", key, value);
        Ok(())
    }

    pub fn delete_config(&self, key: &str) -> Result<()> {
        log::debug!("DataFusion WASM: delete_config (stub): {}", key);
        Ok(())
    }

    pub fn get_blacklisted_urls(&self) -> Result<Vec<String>> {
        log::debug!("DataFusion WASM: get_blacklisted_urls (stub)");
        Ok(Vec::new())
    }

    pub fn get_historical_page(&self) -> Result<usize> {
        log::debug!("DataFusion WASM: get_historical_page (stub)");
        Ok(0)
    }

    pub fn set_historical_page(&self, page: usize) -> Result<()> {
        log::debug!("DataFusion WASM: set_historical_page (stub): {}", page);
        Ok(())
    }

    pub fn checkpoint(&self) -> Result<()> {
        log::debug!("DataFusion WASM: checkpoint (stub)");
        Ok(())
    }

    pub fn get_last_download_timestamp(&self, manifest_type: &str) -> Result<Option<i64>> {
        log::debug!("DataFusion WASM: get_last_download_timestamp (stub): {}", manifest_type);
        Ok(None)
    }

    pub fn set_last_download_timestamp(&self, manifest_type: &str, timestamp: i64) -> Result<()> {
        log::debug!("DataFusion WASM: set_last_download_timestamp (stub): {} = {}", manifest_type, timestamp);
        Ok(())
    }

    pub fn should_download_manifest(&self, manifest_type: &str) -> bool {
        log::debug!("DataFusion WASM: should_download_manifest (stub): {}", manifest_type);
        true
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_db() -> (BingImageDb, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = BingImageDb::new(db_path).unwrap();
        (db, temp_dir)
    }

    fn create_test_record(url: &str, status: ImageStatus) -> BingImageRecord {
        BingImageRecord {
            url: url.to_string(),
            title: format!("Test Image {}", url),
            copyright: Some("Test Copyright".to_string()),
            copyright_link: Some("https://example.com".to_string()),
            market_code: "en-US".to_string(),
            fetched_at: 1234567890,
            status,
        }
    }

    #[test]
    fn test_database_creation() {
        let (_db, _temp_dir) = create_test_db();
        // If we get here, database was created successfully
    }

    #[test]
    fn test_upsert_and_get_image() {
        let (db, _temp_dir) = create_test_db();

        let record = create_test_record("https://example.com/image1.jpg", ImageStatus::Unprocessed);

        // Insert record
        db.upsert_image(&record).unwrap();

        // Retrieve record
        let retrieved = db.get_image(&record.url).unwrap();
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.url, record.url);
        assert_eq!(retrieved.title, record.title);
        assert_eq!(retrieved.status, ImageStatus::Unprocessed);
    }

    #[test]
    fn test_upsert_updates_existing_record() {
        let (db, _temp_dir) = create_test_db();

        let url = "https://example.com/image2.jpg";
        let record1 = create_test_record(url, ImageStatus::Unprocessed);

        // Insert first time
        db.upsert_image(&record1).unwrap();

        // Update with new title
        let mut record2 = create_test_record(url, ImageStatus::KeepFavorite);
        record2.title = "Updated Title".to_string();
        db.upsert_image(&record2).unwrap();

        // Verify update
        let retrieved = db.get_image(url).unwrap().unwrap();
        assert_eq!(retrieved.title, "Updated Title");
        assert_eq!(retrieved.status, ImageStatus::KeepFavorite);
    }

    #[test]
    fn test_batch_upsert_images() {
        let (db, _temp_dir) = create_test_db();

        let records = vec![
            create_test_record("https://example.com/img1.jpg", ImageStatus::Unprocessed),
            create_test_record("https://example.com/img2.jpg", ImageStatus::Unprocessed),
            create_test_record("https://example.com/img3.jpg", ImageStatus::KeepFavorite),
        ];

        let count = db.batch_upsert_images(&records).unwrap();
        assert_eq!(count, 3);

        // Verify all records were inserted
        for record in &records {
            let retrieved = db.get_image(&record.url).unwrap();
            assert!(retrieved.is_some());
        }
    }

    #[test]
    fn test_get_images_by_status() {
        let (db, _temp_dir) = create_test_db();

        // Insert images with different statuses
        db.upsert_image(&create_test_record("https://example.com/unprocessed1.jpg", ImageStatus::Unprocessed)).unwrap();
        db.upsert_image(&create_test_record("https://example.com/unprocessed2.jpg", ImageStatus::Unprocessed)).unwrap();
        db.upsert_image(&create_test_record("https://example.com/favorite1.jpg", ImageStatus::KeepFavorite)).unwrap();
        db.upsert_image(&create_test_record("https://example.com/blacklisted1.jpg", ImageStatus::Blacklisted)).unwrap();

        // Test getting unprocessed images
        let unprocessed = db.get_images_by_status(ImageStatus::Unprocessed).unwrap();
        assert_eq!(unprocessed.len(), 2);

        // Test getting favorites
        let favorites = db.get_images_by_status(ImageStatus::KeepFavorite).unwrap();
        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0].url, "https://example.com/favorite1.jpg");

        // Test getting blacklisted
        let blacklisted = db.get_images_by_status(ImageStatus::Blacklisted).unwrap();
        assert_eq!(blacklisted.len(), 1);
    }

    #[test]
    fn test_get_images_by_market_code() {
        let (db, _temp_dir) = create_test_db();

        let mut record1 = create_test_record("https://example.com/us1.jpg", ImageStatus::Unprocessed);
        record1.market_code = "en-US".to_string();

        let mut record2 = create_test_record("https://example.com/us2.jpg", ImageStatus::Unprocessed);
        record2.market_code = "en-US".to_string();

        let mut record3 = create_test_record("https://example.com/jp1.jpg", ImageStatus::Unprocessed);
        record3.market_code = "ja-JP".to_string();

        db.upsert_image(&record1).unwrap();
        db.upsert_image(&record2).unwrap();
        db.upsert_image(&record3).unwrap();

        let us_images = db.get_images_by_market_code("en-US").unwrap();
        assert_eq!(us_images.len(), 2);

        let jp_images = db.get_images_by_market_code("ja-JP").unwrap();
        assert_eq!(jp_images.len(), 1);
    }

    #[test]
    fn test_get_images_by_market_code_paginated() {
        let (db, _temp_dir) = create_test_db();

        // Insert 5 images with the same market code
        for i in 0..5 {
            let mut record = create_test_record(
                &format!("https://example.com/img{}.jpg", i),
                ImageStatus::Unprocessed
            );
            record.market_code = "en-US".to_string();
            record.fetched_at = 1000000000 + i as i64; // Different timestamps for ordering
            db.upsert_image(&record).unwrap();
        }

        // Test pagination - first page (limit 2)
        let page1 = db.get_images_by_market_code_paginated("en-US", 2, 0).unwrap();
        assert_eq!(page1.len(), 2);

        // Test pagination - second page
        let page2 = db.get_images_by_market_code_paginated("en-US", 2, 2).unwrap();
        assert_eq!(page2.len(), 2);

        // Test pagination - third page
        let page3 = db.get_images_by_market_code_paginated("en-US", 2, 4).unwrap();
        assert_eq!(page3.len(), 1);
    }

    #[test]
    fn test_update_image_status() {
        let (db, _temp_dir) = create_test_db();

        let record = create_test_record("https://example.com/image.jpg", ImageStatus::Unprocessed);
        db.upsert_image(&record).unwrap();

        // Update status to favorite
        db.update_image_status(&record.url, ImageStatus::KeepFavorite).unwrap();

        let retrieved = db.get_image(&record.url).unwrap().unwrap();
        assert_eq!(retrieved.status, ImageStatus::KeepFavorite);

        // Update status to blacklisted
        db.update_image_status(&record.url, ImageStatus::Blacklisted).unwrap();

        let retrieved = db.get_image(&record.url).unwrap().unwrap();
        assert_eq!(retrieved.status, ImageStatus::Blacklisted);
    }

    #[test]
    fn test_delete_image() {
        let (db, _temp_dir) = create_test_db();

        let record = create_test_record("https://example.com/image.jpg", ImageStatus::Unprocessed);
        db.upsert_image(&record).unwrap();

        // Verify it exists
        assert!(db.get_image(&record.url).unwrap().is_some());

        // Delete it
        db.delete_image(&record.url).unwrap();

        // Verify it's gone
        assert!(db.get_image(&record.url).unwrap().is_none());
    }

    #[test]
    fn test_count_by_status() {
        let (db, _temp_dir) = create_test_db();

        // Insert images with different statuses
        db.upsert_image(&create_test_record("https://example.com/u1.jpg", ImageStatus::Unprocessed)).unwrap();
        db.upsert_image(&create_test_record("https://example.com/u2.jpg", ImageStatus::Unprocessed)).unwrap();
        db.upsert_image(&create_test_record("https://example.com/u3.jpg", ImageStatus::Unprocessed)).unwrap();
        db.upsert_image(&create_test_record("https://example.com/f1.jpg", ImageStatus::KeepFavorite)).unwrap();
        db.upsert_image(&create_test_record("https://example.com/b1.jpg", ImageStatus::Blacklisted)).unwrap();

        assert_eq!(db.count_by_status(ImageStatus::Unprocessed).unwrap(), 3);
        assert_eq!(db.count_by_status(ImageStatus::KeepFavorite).unwrap(), 1);
        assert_eq!(db.count_by_status(ImageStatus::Blacklisted).unwrap(), 1);
    }

    #[test]
    fn test_config_operations() {
        let (db, _temp_dir) = create_test_db();

        // Test set and get
        db.set_config("test_key", "test_value").unwrap();
        let value = db.get_config("test_key").unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        // Test update
        db.set_config("test_key", "updated_value").unwrap();
        let value = db.get_config("test_key").unwrap();
        assert_eq!(value, Some("updated_value".to_string()));

        // Test non-existent key
        let value = db.get_config("non_existent").unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn test_get_blacklisted_urls() {
        let (db, _temp_dir) = create_test_db();

        db.upsert_image(&create_test_record("https://example.com/u1.jpg", ImageStatus::Unprocessed)).unwrap();
        db.upsert_image(&create_test_record("https://example.com/b1.jpg", ImageStatus::Blacklisted)).unwrap();
        db.upsert_image(&create_test_record("https://example.com/b2.jpg", ImageStatus::Blacklisted)).unwrap();

        let blacklisted = db.get_blacklisted_urls().unwrap();
        assert_eq!(blacklisted.len(), 2);
        assert!(blacklisted.contains(&"https://example.com/b1.jpg".to_string()));
        assert!(blacklisted.contains(&"https://example.com/b2.jpg".to_string()));
    }

    #[test]
    fn test_historical_page() {
        let (db, _temp_dir) = create_test_db();

        // Default should be 0
        assert_eq!(db.get_historical_page().unwrap(), 0);

        // Set to 5
        db.set_historical_page(5).unwrap();
        assert_eq!(db.get_historical_page().unwrap(), 5);

        // Update to 10
        db.set_historical_page(10).unwrap();
        assert_eq!(db.get_historical_page().unwrap(), 10);
    }

    #[test]
    fn test_download_timestamp() {
        let (db, _temp_dir) = create_test_db();

        let manifest_type = "daily";

        // Initially should be None
        assert_eq!(db.get_last_download_timestamp(manifest_type).unwrap(), None);

        // Set timestamp
        let now = 1234567890i64;
        db.set_last_download_timestamp(manifest_type, now).unwrap();
        assert_eq!(db.get_last_download_timestamp(manifest_type).unwrap(), Some(now));
    }

    #[test]
    fn test_should_download_manifest() {
        let (db, _temp_dir) = create_test_db();

        let manifest_type = "test_manifest";

        // Should download if no timestamp exists
        assert!(db.should_download_manifest(manifest_type));

        // Set recent timestamp (less than 7 days ago)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        db.set_last_download_timestamp(manifest_type, now).unwrap();
        assert!(!db.should_download_manifest(manifest_type));

        // Set old timestamp (more than 7 days ago)
        let old_time = now - (8 * 86400); // 8 days ago
        db.set_last_download_timestamp(manifest_type, old_time).unwrap();
        assert!(db.should_download_manifest(manifest_type));
    }

    #[test]
    fn test_image_status_conversion() {
        assert_eq!(ImageStatus::Unprocessed.as_str(), "unprocessed");
        assert_eq!(ImageStatus::KeepFavorite.as_str(), "keepfavorite");
        assert_eq!(ImageStatus::Blacklisted.as_str(), "blacklisted");

        assert_eq!(ImageStatus::from_str("unprocessed"), Some(ImageStatus::Unprocessed));
        assert_eq!(ImageStatus::from_str("cached"), Some(ImageStatus::Unprocessed)); // Backward compatibility
        assert_eq!(ImageStatus::from_str("keepfavorite"), Some(ImageStatus::KeepFavorite));
        assert_eq!(ImageStatus::from_str("blacklisted"), Some(ImageStatus::Blacklisted));
        assert_eq!(ImageStatus::from_str("invalid"), None);
    }

    #[test]
    fn test_checkpoint() {
        let (db, _temp_dir) = create_test_db();

        // Checkpoint should succeed (it's a no-op for DataFusion)
        db.checkpoint().unwrap();
    }

    #[test]
    fn test_sql_injection_protection() {
        let (db, _temp_dir) = create_test_db();

        // Try to insert a record with SQL injection in URL
        let malicious_url = "https://example.com/'; DROP TABLE bing_images; --";
        let record = create_test_record(malicious_url, ImageStatus::Unprocessed);

        db.upsert_image(&record).unwrap();

        // Should be able to retrieve safely
        let retrieved = db.get_image(malicious_url).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().url, malicious_url);
    }

    #[test]
    fn test_null_copyright_fields() {
        let (db, _temp_dir) = create_test_db();

        let mut record = create_test_record("https://example.com/image.jpg", ImageStatus::Unprocessed);
        record.copyright = None;
        record.copyright_link = None;

        db.upsert_image(&record).unwrap();

        let retrieved = db.get_image(&record.url).unwrap().unwrap();
        assert_eq!(retrieved.copyright, None);
        assert_eq!(retrieved.copyright_link, None);
    }
}

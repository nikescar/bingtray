use diesel::prelude::*;
use anyhow::Result;
use crate::schema::{bing_images, market_codes, config_kv};
use super::models::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn current_timestamp() -> i32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32
}

/// Insert or update a Bing image record
pub fn upsert_image(conn: &mut SqliteConnection, record: &NewBingImage) -> Result<BingImage> {
    use diesel::RunQueryDsl;

    // Check if URL already exists
    let existing: Option<BingImage> = bing_images::table
        .filter(bing_images::url.eq(record.url))
        .first(conn)
        .optional()?;

    if let Some(existing_img) = existing {
        // Update existing record but preserve status (don't overwrite user's keep/blacklist)
        diesel::update(bing_images::table.find(existing_img.id))
            .set((
                bing_images::title.eq(record.title),
                bing_images::copyright.eq(record.copyright),
                bing_images::copyright_link.eq(record.copyright_link),
                bing_images::market_code.eq(record.market_code),
                bing_images::fetched_at.eq(record.fetched_at),
                // DO NOT update status - preserve user's keep/blacklist choices
                bing_images::updated_at.eq(current_timestamp()),
            ))
            .execute(conn)?;

        bing_images::table
            .find(existing_img.id)
            .first(conn)
            .map_err(Into::into)
    } else {
        // Insert new record
        diesel::insert_into(bing_images::table)
            .values(record)
            .execute(conn)?;

        bing_images::table
            .order(bing_images::id.desc())
            .first(conn)
            .map_err(Into::into)
    }
}

/// Get an image by URL
pub fn get_image(conn: &mut SqliteConnection, url: &str) -> Result<Option<BingImage>> {
    bing_images::table
        .filter(bing_images::url.eq(url))
        .first(conn)
        .optional()
        .map_err(Into::into)
}

/// Get all images with a specific status
pub fn get_images_by_status(conn: &mut SqliteConnection, status: ImageStatus) -> Result<Vec<BingImage>> {
    bing_images::table
        .filter(bing_images::status.eq(status.as_str()))
        .order(bing_images::fetched_at.desc())
        .load(conn)
        .map_err(Into::into)
}

/// Get images by market code with pagination
pub fn get_images_by_market_code(
    conn: &mut SqliteConnection,
    market_code: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<BingImage>> {
    bing_images::table
        .filter(bing_images::market_code.eq(market_code))
        .order(bing_images::fetched_at.desc())
        .limit(limit)
        .offset(offset)
        .load(conn)
        .map_err(Into::into)
}

/// Update image status
pub fn update_image_status(conn: &mut SqliteConnection, url: &str, status: ImageStatus) -> Result<()> {
    diesel::update(bing_images::table.filter(bing_images::url.eq(url)))
        .set((
            bing_images::status.eq(status.as_str()),
            bing_images::updated_at.eq(current_timestamp()),
        ))
        .execute(conn)?;
    Ok(())
}

/// Delete an image by URL
pub fn delete_image(conn: &mut SqliteConnection, url: &str) -> Result<()> {
    diesel::delete(bing_images::table.filter(bing_images::url.eq(url)))
        .execute(conn)?;
    Ok(())
}

/// Count images by status
pub fn count_by_status(conn: &mut SqliteConnection, status: ImageStatus) -> Result<i64> {
    bing_images::table
        .filter(bing_images::status.eq(status.as_str()))
        .count()
        .get_result(conn)
        .map_err(Into::into)
}

/// Count images by market code
pub fn count_by_market_code(conn: &mut SqliteConnection, market_code: &str) -> Result<i64> {
    bing_images::table
        .filter(bing_images::market_code.eq(market_code))
        .count()
        .get_result(conn)
        .map_err(Into::into)
}

/// Get config value by key
pub fn get_config(conn: &mut SqliteConnection, key: &str) -> Result<Option<String>> {
    config_kv::table
        .filter(config_kv::key.eq(key))
        .select(config_kv::value)
        .first(conn)
        .optional()
        .map_err(Into::into)
}

/// Set config value
pub fn set_config(conn: &mut SqliteConnection, key: &str, value: &str) -> Result<()> {
    let existing: Option<ConfigKv> = config_kv::table
        .filter(config_kv::key.eq(key))
        .first(conn)
        .optional()?;

    if let Some(existing_config) = existing {
        diesel::update(config_kv::table.find(existing_config.id))
            .set((
                config_kv::value.eq(value),
                config_kv::updated_at.eq(current_timestamp()),
            ))
            .execute(conn)?;
    } else {
        let new_config = NewConfigKv {
            key,
            value,
            created_at: current_timestamp(),
            updated_at: current_timestamp(),
        };
        diesel::insert_into(config_kv::table)
            .values(&new_config)
            .execute(conn)?;
    }
    Ok(())
}

/// Get all blacklisted URLs
pub fn get_blacklisted_urls(conn: &mut SqliteConnection) -> Result<Vec<String>> {
    bing_images::table
        .filter(bing_images::status.eq(ImageStatus::Blacklisted.as_str()))
        .select(bing_images::url)
        .load(conn)
        .map_err(Into::into)
}

/// Get historical page number
pub fn get_historical_page(conn: &mut SqliteConnection) -> Result<usize> {
    Ok(get_config(conn, "historical_page")?
        .and_then(|v| v.parse().ok())
        .unwrap_or(0))
}

/// Set historical page number
pub fn set_historical_page(conn: &mut SqliteConnection, page: usize) -> Result<()> {
    set_config(conn, "historical_page", &page.to_string())
}

/// Get last download timestamp for manifest type
pub fn get_last_download_timestamp(conn: &mut SqliteConnection, manifest_type: &str) -> Result<Option<i64>> {
    let key = format!("last_download_{}", manifest_type);
    Ok(get_config(conn, &key)?.and_then(|v| v.parse().ok()))
}

/// Set last download timestamp for manifest type
pub fn set_last_download_timestamp(conn: &mut SqliteConnection, manifest_type: &str, timestamp: i64) -> Result<()> {
    let key = format!("last_download_{}", manifest_type);
    set_config(conn, &key, &timestamp.to_string())
}

/// Check if should download manifest (>7 days old)
pub fn should_download_manifest(conn: &mut SqliteConnection, manifest_type: &str) -> bool {
    match get_last_download_timestamp(conn, manifest_type) {
        Ok(Some(last_download)) => {
            let now = current_timestamp() as i64;
            let days_elapsed = (now - last_download) / 86400;
            days_elapsed >= 7
        }
        _ => true,
    }
}

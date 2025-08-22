



// WASM functions for SQLite and HTTP operations
pub fn load_market_codes(_config: &Config) -> Result<HashMap<String, i64>> {
    // In WASM, market codes are loaded via SqliteDb in the wasm module
    // This function is kept for compatibility but should use wasm::SqliteDb
    Ok(HashMap::new())
}

pub fn download_images_for_market(_config: &Config, _market_code: &str, _thumb_mode: bool) -> Result<(usize, Vec<BingImage>)> {
    // In WASM, images are downloaded via HttpClient in the wasm module
    // This function is kept for compatibility but should use wasm::HttpClient
    Ok((0, Vec::new()))
}

pub fn download_historical_data(_config: &Config, _starting_index: usize) -> Result<Vec<HistoricalImage>> {
    // In WASM, use HttpClient::download_historical_data instead
    Ok(Vec::new())
}

pub fn download_image(_image: &BingImage, _target_dir: &std::path::Path, _config: &Config) -> Result<std::path::PathBuf> {
    // In WASM, images are not downloaded to filesystem but handled via HttpClient::download_image_bytes
    use std::path::PathBuf;
    Ok(PathBuf::from("/tmp/placeholder.jpg"))
}

pub fn download_thumbnail_image(_image: &BingImage, _config: &Config) -> Result<std::path::PathBuf> {
    // In WASM, thumbnails are not downloaded to filesystem but handled via HttpClient::download_thumbnail_bytes
    use std::path::PathBuf;
    Ok(PathBuf::from("/tmp/placeholder_thumb.jpg"))
}


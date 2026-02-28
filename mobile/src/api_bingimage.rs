use crate::{BingImage, BingResponse};
use eframe::egui;
use std::sync::{Arc, Mutex};

/// State of a Bing API fetch operation
#[derive(Debug, Clone)]
pub enum BingFetchState {
    Idle,
    InProgress,
    Done(Result<Vec<BingImage>, String>),
}

/// State of an image download operation
#[derive(Debug, Clone)]
pub enum ImageFetchState {
    Idle,
    InProgress { url: String },
    Done(Result<(String, Vec<u8>), String>), // (url, bytes)
}

/// Fetch Bing images from the API
///
/// # Arguments
/// * `market_code` - Market/region code (e.g., "en-US", "ja-JP")
/// * `count` - Number of images to fetch (max 8)
/// * `offset` - Offset for pagination (0-7)
/// * `ctx` - egui Context for requesting repaints
/// * `result_store` - Shared state to store results
pub fn fetch_bing_images(
    market_code: String,
    count: u32,
    offset: u32,
    ctx: egui::Context,
    result_store: Arc<Mutex<BingFetchState>>,
) {
    // Set state to in progress
    *result_store.lock().unwrap() = BingFetchState::InProgress;

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

    // Fetch asynchronously
    ehttp::fetch(request, move |response| {
        let result = match response {
            Ok(resp) => {
                if resp.ok {
                    // Parse JSON response
                    let text = resp.text().unwrap_or("");
                    match parse_bing_response(text) {
                        Ok(images) => {
                            log::info!("Successfully fetched {} Bing images", images.len());
                            Ok(images)
                        }
                        Err(e) => {
                            log::error!("Failed to parse Bing response: {}", e);
                            Err(format!("Parse error: {}", e))
                        }
                    }
                } else {
                    let error = format!("HTTP {}: {}", resp.status, resp.status_text);
                    log::error!("Bing API error: {}", error);
                    Err(error)
                }
            }
            Err(e) => {
                log::error!("Network error fetching Bing images: {}", e);
                Err(e)
            }
        };

        *result_store.lock().unwrap() = BingFetchState::Done(result);
        ctx.request_repaint(); // Wake up UI thread
    });
}

/// Download image bytes from a URL
///
/// # Arguments
/// * `url` - Full image URL
/// * `ctx` - egui Context for requesting repaints
/// * `result_store` - Shared state to store results
pub fn fetch_image_bytes(
    url: String,
    ctx: egui::Context,
    result_store: Arc<Mutex<ImageFetchState>>,
) {
    // Set state to in progress
    *result_store.lock().unwrap() = ImageFetchState::InProgress { url: url.clone() };

    log::info!("Downloading image: {}", url);

    // Create request with User-Agent
    let mut request = ehttp::Request::get(&url);
    request.headers.insert(
        "User-Agent".to_string(),
        format!("bingtray/{}", env!("CARGO_PKG_VERSION")),
    );

    let url_clone = url.clone();

    // Fetch asynchronously
    ehttp::fetch(request, move |response| {
        let result = match response {
            Ok(resp) => {
                if resp.ok {
                    log::info!(
                        "Successfully downloaded image: {} ({} bytes)",
                        url_clone,
                        resp.bytes.len()
                    );
                    Ok((url_clone, resp.bytes))
                } else {
                    let error = format!("HTTP {}: {}", resp.status, resp.status_text);
                    log::error!("Image download error: {}", error);
                    Err(error)
                }
            }
            Err(e) => {
                log::error!("Network error downloading image: {}", e);
                Err(e)
            }
        };

        *result_store.lock().unwrap() = ImageFetchState::Done(result);
        ctx.request_repaint(); // Wake up UI thread
    });
}

/// Parse Bing API JSON response
fn parse_bing_response(json: &str) -> Result<Vec<BingImage>, String> {
    let response: BingResponse =
        serde_json::from_str(json).map_err(|e| format!("JSON parse error: {}", e))?;

    // Convert to full URLs
    let images = response
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bing_response() {
        let json = r#"{
            "images": [
                {
                    "startdate": "20260227",
                    "fullstartdate": "202602270800",
                    "enddate": "20260228",
                    "url": "/th?id=OHR.BingWallpaper_EN-US1234567890_1920x1080.jpg",
                    "urlbase": "/th?id=OHR.BingWallpaper_EN-US1234567890",
                    "copyright": "Test Image",
                    "copyrightlink": "https://example.com",
                    "title": "Test Title",
                    "hsh": "abc123"
                }
            ]
        }"#;

        let result = parse_bing_response(json);
        assert!(result.is_ok());

        let images = result.unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].title, "Test Title");
        assert!(images[0].url.starts_with("https://www.bing.com/"));
    }
}

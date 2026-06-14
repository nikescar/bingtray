use crate::{BingImage, BingResponse};
use anyhow::{Context, Result};
use std::sync::Arc;
use std::sync::mpsc;

/// Extract identifier from Bing URL (e.g., "OHR.Hnausapollur" from full URL)
pub fn extract_identifier(url: &str) -> Option<String> {
    // URL format: https://www.bing.com/th?id=OHR.Hnausapollur_EN-US2080493040_1920x1080.jpg
    url.split("th?id=")
        .nth(1)?
        .split('_')
        .next()
        .map(|s| s.to_string())
}

/// Bing API image source (en-US market only)
pub struct BingApiSource {
    ehttp_cache: Option<Arc<crate::ehttp_cache::EhttpCache>>,
}

impl BingApiSource {
    pub fn new(ehttp_cache: Option<Arc<crate::ehttp_cache::EhttpCache>>) -> Self {
        Self { ehttp_cache }
    }

    /// Fetch images from Bing API (en-US, offset=0, n=count)
    pub fn fetch(&self, count: u32) -> Result<Vec<BingImage>> {
        let url = format!(
            "https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n={}&mkt=en-US",
            count.min(8) // Bing API max is 8
        );

        log::info!("Fetching from Bing API: {}", url);

        // Create request with User-Agent
        let mut request = ehttp::Request::get(&url);
        request.headers.insert(
            "User-Agent".to_string(),
            format!("bingtray/{}", env!("CARGO_PKG_VERSION")),
        );

        // Fetch synchronously
        let (tx, rx) = mpsc::channel();
        ehttp::fetch(request, move |response| {
            let _ = tx.send(response);
        });

        let response = rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .context("Timeout waiting for Bing API")?;

        let resp = response.map_err(|e| anyhow::anyhow!("Network error: {}", e))?;

        if !resp.ok {
            anyhow::bail!("HTTP {}: {}", resp.status, resp.status_text);
        }

        // Parse JSON
        let text = resp.text().context("Invalid UTF-8")?;
        let bing_response: BingResponse = serde_json::from_str(text)
            .context("Failed to parse JSON")?;

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

        log::info!("Fetched {} images from Bing API", images.len());
        Ok(images)
    }
}

/// Main image source interface (will add GitHub later)
pub struct ImageSource {
    bing_api: BingApiSource,
}

impl ImageSource {
    pub fn new(ehttp_cache: Option<Arc<crate::ehttp_cache::EhttpCache>>) -> Self {
        Self {
            bing_api: BingApiSource::new(ehttp_cache),
        }
    }

    /// Fetch images (currently Bing API only)
    pub fn fetch_images(&self, count: usize) -> Result<Vec<BingImage>> {
        self.bing_api.fetch(count as u32)
    }
}

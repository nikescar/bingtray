use crate::{BingImage, BingResponse};
use anyhow::{Context, Result};
use std::sync::Arc;
use std::sync::mpsc;
use regex::Regex;

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

/// Parse a markdown table row from GitHub archive
/// Format: | Date | Title | [Download](URL) | Copyright |
pub fn parse_markdown_row(row: &str) -> Option<BingImage> {
    let parts: Vec<&str> = row.split('|').map(|s| s.trim()).collect();
    if parts.len() < 5 {
        return None;
    }

    let title = parts[2].to_string();

    // Extract URL from markdown link [Download](URL)
    let url_regex = Regex::new(r"\[.*?\]\((.*?)\)").ok()?;
    let url = url_regex
        .captures(parts[3])?
        .get(1)?
        .as_str()
        .to_string();

    let copyright = if parts[4].is_empty() {
        None
    } else {
        Some(parts[4].to_string())
    };

    Some(BingImage {
        url,
        title,
        copyright,
        copyright_link: None,
    })
}

/// GitHub archive image source
pub struct GitHubArchiveSource {
    ehttp_cache: Option<Arc<crate::ehttp_cache::EhttpCache>>,
}

impl GitHubArchiveSource {
    pub fn new(ehttp_cache: Option<Arc<crate::ehttp_cache::EhttpCache>>) -> Self {
        Self { ehttp_cache }
    }

    /// Fetch images from GitHub archive (cached 7 days)
    pub fn fetch(&self) -> Result<Vec<BingImage>> {
        let url = "https://github.com/v5tech/bing-wallpaper/blob/main/bing-wallpaper.md?plain=1";

        log::info!("Fetching from GitHub archive: {}", url);

        let mut request = ehttp::Request::get(url);
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
            .context("Timeout waiting for GitHub")?;

        let resp = response.map_err(|e| anyhow::anyhow!("Network error: {}", e))?;

        if !resp.ok {
            anyhow::bail!("HTTP {}: {}", resp.status, resp.status_text);
        }

        // Parse markdown
        let text = resp.text().context("Invalid UTF-8")?;
        let images: Vec<BingImage> = text
            .lines()
            .filter(|line| line.starts_with('|') && !line.contains("Date"))
            .filter_map(parse_markdown_row)
            .collect();

        log::info!("Parsed {} images from GitHub archive", images.len());
        Ok(images)
    }
}

/// Check if two images are duplicates (identifier match OR title match)
pub fn is_duplicate(img1: &BingImage, img2: &BingImage) -> bool {
    // Try identifier match
    if let (Some(id1), Some(id2)) = (extract_identifier(&img1.url), extract_identifier(&img2.url)) {
        if id1 == id2 {
            return true;
        }
    }

    // Try title match (case-insensitive, trimmed)
    let title1 = img1.title.to_lowercase().trim().to_string();
    let title2 = img2.title.to_lowercase().trim().to_string();

    !title1.is_empty() && title1 == title2
}

/// Deduplicate images, preferring Bing API results over GitHub
pub fn deduplicate(bing_images: Vec<BingImage>, github_images: Vec<BingImage>) -> Vec<BingImage> {
    let mut result = bing_images;

    // Add GitHub images that aren't duplicates
    for github_img in github_images {
        let is_dup = result.iter().any(|bing_img| is_duplicate(bing_img, &github_img));
        if !is_dup {
            result.push(github_img);
        }
    }

    result
}

/// Main image source interface with dual-source fetching
pub struct ImageSource {
    bing_api: BingApiSource,
    github_archive: GitHubArchiveSource,
}

impl ImageSource {
    pub fn new(ehttp_cache: Option<Arc<crate::ehttp_cache::EhttpCache>>) -> Self {
        Self {
            bing_api: BingApiSource::new(ehttp_cache.clone()),
            github_archive: GitHubArchiveSource::new(ehttp_cache),
        }
    }

    /// Fetch images from both sources, merge and deduplicate
    pub fn fetch_images(&self, count: usize) -> Result<Vec<BingImage>> {
        // Fetch from Bing API (always fetch 8, the max)
        let bing_images = match self.bing_api.fetch(8) {
            Ok(imgs) => imgs,
            Err(e) => {
                log::warn!("Bing API failed: {}, continuing with GitHub only", e);
                Vec::new()
            }
        };

        // Fetch from GitHub archive
        let github_images = match self.github_archive.fetch() {
            Ok(imgs) => imgs,
            Err(e) => {
                log::warn!("GitHub archive failed: {}, continuing with Bing only", e);
                Vec::new()
            }
        };

        // Merge and deduplicate (Bing takes priority)
        let merged = deduplicate(bing_images, github_images);

        // Return requested count
        Ok(merged.into_iter().take(count).collect())
    }
}

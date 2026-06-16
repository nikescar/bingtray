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

/// Extract markdown content from HTML/JSON response (for GitHub blob URLs with ?plain=1)
/// Returns the original text if it's already markdown (not HTML/JSON)
fn extract_markdown_from_html(text: &str) -> std::borrow::Cow<str> {
    // Check if the content is HTML/JSON
    let trimmed = text.trim_start();
    if !trimmed.starts_with("<!DOCTYPE") && !trimmed.starts_with("<html") && !trimmed.starts_with("<HTML") {
        // Not HTML, return as-is (already markdown)
        return std::borrow::Cow::Borrowed(text);
    }

    log::info!("Detected HTML response, extracting markdown content");

    // GitHub's ?plain=1 response contains embedded JSON with rawLines
    // Look for: "rawLines":["line1","line2",...]
    if let Some(raw_lines_start) = text.find(r#""rawLines":["#) {
        let start_pos = raw_lines_start + r#""rawLines":["#.len();

        // Find the closing bracket - need to handle nested quotes carefully
        let mut extracted_lines = Vec::new();
        let mut current_line = String::new();
        let mut in_string = false;
        let mut escape_next = false;
        let remaining = &text[start_pos..];

        for ch in remaining.chars() {
            if escape_next {
                // Handle escape sequences
                match ch {
                    'n' => current_line.push('\n'),
                    't' => current_line.push('\t'),
                    'r' => current_line.push('\r'),
                    '"' => current_line.push('"'),
                    '\\' => current_line.push('\\'),
                    _ => {
                        current_line.push('\\');
                        current_line.push(ch);
                    }
                }
                escape_next = false;
                continue;
            }

            match ch {
                '\\' if in_string => {
                    escape_next = true;
                }
                '"' if in_string => {
                    // End of string
                    in_string = false;
                    if !current_line.is_empty() {
                        extracted_lines.push(current_line.clone());
                        current_line.clear();
                    }
                }
                '"' if !in_string => {
                    // Start of string
                    in_string = true;
                }
                ']' if !in_string => {
                    // End of array
                    break;
                }
                _ if in_string => {
                    current_line.push(ch);
                }
                _ => {
                    // Skip characters outside strings (commas, spaces, etc.)
                }
            }
        }

        if !extracted_lines.is_empty() {
            log::info!("Extracted {} markdown lines from JSON", extracted_lines.len());
            return std::borrow::Cow::Owned(extracted_lines.join("\n"));
        }
    }

    // Fallback: Look for lines in HTML that match the pattern
    let mut extracted_lines = Vec::new();
    for line in text.lines() {
        let trimmed_line = line.trim();

        if trimmed_line.contains(" | [") && trimmed_line.contains("](https://") {
            let potential_date = trimmed_line.split('|').next().unwrap_or("").trim();
            if potential_date.len() == 10 && potential_date.chars().nth(4) == Some('-') && potential_date.chars().nth(7) == Some('-') {
                let decoded = trimmed_line
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&amp;", "&")
                    .replace("&quot;", "\"")
                    .replace("&#39;", "'");
                extracted_lines.push(decoded);
                continue;
            }
        }

        if trimmed_line.starts_with("## Bing Wallpaper") {
            extracted_lines.push(trimmed_line.to_string());
        }
    }

    if !extracted_lines.is_empty() {
        log::info!("Extracted {} markdown lines from HTML", extracted_lines.len());
        std::borrow::Cow::Owned(extracted_lines.join("\n"))
    } else {
        log::warn!("Could not extract markdown from HTML, returning original text");
        std::borrow::Cow::Borrowed(text)
    }
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

/// Parse a markdown row from GitHub archive
/// Format: YYYY-MM-DD | [Title (© Copyright)](URL)
pub fn parse_markdown_row(row: &str) -> Option<BingImage> {
    // Skip empty lines and headers
    let line = row.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    // Parse format: "YYYY-MM-DD | [Title (© Copyright)](URL)"
    let (date_part, rest) = line.split_once('|')?;
    let date = date_part.trim();
    let rest = rest.trim();

    // Validate date format YYYY-MM-DD
    if date.len() != 10 || date.chars().nth(4) != Some('-') || date.chars().nth(7) != Some('-') {
        return None;
    }

    // Extract markdown link: [text](url)
    let link_start = rest.find('[')?;
    let link_end = rest.find("](")?;
    let url_end = rest.rfind(')')?;

    let content = &rest[link_start + 1..link_end];
    let url = &rest[link_end + 2..url_end];

    // Split content into title and copyright
    let (title, copyright) = if let Some(copyright_start) = content.find("(©") {
        let title = content[..copyright_start].trim();
        let copyright = content[copyright_start + 1..].trim_end_matches(')').trim();
        (title, Some(copyright.to_string()))
    } else {
        (content, None)
    };

    // Normalize URL: cn.bing.com -> www.bing.com
    let normalized_url = url.replace("cn.bing.com", "www.bing.com");

    // Generate copyright link
    let fullstartdate = date.replace('-', "") + "0000";
    let title_query = title.to_lowercase().replace(' ', "+");
    let startdate = &fullstartdate[..8]; // YYYYMMDD
    let copyright_link = format!(
        "https://www.bing.com/search?q={}&form=hpcapt&filters=HpDate%3A%22{}_0700%22",
        title_query, startdate
    );

    Some(BingImage {
        url: normalized_url,
        title: title.to_string(),
        copyright,
        copyright_link: Some(copyright_link),
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

        // Parse markdown - extract from HTML if needed
        let text = resp.text().context("Invalid UTF-8")?;
        let markdown_text = extract_markdown_from_html(&text);
        let images: Vec<BingImage> = markdown_text
            .lines()
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
    ///
    /// # Arguments
    /// * `count` - Number of new images to return
    /// * `existing_urls` - URLs already in database (to skip)
    pub fn fetch_images(&self, count: usize, existing_urls: &[String]) -> Result<Vec<BingImage>> {
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

        // Filter out already-existing URLs and return requested count
        let new_images: Vec<BingImage> = merged
            .into_iter()
            .filter(|img| !existing_urls.contains(&img.url))
            .take(count)
            .collect();

        log::info!("Returning {} new images (filtered out existing URLs)", new_images.len());
        Ok(new_images)
    }
}

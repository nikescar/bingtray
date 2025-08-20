use anyhow::{Context, Result};
use chrono::{NaiveDate, Duration};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "serde")]
use serde_json;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BingImage {
    pub url: String,
    pub title: String,
    pub copyright: Option<String>,
    pub copyrightlink: Option<String>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BingResponse {
    pub images: Vec<BingImage>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct HistoricalImage {
    pub fullstartdate: String,
    pub url: String,
    pub copyright: String,
    pub copyrightlink: String,
    pub title: String,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_async<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    use std::sync::OnceLock;
    use std::sync::mpsc;
    
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    
    let rt = RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime")
    });
    
    let (tx, rx) = mpsc::channel();
    rt.spawn(async move {
        let result = future.await;
        let _ = tx.send(result);
    });
    
    rx.recv().expect("Failed to receive result from async task")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_market_codes() -> Result<Vec<String>> {
    log::info!("get_market_codes: Fetching market codes from Microsoft docs");
    let url = "https://learn.microsoft.com/en-us/bing/search-apis/bing-web-search/reference/market-codes";
    
    let response = run_async(async move {
        reqwest::Client::new()
            .get(url)
            .timeout(std::time::Duration::from_secs(30))
            .header("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8")
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await
    });
    
    let response = match response {
        Ok(resp) => {
            log::info!("get_market_codes: HTTP request successful, status: {}", resp.status());
            resp
        },
        Err(e) => {
            log::error!("get_market_codes: HTTP request failed: {}", e);
            return Err(e.into());
        }
    };
    
    let html = run_async(async { response.text().await });
    let html = match html {
        Ok(text) => {
            log::info!("get_market_codes: Received {} bytes of HTML", text.len());
            text
        }
        Err(e) => {
            log::error!("get_market_codes: Failed to read response text: {}", e);
            return Err(e.into());
        }
    };
    
    let document = scraper::Html::parse_document(&html);
    let table_selector = scraper::Selector::parse("table").unwrap();
    let row_selector = scraper::Selector::parse("tr").unwrap();
    let cell_selector = scraper::Selector::parse("td").unwrap();
    
    let mut market_codes = Vec::new();
    
    for table in document.select(&table_selector) {
        for row in table.select(&row_selector).skip(1) { // Skip header row
            let cells: Vec<_> = row.select(&cell_selector).collect();
            if cells.len() >= 2 {
                if let Some(market_code) = cells.last() {
                    let code = market_code.text().collect::<String>().trim().to_string();
                    if !code.is_empty() && code.contains("-") {
                        market_codes.push(code);
                    }
                }
            }
        }
    }
    
    log::info!("get_market_codes: Parsed {} market codes from HTML", market_codes.len());
    if market_codes.is_empty() {
        log::warn!("get_market_codes: No market codes found, using fallback");
        market_codes = vec![
            "en-US".to_string(),
            "en-GB".to_string(),
            "de-DE".to_string(),
            "fr-FR".to_string(),
            "ja-JP".to_string(),
            "zh-CN".to_string(),
        ];
    }
    
    Ok(market_codes)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_bing_images(market_code: &str) -> Result<Vec<BingImage>> {
    let url = format!("https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=8&mkt={}", market_code);
    log::info!("URL: {}", url);
    
    if let Ok(result) = try_bing_api_url(&url, market_code, 1) {
        log::info!("SUCCESS: URL variant worked!");
        return Ok(result);
    }
    log::error!("FAILED: URL variant failed");
    
    Err(anyhow::anyhow!("All URL variants failed"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn try_bing_api_url(url: &str, market_code: &str, _attempt_num: usize) -> Result<Vec<BingImage>> {
    log::info!("=== NETWORK DIAGNOSTICS START ===");
    log::info!("Target URL: {}", url);
    log::info!("Market Code: {}", market_code);
    
    #[cfg(target_os = "android")]
    {
        log::info!("Platform: Android");
        log::info!("Checking network connectivity...");
        match std::net::TcpStream::connect_timeout(&"8.8.8.8:53".parse().unwrap(), std::time::Duration::from_secs(5)) {
            Ok(_) => log::info!("Basic internet connectivity: OK"),
            Err(e) => log::error!("Basic internet connectivity: FAILED - {}", e),
        }
        
        match std::net::ToSocketAddrs::to_socket_addrs(&"bing.com:443") {
            Ok(addrs) => {
                let addrs: Vec<_> = addrs.collect();
                log::info!("DNS resolution for bing.com: OK - {} addresses resolved", addrs.len());
                for addr in addrs.iter().take(3) {
                    log::info!("  Resolved address: {}", addr);
                }
            }
            Err(e) => log::error!("DNS resolution for bing.com: FAILED - {}", e),
        }
    }
    
    log::info!("=== NETWORK DIAGNOSTICS END ===");
    
    let max_retries = 3;
    let mut last_error = None;
    
    for attempt in 1..=max_retries {
        log::info!("Attempting to fetch Bing images (attempt {}/{}) for market: {}", attempt, max_retries, market_code);
        
        let url_owned = url.to_string();
        let result = run_async(async move {
            reqwest::Client::new()
                .get(&url_owned)
                .timeout(std::time::Duration::from_secs(30))
                .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64; rv:10.0) Gecko/20100101 Firefox/10.0")
                .header("Accept", "application/json, text/plain, */*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Cache-Control", "no-cache")
                .header("Referer", "https://www.bing.com/")
                .send()
                .await
        });
        
        match result {
            Ok(response) => {
                log::info!("HTTP response received, status: {}, content-length: {:?}", 
                          response.status(), response.headers().get("content-length"));
                
                let status = response.status();
                let text_result = run_async(async { response.text().await });
                match text_result {
                    Ok(text) => {
                        log::info!("Response text received, length: {} bytes", text.len());
                        if text.trim().is_empty() {
                            log::warn!("Empty response received, retrying...");
                            last_error = Some(anyhow::anyhow!("Empty response from server"));
                            continue;
                        }
                        
                        #[cfg(feature = "serde")]
                        match serde_json::from_str::<BingResponse>(&text) {
                            Ok(bing_response) => {
                                log::info!("Successfully parsed {} images from response", bing_response.images.len());
                                return Ok(bing_response.images);
                            }
                            Err(e) => {
                                log::error!("JSON parsing failed: {}", e);
                                log::error!("Full response content: {}", &text);
                                log::error!("Response status was: {}", status);
                                last_error = Some(e.into());
                                continue;
                            }
                        }
                        
                        #[cfg(not(feature = "serde"))]
                        {
                            log::error!("Serde feature not enabled - cannot parse JSON response");
                            last_error = Some(anyhow::anyhow!("Serde feature required for JSON parsing"));
                            continue;
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to read response text: {}", e);
                        last_error = Some(e.into());
                        
                        if attempt < max_retries {
                            std::thread::sleep(std::time::Duration::from_millis(1000 * attempt as u64));
                        }
                        continue;
                    }
                }
            }
            Err(e) => {
                log::error!("HTTP request failed (attempt {}): {}", attempt, e);
                log::error!("Error details: {:?}", e);
                
                #[cfg(target_os = "android")]
                {
                    let error_msg = format!("{}", e);
                    if error_msg.contains("unexpected end of file") {
                        log::error!("ANDROID ISSUE: Connection terminated prematurely - likely network security policy or DNS issue");
                    } else if error_msg.contains("connection refused") {
                        log::error!("ANDROID ISSUE: Connection refused - check firewall or network restrictions");
                    } else if error_msg.contains("timeout") {
                        log::error!("ANDROID ISSUE: Connection timeout - check network connectivity");
                    } else if error_msg.contains("certificate") || error_msg.contains("ssl") || error_msg.contains("tls") {
                        log::error!("ANDROID ISSUE: SSL/TLS certificate issue - check network security config");
                    } else {
                        log::error!("ANDROID ISSUE: Unknown network error - {}", error_msg);
                    }
                }
                
                last_error = Some(e.into());
                
                if attempt < max_retries {
                    std::thread::sleep(std::time::Duration::from_millis(1000 * attempt as u64));
                }
                continue;
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All {} attempts failed", max_retries)))
}

/// Download and parse historical data from GitHub repository
#[cfg(not(target_arch = "wasm32"))]
pub fn download_historical_data(_starting_index: usize) -> Result<Vec<HistoricalImage>> {
    let url = "https://raw.githubusercontent.com/v5tech/bing-wallpaper/refs/heads/main/bing-wallpaper.md";
    let response = run_async(async move {
        reqwest::Client::new()
            .get(url)
            .timeout(std::time::Duration::from_secs(30))
            .header("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")
            .send()
            .await
    })?;
    let content = run_async(async { response.text().await })?;
    
    let lines: Vec<&str> = content.lines().collect();
    let mut historical_images = Vec::new();
    
    for line in lines.iter() {
        if let Some(historical_image) = parse_historical_line(line)? {
            historical_images.push(historical_image);
        }
    }

    if historical_images.is_empty() {
        return Ok(Vec::new());
    }

    Ok(historical_images.into_iter().rev().take(8).collect())
}

/// Parse a single line from the historical data markdown
pub fn parse_historical_line(line: &str) -> Result<Option<HistoricalImage>> {
    if !line.contains(" | [") || !line.contains("](") {
        return Ok(None);
    }
    
    let parts: Vec<&str> = line.split(" | ").collect();
    if parts.len() != 2 {
        return Ok(None);
    }
    
    let date_str = parts[0].trim();
    let bracket_content = parts[1];
    
    if let Some(start) = bracket_content.find('[') {
        if let Some(end) = bracket_content.find("](") {
            let title_and_copyright = &bracket_content[start + 1..end];
            if let Some(url_start) = bracket_content.find("](") {
                if let Some(url_end) = bracket_content.rfind(')') {
                    let full_url = &bracket_content[url_start + 2..url_end];
                    
                    let (title, copyright) = if let Some(copyright_start) = title_and_copyright.rfind(" (") {
                        let title = title_and_copyright[..copyright_start].trim();
                        let copyright = title_and_copyright[copyright_start + 2..].trim_end_matches(')');
                        (title, copyright)
                    } else {
                        (title_and_copyright, "")
                    };
                    
                    let display_name = if let Some(id_part) = full_url.split("id=").nth(1) {
                        if let Some(name_part) = id_part.split('_').next() {
                            name_part.to_string()
                        } else {
                            "OHR.Unknown".to_string()
                        }
                    } else {
                        "OHR.Unknown".to_string()
                    };
                    
                    let imagecode = if let Some(id_part) = full_url.split("id=").nth(1) {
                        if let Some(code_part) = id_part.split('_').nth(1) {
                            if let Some(code) = code_part.split('_').next() {
                                code.to_string()
                            } else {
                                "EN-US0000000000".to_string()
                            }
                        } else {
                            "EN-US0000000000".to_string()
                        }
                    } else {
                        "EN-US0000000000".to_string()
                    };
                    
                    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                        .context("Failed to parse date")?;
                    
                    let startdate = date.format("%Y%m%d").to_string();
                    let fullstartdate = format!("{}0300", startdate);
                    let next_date = date + Duration::days(1);
                    let _enddate = next_date.format("%Y%m%d").to_string();
                    
                    let url = format!("/th?id={}_{}_1920x1080.jpg&pid=hp", display_name, imagecode);
                    let _urlbase = format!("/th?id={}", display_name);
                    
                    let title_query = title.to_lowercase().replace(' ', "+");
                    let copyrightlink = format!(
                        "https://www.bing.com/search?q={}&form=hpcapt&filters=HpDate%3A%22{}_0700%22",
                        title_query, startdate
                    );
                    
                    return Ok(Some(HistoricalImage {
                        fullstartdate,
                        url,
                        copyright: format!("{}", copyright),
                        copyrightlink,
                        title: title.to_string(),
                    }));
                }
            }
        }
    }
    
    Ok(None)
}

// WASM stubs
#[cfg(target_arch = "wasm32")]
pub fn get_market_codes() -> Result<Vec<String>> {
    Ok(Vec::new())
}

#[cfg(target_arch = "wasm32")]
pub fn get_bing_images(_market_code: &str) -> Result<Vec<BingImage>> {
    Ok(Vec::new())
}

#[cfg(target_arch = "wasm32")]
pub fn download_historical_data(_starting_index: usize) -> Result<Vec<HistoricalImage>> {
    Ok(Vec::new())
}
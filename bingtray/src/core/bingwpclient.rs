use std::sync::Arc;
use ureq;

use super::request::{RequestQueue, RequestContext};
use super::httpclient::HttpClient;

// Data structures for BingWP API
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BingImage {
    pub url: String,
    pub urlbase: String,
    pub copyright: String,
    pub copyrightlink: String,
    pub title: String,
    pub hsh: String,
    pub startdate: String,
    pub fullstartdate: String,
}

#[cfg(feature = "serde")]
#[derive(Debug, Deserialize, Serialize)]
pub struct BingResponse {
    pub images: Vec<BingImage>,
}

#[derive(Debug, Clone)]
pub struct HistoricalImage {
    pub fullstartdate: String,
    pub url: String,
    pub copyright: String,
    pub copyrightlink: String,
    pub title: String,
}

// BingWP Client using the HttpClient
#[derive(Clone)]
pub struct BingWPClient {
    http_client: HttpClient,
}

impl BingWPClient {
    pub fn new(queue: Arc<RequestQueue>) -> Self {
        let http_client = HttpClient::new(queue, "BingWPClient".to_string());
        Self { http_client }
    }

    // Helper method to execute a request and get the response
    fn execute_request(&self, request_context: RequestContext) -> Result<ureq::Response, Box<dyn std::error::Error>> {
        let mut request = match request_context.method.as_str() {
            "GET" => ureq::get(&request_context.url),
            "POST" => ureq::post(&request_context.url),
            "PUT" => ureq::put(&request_context.url),
            "DELETE" => ureq::delete(&request_context.url),
            _ => return Err("Unsupported HTTP method".into()),
        };

        // Add headers
        for (key, value) in &request_context.headers {
            request = request.set(&key, &value);
        }

        // Execute request
        let response = if let Some(body) = &request_context.body {
            request.send_string(body)?
        } else {
            request.call()?
        };

        Ok(response)
    }

    // Direct execution methods that bypass the queue for immediate results
    pub fn get_market_codes(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let url = "https://learn.microsoft.com/en-us/bing/search-apis/bing-web-search/reference/market-codes";
        
        // Build the request context
        let request_context = RequestContext::new(
            url.to_string(),
            "GET".to_string()
        ).with_header("User-Agent".to_string(), "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0".to_string())
        .with_header("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8".to_string())
        .with_header("Accept-Language".to_string(), "en-US,en;q=0.9".to_string());
        
        // Execute request directly
        let resp = self.execute_request(request_context)?;
        let html = resp.into_string()?;
        
        let market_codes = Self::parse_market_codes_from_html(&html)?;
        Ok(market_codes)
    }

    pub fn get_bing_images(&self, market_code: &str) -> Result<Vec<BingImage>, Box<dyn std::error::Error>> {
        let url = format!("https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=8&mkt={}", market_code);
        
        // Build the request context
        let request_context = RequestContext::new(
            url,
            "GET".to_string()
        ).with_header("User-Agent".to_string(), "Mozilla/5.0 (X11; Linux x86_64; rv:10.0) Gecko/20100101 Firefox/10.0".to_string())
        .with_header("Accept".to_string(), "application/json, text/plain, */*".to_string())
        .with_header("Accept-Language".to_string(), "en-US,en;q=0.9".to_string())
        .with_header("Cache-Control".to_string(), "no-cache".to_string())
        .with_header("Referer".to_string(), "https://www.bing.com/".to_string());
        
        // Execute request directly
        let resp = self.execute_request(request_context)?;
        let json_str = resp.into_string()?;
        
        #[cfg(feature = "serde")]
        {
            let bing_response: BingResponse = serde_json::from_str(&json_str)?;
            Ok(bing_response.images)
        }
        
        #[cfg(not(feature = "serde"))]
        {
            Err("Serde feature required for JSON parsing".into())
        }
    }

    pub fn download_historical_data(&self) -> Result<Vec<HistoricalImage>, Box<dyn std::error::Error>> {
        let url = "https://raw.githubusercontent.com/v5tech/bing-wallpaper/refs/heads/main/bing-wallpaper.md";
        
        // Build the request context
        let request_context = RequestContext::new(
            url.to_string(),
            "GET".to_string()
        ).with_header("User-Agent".to_string(), "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0".to_string());
        
        // Execute request directly
        let resp = self.execute_request(request_context)?;
        let content = resp.into_string()?;
        
        let historical_images = Self::parse_historical_data(&content)?;
        Ok(historical_images)
    }

    pub fn download_image_bytes(&self, url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let full_url = if url.starts_with("http") {
            url.to_string()
        } else {
            format!("https://bing.com{}", url)
        };
        
        // Build the request context
        let request_context = RequestContext::new(
            full_url,
            "GET".to_string()
        ).with_header("User-Agent".to_string(), "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0".to_string())
        .with_header("Accept".to_string(), "image/webp,image/apng,image/*,*/*;q=0.8".to_string())
        .with_header("Referer".to_string(), "https://www.bing.com/".to_string());
        
        // Execute request directly
        let resp = self.execute_request(request_context)?;
        
        let mut bytes = Vec::new();
        std::io::copy(&mut resp.into_reader(), &mut bytes)?;
        
        Ok(bytes)
    }

    pub fn download_thumbnail_bytes(&self, url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let base_url = if url.starts_with("http") {
            url.to_string()
        } else {
            format!("https://bing.com{}", url)
        };
        
        let thumbnail_url = if base_url.contains('?') {
            format!("{}&w=320&h=240", base_url)
        } else {
            format!("{}?w=320&h=240", base_url)
        };
        
        self.download_image_bytes(&thumbnail_url)
    }

    fn parse_market_codes_from_html(html: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut market_codes = Vec::new();
        
        // Simple HTML parsing for market codes
        // Look for pattern like "en-US", "de-DE", etc. in table cells
        let lines: Vec<&str> = html.lines().collect();
        for line in lines {
            if line.contains("<td>") && line.contains("-") {
                // Extract market code from table cell
                if let Some(start) = line.find(">") {
                    if let Some(end) = line[start+1..].find("<") {
                        let code = &line[start+1..start+1+end];
                        if code.len() == 5 && code.chars().nth(2) == Some('-') {
                            market_codes.push(code.to_string());
                        }
                    }
                }
            }
        }
        
        if market_codes.is_empty() {
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

    fn parse_historical_data(content: &str) -> Result<Vec<HistoricalImage>, Box<dyn std::error::Error>> {
        let mut historical_images = Vec::new();
        
        for line in content.lines() {
            if let Some(image) = Self::parse_historical_line(line)? {
                historical_images.push(image);
            }
        }
        
        Ok(historical_images)
    }

    fn parse_historical_line(line: &str) -> Result<Option<HistoricalImage>, Box<dyn std::error::Error>> {
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
                        
                        let date_parts: Vec<&str> = date_str.split('-').collect();
                        if date_parts.len() != 3 {
                            return Err(format!("Invalid date format: {}", date_str).into());
                        }
                        
                        let startdate = format!("{}{:0>2}{:0>2}", date_parts[0], date_parts[1], date_parts[2]);
                        let fullstartdate = format!("{}0300", startdate);
                        
                        let url = format!("/th?id={}_{}_1920x1080.jpg&pid=hp", display_name, imagecode);
                        
                        let title_query = title.to_lowercase().replace(' ', "+");
                        let copyrightlink = format!(
                            "https://www.bing.com/search?q={}&form=hpcapt&filters=HpDate%3A%22{}_0700%22",
                            title_query, startdate
                        );
                        
                        return Ok(Some(HistoricalImage {
                            fullstartdate,
                            url,
                            copyright: copyright.to_string(),
                            copyrightlink,
                            title: title.to_string(),
                        }));
                    }
                }
            }
        }
        
        Ok(None)
    }
}

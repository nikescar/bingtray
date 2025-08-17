use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response, Headers};
use crate::{BingImage, HistoricalImage};
#[cfg(feature = "serde")]
use crate::BingResponse;

pub struct HttpClient;

impl HttpClient {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_market_codes() -> Result<Vec<String>, JsValue> {
        let url = "https://learn.microsoft.com/en-us/bing/search-apis/bing-web-search/reference/market-codes";
        
        let opts = RequestInit::new();
        opts.set_method("GET");
        opts.set_mode(RequestMode::Cors);
        
        let headers = Headers::new()?;
        headers.set("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")?;
        headers.set("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8")?;
        headers.set("Accept-Language", "en-US,en;q=0.9")?;
        opts.set_headers(&headers);

        let request = Request::new_with_str_and_init(url, &opts)?;
        let window = web_sys::window().unwrap();
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
        let resp: Response = resp_value.dyn_into()?;
        
        let text = JsFuture::from(resp.text()?).await?;
        let html = text.as_string().unwrap_or_default();
        
        let market_codes = Self::parse_market_codes_from_html(&html)?;
        Ok(market_codes)
    }

    pub async fn get_bing_images(market_code: &str) -> Result<Vec<BingImage>, JsValue> {
        let url = format!("https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=8&mkt={}", market_code);
        
        let opts = RequestInit::new();
        opts.set_method("GET");
        opts.set_mode(RequestMode::Cors);
        
        let headers = Headers::new()?;
        headers.set("User-Agent", "Mozilla/5.0 (X11; Linux x86_64; rv:10.0) Gecko/20100101 Firefox/10.0")?;
        headers.set("Accept", "application/json, text/plain, */*")?;
        headers.set("Accept-Language", "en-US,en;q=0.9")?;
        headers.set("Cache-Control", "no-cache")?;
        headers.set("Referer", "https://www.bing.com/")?;
        opts.set_headers(&headers);

        let request = Request::new_with_str_and_init(&url, &opts)?;
        let window = web_sys::window().unwrap();
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
        let resp: Response = resp_value.dyn_into()?;
        
        let text = JsFuture::from(resp.text()?).await?;
        let _json_str = text.as_string().unwrap_or_default();
        
        #[cfg(feature = "serde")]
        {
            let bing_response: BingResponse = serde_json::from_str(&_json_str)
                .map_err(|e| JsValue::from_str(&format!("JSON parse error: {}", e)))?;
            Ok(bing_response.images)
        }
        
        #[cfg(not(feature = "serde"))]
        {
            Err(JsValue::from_str("Serde feature required for JSON parsing"))
        }
    }

    pub async fn download_historical_data() -> Result<Vec<HistoricalImage>, JsValue> {
        let url = "https://raw.githubusercontent.com/v5tech/bing-wallpaper/refs/heads/main/bing-wallpaper.md";
        
        let opts = RequestInit::new();
        opts.set_method("GET");
        opts.set_mode(RequestMode::Cors);
        
        let headers = Headers::new()?;
        headers.set("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")?;
        opts.set_headers(&headers);

        let request = Request::new_with_str_and_init(url, &opts)?;
        let window = web_sys::window().unwrap();
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
        let resp: Response = resp_value.dyn_into()?;
        
        let text = JsFuture::from(resp.text()?).await?;
        let content = text.as_string().unwrap_or_default();
        
        let historical_images = Self::parse_historical_data(&content)?;
        Ok(historical_images)
    }

    pub async fn download_image_bytes(url: &str) -> Result<Vec<u8>, JsValue> {
        let full_url = if url.starts_with("http") {
            url.to_string()
        } else {
            format!("https://bing.com{}", url)
        };
        
        let opts = RequestInit::new();
        opts.set_method("GET");
        opts.set_mode(RequestMode::Cors);
        
        let headers = Headers::new()?;
        headers.set("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")?;
        headers.set("Accept", "image/webp,image/apng,image/*,*/*;q=0.8")?;
        headers.set("Referer", "https://www.bing.com/")?;
        opts.set_headers(&headers);

        let request = Request::new_with_str_and_init(&full_url, &opts)?;
        let window = web_sys::window().unwrap();
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
        let resp: Response = resp_value.dyn_into()?;
        
        let array_buffer = JsFuture::from(resp.array_buffer()?).await?;
        let uint8_array = js_sys::Uint8Array::new(&array_buffer);
        let mut bytes = vec![0; uint8_array.length() as usize];
        uint8_array.copy_to(&mut bytes);
        
        Ok(bytes)
    }

    pub async fn download_thumbnail_bytes(url: &str) -> Result<Vec<u8>, JsValue> {
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
        
        Self::download_image_bytes(&thumbnail_url).await
    }

    fn parse_market_codes_from_html(html: &str) -> Result<Vec<String>, JsValue> {
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

    fn parse_historical_data(content: &str) -> Result<Vec<HistoricalImage>, JsValue> {
        let mut historical_images = Vec::new();
        
        for line in content.lines() {
            if let Some(image) = Self::parse_historical_line(line)? {
                historical_images.push(image);
            }
        }
        
        Ok(historical_images)
    }

    fn parse_historical_line(line: &str) -> Result<Option<HistoricalImage>, JsValue> {
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
                        
                        use chrono::NaiveDate;
                        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                            .map_err(|e| JsValue::from_str(&format!("Date parse error: {}", e)))?;
                        
                        let startdate = date.format("%Y%m%d").to_string();
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
}
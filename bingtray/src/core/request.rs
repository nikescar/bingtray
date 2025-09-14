use std::collections::{VecDeque, HashMap};
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;
use web_sys::{Request as WebRequest, RequestInit, RequestMode, Response, Headers};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

// Singleton pattern for shared request queue
pub struct RequestQueue {
    queue: Arc<Mutex<VecDeque<RequestContext>>>,
    semaphore: Arc<Semaphore>,
}

impl RequestQueue {
    // Singleton instance
    fn instance() -> &'static Arc<RequestQueue> {
        static INSTANCE: OnceLock<Arc<RequestQueue>> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            Arc::new(RequestQueue {
                queue: Arc::new(Mutex::new(VecDeque::new())),
                semaphore: Arc::new(Semaphore::new(10)), // Default max concurrent
            })
        })
    }

    pub fn global() -> Arc<RequestQueue> {
        Arc::clone(Self::instance())
    }

    pub fn enqueue(&self, request: RequestContext) {
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(request);
    }

    pub fn dequeue(&self) -> Option<RequestContext> {
        let mut queue = self.queue.lock().unwrap();
        queue.pop_front()
    }

    pub fn len(&self) -> usize {
        let queue = self.queue.lock().unwrap();
        queue.len()
    }

    pub fn semaphore(&self) -> &Arc<Semaphore> {
        &self.semaphore
    }
}

// Simplified request context without flyweight
#[derive(Debug)]
pub struct RequestContext {
    url: String,
    method: String,
    headers: HashMap<String, String>,
    body: Option<String>,
}

impl RequestContext {
    pub fn new(url: String, method: String) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("User-Agent".to_string(), "BingTray/1.0".to_string());
        
        Self {
            url,
            method,
            headers,
            body: None,
        }
    }

    pub fn with_header(mut self, key: String, value: String) -> Self {
        self.headers.insert(key, value);
        self
    }

    pub fn with_body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }
}

// Client that shares the queue
#[derive(Clone)]
pub struct HttpClient {
    queue: Arc<RequestQueue>,
    client_id: String,
}

impl HttpClient {
    pub fn new(queue: Arc<RequestQueue>, client_id: String) -> Self {
        Self { queue, client_id }
    }

    pub fn get(&self, url: &str) -> RequestBuilder {
        let template = self.queue.get_template("GET");
        RequestBuilder::new(url.to_string(), template, Arc::clone(&self.queue))
    }

    pub fn post(&self, url: &str) -> RequestBuilder {
        let template = self.queue.get_template("POST");
        RequestBuilder::new(url.to_string(), template, Arc::clone(&self.queue))
    }

    pub fn process_next(&self) -> Option<RequestContext> {
        self.queue.dequeue()
    }

    pub fn queue_size(&self) -> usize {
        self.queue.len()
    }
}

// Builder pattern for creating requests
pub struct RequestBuilder {
    context: RequestContext,
    queue: Arc<RequestQueue>,
}

impl RequestBuilder {
    pub fn new(url: String, template: Arc<RequestTemplate>, queue: Arc<RequestQueue>) -> Self {
        Self {
            context: RequestContext::new(url, template),
            queue,
        }
    }

    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.context = self.context.with_header(key.to_string(), value.to_string());
        self
    }

    pub fn body(mut self, body: &str) -> Self {
        self.context = self.context.with_body(body.to_string());
        self
    }

    pub fn send(self) {
        self.queue.enqueue(self.context);
    }
}

// Data structures for BingWP API
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, OnceLock};

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

// BingWP Client using the flyweight pattern
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
    async fn execute_request(&self, request_context: RequestContext) -> Result<Response, JsValue> {
        // Combine template headers with custom headers
        let mut final_headers = request_context.template.default_headers.clone();
        for (key, value) in &request_context.custom_headers {
            final_headers.insert(key.clone(), value.clone());
        }

        let opts = RequestInit::new();
        opts.set_method(&request_context.template.method);
        opts.set_mode(RequestMode::Cors);
        
        let headers = Headers::new()?;
        for (key, value) in &final_headers {
            headers.set(key, value)?;
        }
        opts.set_headers(&headers);

        if let Some(body) = &request_context.body {
            opts.set_body(&JsValue::from_str(body));
        }

        let request = WebRequest::new_with_str_and_init(&request_context.url, &opts)?;
        let window = web_sys::window().unwrap();
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
        let resp: Response = resp_value.dyn_into()?;
        
        Ok(resp)
    }

    // Direct execution methods that bypass the queue for immediate results
    pub async fn get_market_codes(&self) -> Result<Vec<String>, JsValue> {
        let url = "https://learn.microsoft.com/en-us/bing/search-apis/bing-web-search/reference/market-codes";
        
        // Create request using HttpClient builder pattern
        let request_builder = self.http_client.get(url)
            .header("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8")
            .header("Accept-Language", "en-US,en;q=0.9");
        
        // Build the request context
        let request_context = RequestContext::new(
            url.to_string(),
            self.http_client.queue.get_template("GET")
        ).with_header("User-Agent".to_string(), "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0".to_string())
        .with_header("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8".to_string())
        .with_header("Accept-Language".to_string(), "en-US,en;q=0.9".to_string());
        
        // Execute request directly
        let resp = self.execute_request(request_context).await?;
        
        let text = JsFuture::from(resp.text()?).await?;
        let html = text.as_string().unwrap_or_default();
        
        let market_codes = Self::parse_market_codes_from_html(&html)?;
        Ok(market_codes)
    }

    pub async fn get_bing_images(&self, market_code: &str) -> Result<Vec<BingImage>, JsValue> {
        let url = format!("https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=8&mkt={}", market_code);
        
        // Build the request context using HttpClient's flyweight pattern
        let request_context = RequestContext::new(
            url,
            self.http_client.queue.get_template("GET")
        ).with_header("User-Agent".to_string(), "Mozilla/5.0 (X11; Linux x86_64; rv:10.0) Gecko/20100101 Firefox/10.0".to_string())
        .with_header("Accept".to_string(), "application/json, text/plain, */*".to_string())
        .with_header("Accept-Language".to_string(), "en-US,en;q=0.9".to_string())
        .with_header("Cache-Control".to_string(), "no-cache".to_string())
        .with_header("Referer".to_string(), "https://www.bing.com/".to_string());
        
        // Execute request directly
        let resp = self.execute_request(request_context).await?;
        
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

    pub async fn download_historical_data(&self) -> Result<Vec<HistoricalImage>, JsValue> {
        let url = "https://raw.githubusercontent.com/v5tech/bing-wallpaper/refs/heads/main/bing-wallpaper.md";
        
        // Build the request context using HttpClient's flyweight pattern
        let request_context = RequestContext::new(
            url.to_string(),
            self.http_client.queue.get_template("GET")
        ).with_header("User-Agent".to_string(), "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0".to_string());
        
        // Execute request directly
        let resp = self.execute_request(request_context).await?;
        
        let text = JsFuture::from(resp.text()?).await?;
        let content = text.as_string().unwrap_or_default();
        
        let historical_images = Self::parse_historical_data(&content)?;
        Ok(historical_images)
    }

    pub async fn download_image_bytes(&self, url: &str) -> Result<Vec<u8>, JsValue> {
        let full_url = if url.starts_with("http") {
            url.to_string()
        } else {
            format!("https://bing.com{}", url)
        };
        
        // Build the request context using HttpClient's flyweight pattern
        let request_context = RequestContext::new(
            full_url,
            self.http_client.queue.get_template("GET")
        ).with_header("User-Agent".to_string(), "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0".to_string())
        .with_header("Accept".to_string(), "image/webp,image/apng,image/*,*/*;q=0.8".to_string())
        .with_header("Referer".to_string(), "https://www.bing.com/".to_string());
        
        // Execute request directly
        let resp = self.execute_request(request_context).await?;
        
        let array_buffer = JsFuture::from(resp.array_buffer()?).await?;
        let uint8_array = js_sys::Uint8Array::new(&array_buffer);
        let mut bytes = vec![0; uint8_array.length() as usize];
        uint8_array.copy_to(&mut bytes);
        
        Ok(bytes)
    }

    pub async fn download_thumbnail_bytes(&self, url: &str) -> Result<Vec<u8>, JsValue> {
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
        
        self.download_image_bytes(&thumbnail_url).await
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
                        
                        // For WASM compatibility, we'll use a simple date parser
                        // instead of chrono since chrono may not work well in WASM
                        let date_parts: Vec<&str> = date_str.split('-').collect();
                        if date_parts.len() != 3 {
                            return Err(JsValue::from_str(&format!("Invalid date format: {}", date_str)));
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

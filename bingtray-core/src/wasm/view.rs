use wasm_bindgen::prelude::*;
use std::collections::HashSet;
use crate::{BingImage, HistoricalImage};
use super::db::SqliteDb;
use super::request::HttpClient;

#[wasm_bindgen]
pub struct WasmBingtrayApp {
    db: Option<SqliteDb>,
    carousel_images: Vec<BingImage>,
    market_codes: Vec<String>,
    market_code_index: usize,
    infinite_scroll_page_index: usize,
    showing_historical: bool,
    loading_more: bool,
    seen_image_names: HashSet<String>,
    historical_data_exhausted: bool,
    current_page: usize,
    total_pages: usize,
    initialization_started: bool,
    initialization_completed: bool,
}

impl Default for WasmBingtrayApp {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl WasmBingtrayApp {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WasmBingtrayApp {
        console_log!("Initializing WasmBingtrayApp");
        
        // Create a synchronous database instance
        let db = super::db::Posts::new();
        let sqlite_db = super::db::SqliteDb::new_sync(db);
        
        let app = WasmBingtrayApp {
            db: Some(sqlite_db),
            carousel_images: Vec::new(),
            market_codes: vec!["en-US".to_string()],
            market_code_index: 0,
            infinite_scroll_page_index: 0,
            showing_historical: false,
            loading_more: false,
            seen_image_names: HashSet::new(),
            historical_data_exhausted: false,
            current_page: 0,
            total_pages: 0,
            initialization_started: true,
            initialization_completed: true,
        };
        
        console_log!("WasmBingtrayApp initialized with database");
        app
    }

    #[wasm_bindgen]
    pub async fn init(&mut self) -> Result<(), JsValue> {
        console_log!("[WasmBingtrayApp] Starting initialization...");
        console_log!("[WasmBingtrayApp] Initializing database and loading market codes");
        
        // Initialize SQLite database
        console_log!("[WasmBingtrayApp] Creating SQLite database instance...");
        let db = match SqliteDb::new().await {
            Ok(database) => {
                console_log!("[WasmBingtrayApp] ✓ SQLite database created successfully");
                database
            },
            Err(e) => {
                console_log!("[WasmBingtrayApp] ✗ ERROR: Failed to create SQLite database: {:?}", e);
                return Err(e);
            }
        };
        
        console_log!("[WasmBingtrayApp] Initializing database tables...");
        match db.init_all_tables() {
            Ok(()) => {
                console_log!("[WasmBingtrayApp] ✓ All database tables initialized successfully");
            },
            Err(e) => {
                console_log!("[WasmBingtrayApp] ✗ ERROR: Failed to initialize database tables: {:?}", e);
                return Err(e);
            }
        }
        
        // Load market codes from database or fetch from web
        let market_codes = match db.load_market_codes() {
            Ok(_codes) => {
                // For now, just use default codes since we simplified the database
                console_log!("Database available but using default codes");
                vec![]
            },
            _ => {
                console_log!("Fetching market codes from web");
                match HttpClient::get_market_codes().await {
                    Ok(codes) => {
                        console_log!("Fetched {} market codes from web", codes.len());
                        // Skip saving to database for now
                        let _ = db.save_market_codes(JsValue::NULL);
                        codes
                    },
                    Err(_) => vec![
                        "en-US".to_string(),
                        "en-GB".to_string(),
                        "de-DE".to_string(),
                        "fr-FR".to_string(),
                        "ja-JP".to_string(),
                        "zh-CN".to_string(),
                    ],
                }
            }
        };
        
        self.market_codes = market_codes;
        self.db = Some(db);
        
        Ok(())
    }

    #[wasm_bindgen]
    pub async fn load_images(&mut self) -> Result<(), JsValue> {
        loop {
            if self.market_code_index >= self.market_codes.len() {
                console_log!("All market codes exhausted, switching to historical data");
                return self.load_historical_images().await;
            }
            
            let market_code = &self.market_codes[self.market_code_index];
            console_log!("Loading images for market code: {}", market_code);
            
            match HttpClient::get_bing_images(market_code).await {
                Ok(images) => {
                    console_log!("Loaded {} images", images.len());
                    
                    // Filter out already seen images
                    let mut new_images = Vec::new();
                    for image in images {
                        let display_name = Self::extract_display_name(&image.url);
                        if !self.seen_image_names.contains(&display_name) {
                            self.seen_image_names.insert(display_name.clone());
                            new_images.push(image);
                        }
                    }
                    
                    if new_images.is_empty() {
                        console_log!("No new images found for market {}, trying next", market_code);
                        self.market_code_index += 1;
                        continue; // Use continue instead of recursive call
                    }
                    
                    self.carousel_images.extend(new_images);
                    self.market_code_index += 1;
                    return Ok(());
                },
                Err(e) => {
                    console_log!("Failed to load images for market {}: {:?}", market_code, e);
                    self.market_code_index += 1;
                    continue; // Use continue instead of recursive call
                }
            }
        }
    }

    #[wasm_bindgen]
    pub async fn load_historical_images(&mut self) -> Result<(), JsValue> {
        if self.historical_data_exhausted {
            console_log!("Historical data exhausted");
            return Ok(());
        }
        
        self.showing_historical = true;
        
        if let Some(ref db) = self.db {
            let count = db.get_historical_metadata_count()?;
            if count == 0 {
                console_log!("No historical data in database, downloading from web");
                match HttpClient::download_historical_data().await {
                    Ok(historical_images) => {
                        console_log!("Downloaded {} historical images", historical_images.len());
                        
                        // Save to database
                        for image in &historical_images {
                            #[cfg(feature = "serde")]
                            {
                                let json = serde_json::to_string(image)
                                    .map_err(|e| JsValue::from_str(&format!("JSON serialize error: {}", e)))?;
                                let _ = db.save_historical_metadata(&json);
                            }
                            #[cfg(not(feature = "serde"))]
                            {
                                // Save basic string representation when serde is not available
                                let json_like = format!("{{\"title\":\"{}\",\"url\":\"{}\"}}", image.title, image.url);
                                let _ = db.save_historical_metadata(&json_like);
                            }
                        }
                        
                        // Convert to BingImage format for carousel
                        let bing_images = Self::historical_to_bing_images(&historical_images);
                        self.carousel_images.extend(bing_images);
                    },
                    Err(e) => {
                        console_log!("Failed to download historical data: {:?}", e);
                        self.historical_data_exhausted = true;
                    }
                }
            } else {
                console_log!("Loading historical data from database");
                self.total_pages = db.get_total_pages()?;
                
                if self.current_page < self.total_pages {
                    let json_list = db.get_historical_metadata_page(self.current_page)?;
                    let historical_images = Vec::new();
                    
                    for _json in json_list {
                        #[cfg(feature = "serde")]
                        {
                            if let Ok(image) = serde_json::from_str::<HistoricalImage>(&_json) {
                                historical_images.push(image);
                            }
                        }
                        #[cfg(not(feature = "serde"))]
                        {
                            // Basic parsing for minimal JSON-like format when serde not available
                            // This is a fallback - ideally serde should be enabled
                            console_log!("Serde feature not enabled, cannot parse historical data");
                        }
                    }
                    
                    let bing_images = Self::historical_to_bing_images(&historical_images);
                    self.carousel_images.extend(bing_images);
                    self.current_page += 1;
                } else {
                    console_log!("All historical pages loaded");
                    self.historical_data_exhausted = true;
                }
            }
        }
        
        Ok(())
    }

    #[wasm_bindgen]
    pub async fn add_to_blacklist(&self, image_title: &str) -> Result<(), JsValue> {
        if let Some(ref db) = self.db {
            console_log!("Adding {} to blacklist", image_title);
            db.add_to_blacklist(image_title)?;
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub async fn is_blacklisted(&self, image_title: &str) -> Result<bool, JsValue> {
        if let Some(ref db) = self.db {
            db.is_blacklisted(image_title)
        } else {
            Ok(false)
        }
    }

    #[wasm_bindgen]
    pub async fn save_image_metadata(&self, title: &str, copyright: &str, url: &str) -> Result<(), JsValue> {
        if let Some(ref db) = self.db {
            console_log!("Saving metadata for {}", title);
            db.save_metadata(title, copyright, url)?;
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub async fn get_image_metadata(&self, title: &str) -> Result<JsValue, JsValue> {
        if let Some(ref db) = self.db {
            // db.get_metadata now returns JsValue directly
            db.get_metadata(title)
        } else {
            Ok(JsValue::NULL)
        }
    }

    #[wasm_bindgen]
    pub fn get_carousel_images_count(&self) -> usize {
        self.carousel_images.len()
    }

    #[wasm_bindgen]
    pub fn get_carousel_image(&self, index: usize) -> Result<JsValue, JsValue> {
        if let Some(image) = self.carousel_images.get(index) {
            let obj = js_sys::Object::new();
            js_sys::Reflect::set(&obj, &"url".into(), &image.url.clone().into())?;
            js_sys::Reflect::set(&obj, &"title".into(), &image.title.clone().into())?;
            js_sys::Reflect::set(&obj, &"copyright".into(), &image.copyright.as_ref().unwrap_or(&String::new()).clone().into())?;
            js_sys::Reflect::set(&obj, &"copyrightlink".into(), &image.copyrightlink.as_ref().unwrap_or(&String::new()).clone().into())?;
            Ok(obj.into())
        } else {
            Ok(JsValue::NULL)
        }
    }

    #[wasm_bindgen]
    pub fn is_showing_historical(&self) -> bool {
        self.showing_historical
    }

    #[wasm_bindgen]
    pub fn get_current_page(&self) -> usize {
        self.current_page
    }

    #[wasm_bindgen]
    pub fn get_total_pages(&self) -> usize {
        self.total_pages
    }

    fn extract_display_name(url: &str) -> String {
        url.split("th?id=")
            .nth(1)
            .and_then(|s| s.split('_').next())
            .unwrap_or("Unknown")
            .to_string()
    }

    fn historical_to_bing_images(historical_images: &[HistoricalImage]) -> Vec<BingImage> {
        historical_images.iter().map(|hist| BingImage {
            url: hist.url.clone(),
            title: hist.title.clone(),
            copyright: Some(hist.copyright.clone()),
            copyrightlink: Some(hist.copyrightlink.clone()),
        }).collect()
    }
}

macro_rules! console_log {
    ($($t:tt)*) => (web_sys::console::log_1(&format_args!($($t)*).to_string().into()))
}

pub(crate) use console_log;

// Add eframe::App implementation for web compatibility
#[cfg(target_arch = "wasm32")]
impl eframe::App for WasmBingtrayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Only log initialization once and start async init
        if !self.initialization_started {
            console_log!("[WasmBingtrayApp] Starting initialization...");
            self.initialization_started = true;
            
            // Start async initialization
            let ctx_clone = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                console_log!("[WasmBingtrayApp] Async initialization started...");
                // Note: We can't modify self from here since we don't have access to it
                // We'll need to create a different approach
                ctx_clone.request_repaint();
            });
        }
        
        // Display directly in the central panel
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🌐 Bingtray WASM");
            ui.separator();
            
            // Status display
            ui.horizontal(|ui| {
                ui.label("Status:");
                ui.colored_label(egui::Color32::GREEN, "WASM version running");
            });
            
            ui.separator();
            
            // Simple test content
            ui.label("This is the WASM version of Bingtray running in the browser.");
            ui.label("The full WASM implementation with SQLite and HTTP requests is available.");
            
            ui.separator();
            
            // Database status
            ui.horizontal(|ui| {
                ui.label("Database:");
                if let Some(ref db) = self.db {
                    let posts = db.list_posts();
                    console_log!("posts: {:?}", posts);
                    ui.colored_label(egui::Color32::GREEN, &format!("✓ Connected ({} posts)", posts.len()));
                } else {
                    console_log!("Database not initialized yet");
                    ui.colored_label(egui::Color32::RED, "✗ Not Connected");
                }
            });
            
            ui.separator();
            
            // if ui.button("🗃️ Initialize Database").clicked() {
            //     console_log!("🗃️ Initializing app database...");
                
            //     // Set initialization started flag and trigger async init
            //     self.initialization_started = true;
                
            //     let ctx_clone = ctx.clone();
            //     wasm_bindgen_futures::spawn_local(async move {
            //         console_log!("Starting database initialization...");
            //         match super::db::SqliteDb::new().await {
            //             Ok(_db) => {
            //                 console_log!("✓ Database initialized successfully!");
            //                 // Note: We can't set self.db from here due to Rust ownership rules
            //                 console_log!("Database created but can't be stored in app state from async context");
            //             },
            //             Err(e) => {
            //                 console_log!("✗ Failed to initialize database: {:?}", e);
            //             }
            //         }
            //         ctx_clone.request_repaint();
            //     });
            // }
            
            // if ui.button("🧪 Test Embedded SQLite").clicked() {
            //     console_log!("🧪 Testing embedded SQLite WASM...");
                
            //     // Test the embedded SQLite loading (creates temporary instance)
            //     let ctx_clone = ctx.clone();
            //     wasm_bindgen_futures::spawn_local(async move {
            //         console_log!("Testing embedded SQLite instantiation...");
                    
            //         match super::db::SqliteDb::new().await {
            //             Ok(_sqlite) => {
            //                 console_log!("✓ SQLite database test successful! (temporary instance)");
            //             },
            //             Err(e) => {
            //                 console_log!("✗ Failed to load embedded SQLite WASM: {:?}", e);
            //             }
            //         }
                    
            //         ctx_clone.request_repaint();
            //     });
            // }
            
            if ui.button("📥 Load Images").clicked() && self.db.is_some() {
                web_sys::console::log_1(&"Manual image loading requested".into());
                // Note: We can't call async methods directly from UI buttons  
                // The user should call load_images() from JavaScript
            }

            // show list of tables in the sqlite database
            if let Some(ref db) = self.db {
                if let Ok(tables) = db.list_tables() {
                    ui.label("SQLite Tables:");
                    for table in tables {
                        ui.label(format!("- {}", table));
                    }
                } else {
                    ui.label("Failed to list SQLite tables");
                }
            } else {
                ui.label("SQLite database not initialized");
            }

            // show each tables description for all tables in the database
            if let Some(ref db) = self.db {
                if let Ok(descriptions) = db.describe_tables() {
                    ui.label("SQLite Table Descriptions:");
                    for desc_obj in descriptions {
                        if let Ok(table) = js_sys::Reflect::get(&desc_obj, &"table".into()) {
                            if let Ok(description) = js_sys::Reflect::get(&desc_obj, &"description".into()) {
                                ui.label(format!("{}: {}", 
                                    table.as_string().unwrap_or_default(),
                                    description.as_string().unwrap_or_default()
                                ));
                            }
                        }
                    }
                } else {
                    ui.label("Failed to describe SQLite tables");
                }
            } else {
                ui.label("SQLite database not initialized");
            }
        });
    }
}

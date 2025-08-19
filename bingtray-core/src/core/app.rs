use anyhow::Result;
use log::{info, warn, error};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use poll_promise::Promise;

use crate::{BingImage, Config, load_market_codes, get_old_market_codes, load_historical_metadata, get_image_metadata};

// UI Traits (previously from gui/mod.rs)
pub trait View {
    fn ui(&mut self, ui: &mut egui::Ui);
}

pub fn is_mobile(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.screen_rect().width() < 768.0)
}

pub trait Demo {
    fn is_enabled(&self, _ctx: &egui::Context) -> bool {
        true
    }
    fn name(&self) -> &'static str;
    fn show(&mut self, ctx: &egui::Context, open: &mut bool);
    
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

pub trait WallpaperSetter: Send + Sync {
    fn set_wallpaper_from_bytes(&self, image_bytes: &[u8]) -> std::io::Result<bool>;
}

pub trait ScreenSizeProvider: Send + Sync {
    fn get_screen_size(&self) -> std::io::Result<(i32, i32)>;
}

#[derive(Clone)]
pub struct CarouselImage {
    pub title: String,
    pub copyright: String,
    pub copyright_link: String,
    pub thumbnail_url: String,
    pub full_url: String,
    pub image_bytes: Option<Vec<u8>>,
}

pub struct BingtrayAppState {
    pub config: Option<Config>,
    pub market_code_index: usize,
    pub infinite_scroll_page_index: usize,
    pub current_market_codes: Vec<String>,
    pub loading_more: bool,
    pub wallpaper_setter: Option<Arc<dyn WallpaperSetter + Send + Sync>>,
    pub screen_size_provider: Option<Arc<dyn ScreenSizeProvider + Send + Sync>>,
    pub cached_screen_size: Option<(f32, f32)>,
    pub screen_size_failed: bool,
    pub seen_image_names: HashSet<String>,
    pub showing_historical: bool,
    pub market_exhausted: bool,
    pub market_code_timestamps: HashMap<String, i64>,
    pub all_data_exhausted: bool,
    pub showing_cached: bool,
    pub cached_page_index: usize,
    pub carousel_images: Vec<CarouselImage>,
    pub selected_carousel_image: Option<CarouselImage>,
    pub main_panel_image: Option<CarouselImage>,
    pub image_cache: HashMap<String, CarouselImage>,
    pub bing_api_promise: Option<Promise<Result<Vec<BingImage>, String>>>,
    pub carousel_promises: Vec<Promise<ehttp::Result<CarouselImage>>>,
    pub main_panel_promise: Option<Promise<ehttp::Result<CarouselImage>>>,
    pub wallpaper_status: Option<String>,
    pub wallpaper_start_time: Option<SystemTime>,
}

impl Default for BingtrayAppState {
    fn default() -> Self {
        let config = Config::new().ok();
        info!("Config creation result: {:?}", config.is_some());
        
        let (current_market_codes, market_code_timestamps) = if let Some(ref cfg) = config {
            info!("Loading market codes from config (will use marketcodes.conf if available)");
            match load_market_codes(cfg) {
                Ok(codes) => {
                    let old_codes = get_old_market_codes(&codes);
                    info!("Successfully loaded {} market codes from config", old_codes.len());
                    if codes.len() > 0 && cfg.marketcodes_file.exists() {
                        info!("Market codes loaded from local file: {:?}", cfg.marketcodes_file);
                    } else if codes.len() > 0 {
                        info!("Market codes fetched from internet and will be saved to: {:?}", cfg.marketcodes_file);
                    }
                    (old_codes, codes)
                }
                Err(e) => {
                    warn!("Failed to load market codes from config: {}, using fallback", e);
                    (vec!["en-US".to_string()], HashMap::new())
                }
            }
        } else {
            info!("No config available, using default market codes");
            (vec!["en-US".to_string()], HashMap::new())
        };
        info!("Final market codes: {:?}", current_market_codes);

        Self {
            config,
            market_code_index: 0,
            infinite_scroll_page_index: 0,
            current_market_codes,
            loading_more: false,
            wallpaper_setter: None,
            screen_size_provider: None,
            cached_screen_size: None,
            screen_size_failed: false,
            seen_image_names: HashSet::new(),
            showing_historical: false,
            market_exhausted: false,
            market_code_timestamps,
            all_data_exhausted: false,
            showing_cached: false,
            cached_page_index: 0,
            carousel_images: Vec::new(),
            selected_carousel_image: None,
            main_panel_image: None,
            image_cache: HashMap::new(),
            bing_api_promise: None,
            carousel_promises: Vec::new(),
            main_panel_promise: None,
            wallpaper_status: None,
            wallpaper_start_time: None,
        }
    }
}

impl BingtrayAppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_wallpaper_setter(mut self, setter: Arc<dyn WallpaperSetter + Send + Sync>) -> Self {
        self.wallpaper_setter = Some(setter);
        self
    }

    pub fn with_screen_size_provider(mut self, provider: Arc<dyn ScreenSizeProvider + Send + Sync>) -> Self {
        self.screen_size_provider = Some(provider);
        self
    }

    pub fn set_wallpaper_from_bytes(&self, image_bytes: &[u8]) -> std::io::Result<bool> {
        if let Some(ref setter) = self.wallpaper_setter {
            setter.set_wallpaper_from_bytes(image_bytes)
        } else {
            warn!("No wallpaper setter configured");
            Ok(false)
        }
    }

    pub fn get_screen_size(&self) -> (i32, i32) {
        if let Some(ref provider) = self.screen_size_provider {
            match provider.get_screen_size() {
                Ok(size) => size,
                Err(e) => {
                    error!("Failed to get screen size: {}", e);
                    (1920, 1080) // Fallback
                }
            }
        } else {
            info!("No screen size provider configured, using default");
            (1920, 1080)
        }
    }

    pub fn get_initial_screen_size() -> (f32, f32) {
        // Default screen size if no provider is available
        (1920.0, 1080.0)
    }

    pub fn get_actual_screen_size(&mut self) -> (f32, f32) {
        if let Some(cached) = self.cached_screen_size {
            return cached;
        }

        if self.screen_size_failed {
            return (1920.0, 1080.0);
        }

        let (width, height) = self.get_screen_size();
        let screen_size = (width as f32, height as f32);
        
        if width > 0 && height > 0 {
            self.cached_screen_size = Some(screen_size);
            info!("Successfully retrieved screen size: {}x{}", width, height);
        } else {
            self.screen_size_failed = true;
            warn!("Failed to get valid screen size, using default");
            return (1920.0, 1080.0);
        }

        screen_size
    }

    pub fn has_next_wallpaper_available(&self) -> bool {
        if self.carousel_images.is_empty() {
            return false;
        }
        
        if self.showing_cached {
            return true;
        }
        
        if self.showing_historical {
            return true;
        }
        
        if self.market_code_index < self.current_market_codes.len() {
            return true;
        }
        
        false
    }

    pub fn resolve_url(url: &str) -> Option<String> {
        if url.is_empty() {
            return None;
        }
        
        if url.starts_with("http://") || url.starts_with("https://") {
            Some(url.to_string())
        } else if url.starts_with("//") {
            Some(format!("https:{}", url))
        } else if url.starts_with("/") {
            Some(format!("https://www.bing.com{}", url))
        } else {
            Some(format!("https://{}", url))
        }
    }

    pub fn is_market_code_recent(&self, market_code: &str) -> bool {
        if let Some(&timestamp) = self.market_code_timestamps.get(market_code) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            
            let hours_since_request = (now - timestamp) / 3600;
            hours_since_request < 24
        } else {
            false
        }
    }

    pub fn update_market_code_timestamp(&mut self, market_code: &str) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        
        self.market_code_timestamps.insert(market_code.to_string(), timestamp);
        self.save_market_code_timestamps();
    }

    pub fn load_market_code_timestamps(&mut self) {
        if let Some(ref config) = self.config {
            let timestamps_file = config.config_dir.join("market_code_timestamps.conf");
            if let Ok(content) = std::fs::read_to_string(&timestamps_file) {
                let mut timestamps = HashMap::new();
                for line in content.lines() {
                    if let Some((code, timestamp_str)) = line.split_once('|') {
                        if let Ok(timestamp) = timestamp_str.parse::<i64>() {
                            timestamps.insert(code.to_string(), timestamp);
                        }
                    }
                }
                self.market_code_timestamps = timestamps;
                info!("Loaded {} market code timestamps", self.market_code_timestamps.len());
            }
        }
    }

    pub fn save_market_code_timestamps(&self) {
        if let Some(ref config) = self.config {
            let timestamps_file = config.config_dir.join("market_code_timestamps.conf");
            let mut content = String::new();
            for (code, timestamp) in &self.market_code_timestamps {
                content.push_str(&format!("{}|{}\n", code, timestamp));
            }
            if let Err(e) = std::fs::write(&timestamps_file, content) {
                error!("Failed to save market code timestamps: {}", e);
            }
        }
    }

    pub fn load_cached_images(&mut self) -> Result<()> {
        if let Some(ref config) = self.config {
            match load_historical_metadata(config) {
                Ok((_, historical_images)) => {
                    info!("Loaded {} cached images from metadata", historical_images.len());
                    
                    for bing_image in historical_images {
                        let thumbnail_url = Self::resolve_url(&bing_image.url).unwrap_or_else(|| bing_image.url.clone());
                        let full_url = thumbnail_url.clone();
                        
                        let carousel_image = CarouselImage {
                            title: bing_image.title.clone(),
                            thumbnail_url: thumbnail_url.clone(),
                            full_url: full_url.clone(),
                            image_bytes: None,
                            copyright: bing_image.copyright.clone(),
                            copyright_link: bing_image.copyrightlink.clone(),
                        };
                        
                        self.carousel_images.push(carousel_image.clone());
                        self.image_cache.insert(bing_image.title.clone(), carousel_image);
                    }
                    
                    info!("Added {} cached images to carousel", self.carousel_images.len());
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to load cached images: {}", e);
                    Err(e)
                }
            }
        } else {
            Err(anyhow::anyhow!("No config available"))
        }
    }

    pub fn get_image_bing_url(&self, image_title: &str) -> Option<String> {
        if let Some(ref config) = self.config {
            match get_image_metadata(config, image_title) {
                Some((_, bing_url)) => {
                    info!("Found Bing URL for cached image {}: {}", image_title, bing_url);
                    Some(bing_url)
                }
                None => {
                    error!("Could not find Bing URL for cached image: {}", image_title);
                    None
                }
            }
        } else {
            None
        }
    }
}

// Main Application struct (previously from gui/bingtray_app.rs)
pub struct BingtrayApp {
    egui_app: crate::core::egui::BingtrayEguiApp,
}

impl Default for BingtrayApp {
    fn default() -> Self {
        let app_state = BingtrayAppState::default();
        let egui_app = crate::core::egui::BingtrayEguiApp::new(app_state);
        
        Self {
            egui_app,
        }
    }
}

impl BingtrayApp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_wallpaper_setter(mut self, setter: Arc<dyn WallpaperSetter + Send + Sync>) -> Self {
        let app_state = self.egui_app.app_state_mut();
        app_state.wallpaper_setter = Some(setter);
        self
    }

    pub fn with_screen_size_provider(mut self, provider: Arc<dyn ScreenSizeProvider + Send + Sync>) -> Self {
        let app_state = self.egui_app.app_state_mut();
        app_state.screen_size_provider = Some(provider);
        self
    }

    // Legacy methods for compatibility
    pub fn get_screen_size(&self) -> (i32, i32) {
        self.egui_app.app_state().get_screen_size()
    }

    pub fn set_wallpaper_from_bytes(&self, image_bytes: &[u8]) -> std::io::Result<bool> {
        self.egui_app.app_state().set_wallpaper_from_bytes(image_bytes)
    }

    // Delegate to the new egui app
    pub fn ui(&mut self, ctx: &egui::Context) {
        self.egui_app.ui(ctx);
    }
}

impl Demo for BingtrayApp {
    fn name(&self) -> &'static str {
        "🖼 Bingtray"
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn show(&mut self, ctx: &egui::Context, _open: &mut bool) {
        self.ui(ctx);
    }
}

impl eframe::App for BingtrayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ui(ctx);
    }
}

impl View for BingtrayApp {
    fn ui(&mut self, ui: &mut egui::Ui) {
        // Delegate to the new modular structure
        View::ui(&mut self.egui_app, ui);
    }
}
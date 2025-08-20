use egui::{Align2, Vec2};
use log::{info, error};
use poll_promise::Promise;

use crate::{BingImage, get_bing_images};
use crate::core::app::{BingtrayAppState, CarouselImage};
use crate::core::egui_carousel::{EguiCarouselState, Resource};
use crate::core::app::{Demo, View};

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct BingtrayEguiApp {
    // UI state
    title: String,
    title_bar: bool,
    collapsible: bool,
    resizable: bool,
    constrain: bool,
    anchored: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    anchor: Align2,
    #[cfg_attr(feature = "serde", serde(skip))]
    anchor_offset: Vec2,

    // URL and promise handling
    url: String,
    #[cfg_attr(feature = "serde", serde(skip))]
    promise: Option<Promise<ehttp::Result<Resource>>>,

    // Application state and carousel
    #[cfg_attr(feature = "serde", serde(skip))]
    app_state: BingtrayAppState,
    #[cfg_attr(feature = "serde", serde(skip))]
    carousel_state: EguiCarouselState,
}

impl Default for BingtrayEguiApp {
    fn default() -> Self {
        Self {
            title: "Bingtray".to_owned(),
            title_bar: true,
            collapsible: true,
            resizable: true,
            constrain: true,
            anchored: false,
            anchor: Align2::RIGHT_TOP,
            anchor_offset: Vec2::ZERO,
            url: "https://www.bing.com".to_owned(),
            promise: None,
            app_state: BingtrayAppState::default(),
            carousel_state: EguiCarouselState::default(),
        }
    }
}

impl BingtrayEguiApp {
    pub fn new(app_state: BingtrayAppState) -> Self {
        Self {
            app_state,
            ..Default::default()
        }
    }

    pub fn with_title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    pub fn app_state(&self) -> &BingtrayAppState {
        &self.app_state
    }

    pub fn app_state_mut(&mut self) -> &mut BingtrayAppState {
        &mut self.app_state
    }

    fn load_more_images(&mut self, ctx: &egui::Context) {
        if self.app_state.loading_more {
            return;
        }

        self.app_state.loading_more = true;

        if self.app_state.showing_cached {
            self.load_cached_images_batch(ctx);
        } else if self.app_state.showing_historical {
            self.load_historical_images_batch(ctx);
        } else {
            self.load_regular_images_batch(ctx);
        }
    }

    fn load_cached_images_batch(&mut self, _ctx: &egui::Context) {
        if let Err(e) = self.app_state.load_cached_images() {
            error!("Failed to load cached images: {}", e);
            self.app_state.all_data_exhausted = true;
        }
        self.app_state.loading_more = false;
    }

    fn load_historical_images_batch(&mut self, _ctx: &egui::Context) {
        // Implementation for loading historical images
        // This would be similar to the cached images but for historical data
        self.app_state.loading_more = false;
    }

    fn load_regular_images_batch(&mut self, ctx: &egui::Context) {
        if self.app_state.market_code_index >= self.app_state.current_market_codes.len() {
            info!("All market codes processed, switching to historical images");
            self.app_state.showing_historical = true;
            self.app_state.market_exhausted = true;
            self.load_historical_images_batch(ctx);
            return;
        }

        let market_code = self.app_state.current_market_codes[self.app_state.market_code_index].clone();
        
        if self.app_state.is_market_code_recent(&market_code) {
            info!("Market code {} is recent, skipping", market_code);
            self.app_state.market_code_index += 1;
            self.app_state.loading_more = false;
            return;
        }

        info!("Loading images for market code: {}", market_code);

        let market_code_clone = market_code.clone();
        
        let result = match get_bing_images(&market_code_clone) {
            Ok(images) => {
                info!("Successfully fetched {} images for market code: {}", images.len(), market_code_clone);
                Ok(images)
            }
            Err(e) => {
                error!("Failed to fetch images for market code {}: {}", market_code_clone, e);
                Err(format!("Failed to fetch images: {}", e))
            }
        };
        let promise = Promise::from_ready(result);

        self.app_state.bing_api_promise = Some(promise);
        self.app_state.update_market_code_timestamp(&market_code);
        self.app_state.market_code_index += 1;
    }

    fn process_bing_images(&mut self, images: Vec<BingImage>, ctx: &egui::Context) {
        for bing_image in images {
            if self.app_state.seen_image_names.contains(&bing_image.title) {
                continue;
            }

            self.app_state.seen_image_names.insert(bing_image.title.clone());

            let thumbnail_url = if let Some(url) = BingtrayAppState::resolve_url(&bing_image.url) {
                url
            } else {
                continue;
            };

            let full_url = thumbnail_url.clone();
            let title = bing_image.title.clone();
            let copyright = bing_image.copyright.clone().unwrap_or_else(|| "Bing Image".to_string());
            let copyright_link = bing_image.copyrightlink.clone().unwrap_or_else(|| "#".to_string());

            let ctx_clone = ctx.clone();
            let title_clone = title.clone();
            let thumbnail_url_clone = thumbnail_url.clone();
            let full_url_clone = full_url.clone();
            let copyright_clone = copyright.clone();
            let copyright_link_clone = copyright_link.clone();
            
            ehttp::fetch(ehttp::Request::get(&thumbnail_url), move |result| {
                match result {
                    Ok(response) => {
                        let _carousel_image = CarouselImage {
                            title: title_clone,
                            thumbnail_url: thumbnail_url_clone,
                            full_url: full_url_clone,
                            image_bytes: Some(response.bytes),
                            copyright: copyright_clone,
                            copyright_link: copyright_link_clone,
                        };
                        ctx_clone.request_repaint();
                        // Store the result somewhere that can be picked up in the next frame
                        // For now, we'll skip adding to promises since ehttp handles this differently
                    }
                    Err(e) => {
                        error!("Failed to fetch image {}: {}", title_clone, e);
                    }
                }
            });
        }

        self.app_state.loading_more = false;
    }

    fn render_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Load Regular Images").clicked() {
                self.app_state.showing_historical = false;
                self.app_state.showing_cached = false;
                self.app_state.market_code_index = 0;
                self.app_state.all_data_exhausted = false;
                self.app_state.carousel_images.clear();
                self.app_state.image_cache.clear();
                info!("Switched to regular images mode");
            }

            if ui.button("Load Historical Images").clicked() {
                self.app_state.showing_historical = true;
                self.app_state.showing_cached = false;
                self.app_state.carousel_images.clear();
                self.app_state.image_cache.clear();
                info!("Switched to historical images mode");
            }

            if ui.button("Load Cached Images").clicked() {
                self.app_state.showing_cached = true;
                self.app_state.showing_historical = false;
                self.app_state.cached_page_index = 0;
                self.app_state.carousel_images.clear();
                self.app_state.image_cache.clear();
                
                if let Err(e) = self.app_state.load_cached_images() {
                    error!("Failed to load cached images: {}", e);
                } else {
                    info!("Loaded cached images from metadata");
                }
            }
        });

        ui.separator();
    }

    fn update_promises(&mut self, ctx: &egui::Context) {
        // Process Bing API promise
        if let Some(promise) = self.app_state.bing_api_promise.take() {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(images) => {
                        info!("Received {} images from Bing API", images.len());
                        self.process_bing_images(images.clone(), ctx);
                    }
                    Err(e) => {
                        error!("Bing API request failed: {}", e);
                        self.app_state.loading_more = false;
                    }
                }
            } else {
                // Put the promise back if not ready
                self.app_state.bing_api_promise = Some(promise);
            }
        }
    }
}

impl Demo for BingtrayEguiApp {
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

impl eframe::App for BingtrayEguiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ui(ctx);
    }
}

impl View for BingtrayEguiApp {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.update_promises(&ctx);

        self.render_controls(ui);

        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(200.0);
                ui.heading("Image Carousel");
                
                self.carousel_state.render_carousel_images(ui, &mut self.app_state, &ctx);
            });

            ui.separator();

            ui.vertical(|ui| {
                ui.heading("Main Panel");
                self.carousel_state.render_main_panel(ui, &mut self.app_state);
            });
        });

        // Auto-load more images if needed
        if !self.app_state.loading_more && self.app_state.carousel_images.len() < 10 && !self.app_state.all_data_exhausted {
            self.load_more_images(&ctx);
        }
    }
}

impl BingtrayEguiApp {
    pub fn ui(&mut self, ctx: &egui::Context) {
        
        let mut window = egui::Window::new(&self.title)
            .collapsible(self.collapsible)
            .resizable(self.resizable)
            .title_bar(self.title_bar)
            .constrain(self.constrain);

        if self.anchored {
            window = window.anchor(self.anchor, self.anchor_offset);
        }

        window.show(ctx, |ui| {
            View::ui(self, ui);
        });
    }
}
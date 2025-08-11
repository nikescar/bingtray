use egui::Image;
use poll_promise::Promise;
use egui::{Vec2b, Pos2, pos2, Rect, Sense, Shape, Stroke, Vec2, emath};
use egui::epaint::StrokeKind;
use log::{trace, warn, info, error};
use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;
use bingtray_core::*;

#[cfg(target_os = "android")]
use crate::android_screensize::get_screen_size;

#[cfg(not(target_os = "android"))]
use screen_size;


#[derive(Clone)]
struct CarouselImage {
    title: String,
    copyright: String,
    copyright_link: String,
    thumbnail_url: String,
    full_url: String,
    image: Option<Image<'static>>,
    image_bytes: Option<Vec<u8>>,
}

struct Resource {
    response: ehttp::Response,
    text: Option<String>,
    image: Option<Image<'static>>,
}

impl Resource {
    fn from_response(ctx: &egui::Context, response: ehttp::Response) -> Self {
        let content_type = response.content_type().unwrap_or_default();
        if content_type.starts_with("image/") {
            // Use include_bytes method and ensure proper image loading
            ctx.include_bytes(response.url.clone(), response.bytes.clone());
            // Force a repaint to ensure the image is loaded
            ctx.request_repaint();
            let image = Image::from_uri(response.url.clone());
            trace!("Image URL: {} (size: {} bytes)", response.url, response.bytes.len());

            Self {
                response,
                text: None,
                image: Some(image),
            }
        } else {
            let text = response.text();
            let text = text.map(|text| text.to_owned());

            Self {
                response,
                text,
                image: None,
            }
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct BingtrayApp {
    title: String,
    title_bar: bool,
    collapsible: bool,
    resizable: bool,
    constrain: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    scroll2: Vec2b,
    anchored: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    anchor: egui::Align2,
    #[cfg_attr(feature = "serde", serde(skip))]
    anchor_offset: egui::Vec2,

    url: String,
    #[cfg_attr(feature = "serde", serde(skip))]
    promise: Option<Promise<ehttp::Result<Resource>>>,
    #[cfg_attr(feature = "serde", serde(skip))]
    wallpaper_status: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip))]
    wallpaper_start_time: Option<SystemTime>,
    #[cfg_attr(feature = "serde", serde(skip))]
    carousel_images: Vec<CarouselImage>,
    #[cfg_attr(feature = "serde", serde(skip))]
    carousel_promises: Vec<Promise<ehttp::Result<CarouselImage>>>,
    #[cfg_attr(feature = "serde", serde(skip))]
    selected_carousel_image: Option<CarouselImage>,
    #[cfg_attr(feature = "serde", serde(skip))]
    main_panel_image: Option<CarouselImage>,
    #[cfg_attr(feature = "serde", serde(skip))]
    main_panel_promise: Option<Promise<ehttp::Result<CarouselImage>>>,
    #[cfg_attr(feature = "serde", serde(skip))]
    image_cache: std::collections::HashMap<String, CarouselImage>,
    #[cfg_attr(feature = "serde", serde(skip))]
    bing_api_promise: Option<Promise<Result<Vec<BingImage>, String>>>,
    #[cfg_attr(feature = "serde", serde(skip))]
    config: Option<Config>,
    #[cfg_attr(feature = "serde", serde(skip))]
    market_code_index: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    infinite_scroll_page_index: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    current_market_codes: Vec<String>,
    #[cfg_attr(feature = "serde", serde(skip))]
    scroll_position: f32,
    #[cfg_attr(feature = "serde", serde(skip))]
    loading_more: bool,
    // Screen ratio square shape fields
    #[cfg_attr(feature = "serde", serde(skip))]
    square_corners: [Pos2; 4],
    #[cfg_attr(feature = "serde", serde(skip))]
    square_size_factor: f32,
    #[cfg_attr(feature = "serde", serde(skip))]
    square_center: Pos2,
    #[cfg_attr(feature = "serde", serde(skip))]
    dragging_corner: Option<usize>,
    #[cfg_attr(feature = "serde", serde(skip))]
    screen_ratio: f32,
    #[cfg_attr(feature = "serde", serde(skip))]
    reset_rectangle_for_new_image: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    current_main_image_url: Option<String>,
    #[cfg_attr(feature = "serde", serde(skip))]
    cached_screen_size: Option<(f32, f32)>,
    #[cfg_attr(feature = "serde", serde(skip))]
    screen_size_failed: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    seen_image_names: std::collections::HashSet<String>,
    #[cfg_attr(feature = "serde", serde(skip))]
    image_display_rect: Option<egui::Rect>,
    // New fields for historical images and market code caching
    #[cfg_attr(feature = "serde", serde(skip))]
    showing_historical: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    market_exhausted: bool,
    #[cfg_attr(feature = "serde", serde(skip))]
    market_code_timestamps: std::collections::HashMap<String, i64>,
}

impl Default for BingtrayApp {
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
                    (vec!["en-US".to_string()], std::collections::HashMap::new())
                }
            }
        } else {
            info!("No config available, using default market codes");
            (vec!["en-US".to_string()], std::collections::HashMap::new())
        };
        info!("Final market codes: {:?}", current_market_codes);
        
        // Get actual screen size for rectangle calculation
        let (screen_width, screen_height) = Self::get_initial_screen_size();
        let screen_ratio = screen_width / screen_height;
        
        info!("Detected screen size: {}x{}, ratio: {:.2}", screen_width, screen_height, screen_ratio);
        
        // Initialize rectangle based on screen dimensions (scaled down to fit in UI)
        let rect_scale_factor = 0.3; // Start with 30% of screen size
        let rect_width = screen_width * rect_scale_factor;
        let rect_height = screen_height * rect_scale_factor;
        
        // Center the rectangle initially
        let square_center = pos2(400.0, 300.0); // Will be updated when image is loaded
        let half_width = rect_width / 2.0;
        let half_height = rect_height / 2.0;
        
        let square_corners = [
            pos2(square_center.x - half_width, square_center.y - half_height), // Top-left
            pos2(square_center.x + half_width, square_center.y - half_height), // Top-right
            pos2(square_center.x + half_width, square_center.y + half_height), // Bottom-right
            pos2(square_center.x - half_width, square_center.y + half_height), // Bottom-left
        ];
        
        Self {
            title: "BingtrayApp Window".to_owned(),
            title_bar: false,
            collapsible: false,
            resizable: false,
            constrain: false,
            scroll2: Vec2b::TRUE,
            anchored: true,
            anchor: egui::Align2::CENTER_TOP,
            anchor_offset: egui::Vec2::ZERO,

            url: String::new(),
            promise: Default::default(),
            wallpaper_status: None,
            wallpaper_start_time: None,
            carousel_images: Vec::new(),
            carousel_promises: Vec::new(),
            selected_carousel_image: None,
            main_panel_image: None,
            main_panel_promise: None,
            image_cache: std::collections::HashMap::new(),
            bing_api_promise: None,
            config,
            market_code_index: 0,
            infinite_scroll_page_index: 0,
            current_market_codes,
            scroll_position: 0.0,
            loading_more: false,
            square_corners,
            square_size_factor: rect_scale_factor, // Use the actual rectangle scale factor
            square_center,
            dragging_corner: None,
            screen_ratio,
            reset_rectangle_for_new_image: false,
            current_main_image_url: None,
            cached_screen_size: None,
            screen_size_failed: false,
            seen_image_names: std::collections::HashSet::new(),
            image_display_rect: None,
            // Initialize new fields
            showing_historical: false,
            market_exhausted: false,
            market_code_timestamps,
        }
    }
}

impl crate::Demo for BingtrayApp {
    fn name(&self) -> &'static str {
        "BingtrayApp"
    }

    fn show(&mut self, ctx: &egui::Context, _open: &mut bool) {
        use crate::View as _;

        // Always show the window regardless of internet connectivity
        // Get screen size with fallback values to ensure UI remains functional
        #[cfg(target_os = "android")]
        let screen_size = get_screen_size().unwrap_or((1080, 1920)); // Default mobile portrait
        #[cfg(not(target_os = "android"))]
        let screen_size = screen_size::get_primary_screen_size().unwrap_or((1920, 1080)); // Default desktop

        // Create window that's always visible and functional
        let mut window = egui::Window::new(&self.title)
            .default_width(screen_size.0 as f32)
            .default_height(screen_size.1 as f32)
            .id(egui::Id::new("demo_window_options"))
            .resizable(self.resizable)
            .constrain(self.constrain)
            .collapsible(self.collapsible)
            .title_bar(self.title_bar)
            .scroll(self.scroll2);
            
        if self.anchored {
            window = window.anchor(self.anchor, self.anchor_offset);
        }
        
        // Always show the window - UI functionality should not depend on network connectivity
        window.show(ctx, |ui| {
            // Ensure UI is always rendered, even without network
            self.ui(ui);
        });
    }
}


impl crate::View for BingtrayApp {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let prev_url = self.url.clone();
        let trigger_fetch = ui_url(ui, &mut self.url, &mut self.carousel_images, &mut self.carousel_promises, &mut self.selected_carousel_image, &mut self.main_panel_image, &mut self.main_panel_promise, &mut self.image_cache, &mut self.bing_api_promise, &mut self.config, &mut self.market_code_index, &mut self.infinite_scroll_page_index, &mut self.current_market_codes, &mut self.scroll_position, &mut self.loading_more, &mut self.reset_rectangle_for_new_image, &mut self.current_main_image_url, &mut self.seen_image_names, &mut self.wallpaper_status, &mut self.wallpaper_start_time, &mut self.showing_historical, &mut self.market_exhausted, &mut self.market_code_timestamps);

        if trigger_fetch {
            let ctx = ui.ctx().clone();
            let (sender, promise) = Promise::new();
            let request = ehttp::Request::get(&self.url);
            
            ehttp::fetch(request, move |response| {
                ctx.forget_image(&prev_url);
                ctx.request_repaint(); // wake up UI thread
                let resource = response.map(|response| Resource::from_response(&ctx, response));
                sender.send(resource);
            });
            self.promise = Some(promise);
        }

        // Handle main panel promise completion  
        if let Some(main_promise) = &self.main_panel_promise {
            if let Some(result) = main_promise.ready() {
                match result {
                    Ok(full_res_image) => {
                        info!("Main panel high-res image loaded successfully");
                        // Update the main panel image
                        self.main_panel_image = Some(full_res_image.clone());
                        // Cache the high-res image
                        self.image_cache.insert(full_res_image.full_url.clone(), full_res_image.clone());
                        // Reset rectangle size based on new image
                        self.reset_rectangle_for_new_image = true;
                        // Clear the promise
                        self.main_panel_promise = None;
                        ui.ctx().request_repaint();
                    }
                    Err(e) => {
                        error!("Failed to load main panel high-res image: {}", e);
                        self.main_panel_promise = None;
                    }
                }
            }
        }

        // Handle Bing API promise completion
        let bing_api_result = if let Some(bing_promise) = &self.bing_api_promise {
            trace!("Checking Bing API promise...");
            bing_promise.ready().cloned()
        } else {
            None
        };
        
        if let Some(result) = bing_api_result {
            info!("=== Bing API promise completed ===");
            match result {
                Ok(bing_images) => {
                    info!("Bing API data received with {} images", bing_images.len());
                    for (i, img) in bing_images.iter().enumerate() {
                        info!("Image {}: title='{}', url='{}'", i, img.title, img.url);
                    }
                    
                    // Clear the Bing API promise immediately to re-enable the button
                    self.bing_api_promise = None;
                    
                    // Reset loading_more to allow more infinite scroll
                    self.loading_more = false;
                    
                    // Force immediate screen update
                    ui.ctx().request_repaint();
                    
                    // Process each image from the Bing API response
                    // Process all images without throttling to ensure all pages load
                    for bing_image in bing_images {
                            // Use bingtray-core functions for URL handling
                            let base_url = if bing_image.url.starts_with("http") {
                                bing_image.url.clone()
                            } else {
                                format!("https://bing.com{}", bing_image.url)
                            };
                            
                            // Create thumbnail and full URLs using the same method as core
                            let separator = if base_url.contains('?') { "&" } else { "?" };
                            let thumbnail_url = format!("{}{}w=320&h=240", base_url, separator);
                            let full_url = format!("{}{}w=1920&h=1080", base_url, separator);
                            
                            info!("Base URL: {}", base_url);
                            info!("Thumbnail URL: {}", thumbnail_url);
                            info!("Full URL: {}", full_url);
                            
                            // Extract display name using bingtray-core method
                            let display_name = bing_image.url
                                .split("th?id=")
                                .nth(1)
                                .and_then(|s| s.split('_').next())
                                .unwrap_or(&bing_image.title)
                                .to_string();
                            
                            // Skip duplicate images based on display name (e.g., OHR_AdelieWPD, OHR_AileyUptown)
                            let image_name = display_name.replace("OHR.", "");
                            if self.seen_image_names.contains(&image_name) {
                                info!("Skipping duplicate image: {}", image_name);
                                continue;
                            }
                            self.seen_image_names.insert(image_name.clone());
                            
                            let display_title = if bing_image.title == "Info" || bing_image.title.is_empty() {
                                sanitize_filename(&display_name).replace("OHR.", "").replace("_", " ")
                            } else {
                                bing_image.title.clone()
                            };
                            
                            let carousel_image = CarouselImage {
                                title: display_title.clone(),
                                copyright: bing_image.copyright.clone().unwrap_or_default(),
                                copyright_link: bing_image.copyrightlink.clone().unwrap_or_default(),
                                thumbnail_url: thumbnail_url.clone(),
                                full_url: full_url.clone(),
                                image: None,
                                image_bytes: None,
                            };
                            
                            self.carousel_images.push(carousel_image.clone());
                            
                            // Check cache before downloading thumbnails
                            if self.image_cache.contains_key(&thumbnail_url) {
                                info!("Using cached thumbnail for: {}", display_title);
                                continue;
                            }
                            
                            // Download thumbnails using the same pattern as core
                            let ctx = ui.ctx().clone();
                            let (sender, promise) = Promise::new();
                            let request = ehttp::Request::get(&thumbnail_url);
                            
                            ehttp::fetch(request, move |response| {
                            ctx.request_repaint();
                            let result = response.map(|response| {
                                info!("Bing image response: status={}, size={} bytes", response.status, response.bytes.len());
                                
                                if response.status == 200 && !response.bytes.is_empty() {
                                    let image_bytes = response.bytes.to_vec();
                                    info!("Loading carousel image: {} bytes from {}", image_bytes.len(), response.url);
                                    ctx.include_bytes(response.url.clone(), response.bytes.clone());
                                    ctx.request_repaint();
                                    let image = Image::from_uri(response.url.clone());
                                    info!("Created carousel image widget for: {}", response.url);
                                    
                                    CarouselImage {
                                        title: carousel_image.title.clone(),
                                        copyright: carousel_image.copyright.clone(),
                                        copyright_link: carousel_image.copyright_link.clone(),
                                        thumbnail_url: carousel_image.thumbnail_url.clone(),
                                        full_url: carousel_image.full_url.clone(),
                                        image: Some(image),
                                        image_bytes: Some(image_bytes),
                                    }
                                } else {
                                    // Try fallback URL patterns for 404 errors
                                    // if response.status == 404 {
                                    //     info!("Attempting fallback URL patterns for 404 error");
                                        
                                    //     // Extract the image ID from the failed URL and try alternative patterns
                                    //     let original_url = &response.url;
                                    //     let fallback_urls = generate_fallback_urls(original_url);
                                        
                                    //     if !fallback_urls.is_empty() {
                                    //         for fallback_url in fallback_urls {
                                    //             info!("Trying fallback URL: {}", fallback_url);
                                    //             let ctx = ctx.clone();
                                    //             let carousel_image_for_fallback = carousel_image.clone();
                                    //             let (sender, _promise) = Promise::new();
                                    //             let request = ehttp::Request::get(&fallback_url);
                                                
                                    //             ehttp::fetch(request, move |result| {
                                    //                 let result = result.map(|response| {
                                    //                     if response.status == 200 && !response.bytes.is_empty() {
                                    //                         let image_bytes = response.bytes.to_vec();
                                    //                         info!("Fallback URL successful: {} bytes from {}", image_bytes.len(), response.url);
                                    //                         ctx.include_bytes(response.url.clone(), response.bytes.clone());
                                    //                         ctx.request_repaint();
                                    //                         let image = Image::from_uri(response.url.clone());
                                                            
                                    //                         CarouselImage {
                                    //                             title: carousel_image_for_fallback.title.clone(),
                                    //                             copyright: carousel_image_for_fallback.copyright.clone(),
                                    //                             copyright_link: carousel_image_for_fallback.copyright_link.clone(),
                                    //                             thumbnail_url: carousel_image_for_fallback.thumbnail_url.clone(),
                                    //                             full_url: carousel_image_for_fallback.full_url.clone(),
                                    //                             image: Some(image),
                                    //                             image_bytes: Some(image_bytes),
                                    //                         }
                                    //                     } else {
                                    //                         info!("Fallback URL also failed: status {} - marking for removal", response.status);
                                    //                         // Return a special marker to indicate this image should be removed
                                    //                         CarouselImage {
                                    //                             title: "REMOVE_ME".to_string(), // Special marker
                                    //                             copyright: carousel_image_for_fallback.copyright.clone(),
                                    //                             copyright_link: carousel_image_for_fallback.copyright_link.clone(),
                                    //                             thumbnail_url: carousel_image_for_fallback.thumbnail_url.clone(),
                                    //                             full_url: carousel_image_for_fallback.full_url.clone(),
                                    //                             image: None,
                                    //                             image_bytes: None,
                                    //                         }
                                    //                     }
                                    //                 });
                                    //                 sender.send(result);
                                    //             });
                                                
                                    //             // Try only the first fallback URL for now to avoid too many requests
                                    //             break;
                                    //         }
                                    //     } else {
                                    //         info!("No fallback URLs available - marking for removal");
                                    //         // No fallback URLs available, mark for removal
                                    //         return CarouselImage {
                                    //             title: "REMOVE_ME".to_string(), // Special marker
                                    //             copyright: carousel_image.copyright.clone(),
                                    //             copyright_link: carousel_image.copyright_link.clone(),
                                    //             thumbnail_url: carousel_image.thumbnail_url.clone(),
                                    //             full_url: carousel_image.full_url.clone(),
                                    //             image: None,
                                    //             image_bytes: None,
                                    //         };
                                    //     }
                                    // }
                                    
                                    // Default fallback - mark for removal
                                    info!("Failed to load image from {}: status {} - marking for removal", response.url, response.status);
                                    CarouselImage {
                                        title: "REMOVE_ME".to_string(), // Special marker
                                        copyright: carousel_image.copyright.clone(),
                                        copyright_link: carousel_image.copyright_link.clone(),
                                        thumbnail_url: carousel_image.thumbnail_url.clone(),
                                        full_url: carousel_image.full_url.clone(),
                                        image: None,
                                        image_bytes: None,
                                    }
                                }
                            });
                            sender.send(result);
                        });
                        
                        self.carousel_promises.push(promise);
                        }
                        
                        // Force another repaint after adding all promises
                        ui.ctx().request_repaint();
                }
                Err(e) => {
                    error!("Failed to fetch Bing API data: {}", e);
                    self.bing_api_promise = None;
                    self.loading_more = false; // Reset loading state on error
                    
                    // Show error message to user
                    self.wallpaper_status = Some(format!("✗ {}", e));
                    self.wallpaper_start_time = Some(SystemTime::now());
                    
                    // Force repaint to update UI on error
                    ui.ctx().request_repaint();
                }
            }
        }

        // Handle carousel image promise completion
        if !self.carousel_promises.is_empty() {
            info!("Processing {} carousel promises", self.carousel_promises.len());
        }
        let mut completed_indices = Vec::new();
        for (i, promise) in self.carousel_promises.iter().enumerate() {
            if let Some(result) = promise.ready() {
                completed_indices.push(i);
                match result {
                    Ok(carousel_image) => {
                        info!("Promise completed for image: {} (has image: {})", carousel_image.thumbnail_url, carousel_image.image.is_some());
                        
                        // Check if this image should be removed
                        if carousel_image.title == "REMOVE_ME" {
                            info!("Removing failed image from carousel: {}", carousel_image.thumbnail_url);
                            // Find and remove the image from carousel_images
                            self.carousel_images.retain(|img| {
                                !(img.thumbnail_url == carousel_image.thumbnail_url || img.full_url == carousel_image.full_url)
                            });
                            ui.ctx().request_repaint();
                        } else {
                            // Find the corresponding image in carousel_images and update it
                            let mut found = false;
                            for existing_img in self.carousel_images.iter_mut() {
                                if existing_img.thumbnail_url == carousel_image.thumbnail_url || existing_img.full_url == carousel_image.full_url {
                                    existing_img.image = carousel_image.image.clone();
                                    existing_img.image_bytes = carousel_image.image_bytes.clone();
                                    info!("Updated image in carousel for: {} (image: {})", existing_img.title, existing_img.image.is_some());
                                    found = true;
                                    break;
                                }
                            }
                            if !found {
                                warn!("Could not find matching carousel image for: {}", carousel_image.thumbnail_url);
                                // Let's also try to match by title as a fallback, but skip REMOVE_ME
                                if carousel_image.title != "REMOVE_ME" {
                                    for existing_img in self.carousel_images.iter_mut() {
                                        if existing_img.title == carousel_image.title {
                                            existing_img.image = carousel_image.image.clone();
                                            existing_img.image_bytes = carousel_image.image_bytes.clone();
                                            info!("Updated image in carousel by title for: {}", existing_img.title);
                                            break;
                                        }
                                    }
                                }
                            }
                            // Force immediate repaint when each image is updated
                            ui.ctx().request_repaint();
                        }
                    }
                    Err(e) => {
                        error!("Promise failed for carousel image: {}", e);
                    }
                }
            }
        }
        // Remove completed promises in reverse order to maintain indices
        for &i in completed_indices.iter().rev() {
            let _ = self.carousel_promises.remove(i);
        }
        
        // Note: Disabled deferred downloading to fix infinite loop on Android
        // The main download system during API response processing should be sufficient
        
        // Clean up images that have been in the carousel for a while but still have no image loaded
        // This handles cases where promises might have failed without being caught
        if self.carousel_promises.is_empty() {
            let initial_count = self.carousel_images.len();
            self.carousel_images.retain(|img| {
                // Keep images that have successfully loaded
                if img.image.is_some() {
                    true
                } else {
                    // Remove images that are placeholders and have been around for more than expected
                    info!("Removing image that failed to load: {}", img.title);
                    false
                }
            });
            if self.carousel_images.len() != initial_count {
                info!("Cleaned up {} failed images from carousel", initial_count - self.carousel_images.len());
                ui.ctx().request_repaint();
            }
        }

        // Force regular repaints while promises are active to ensure timely UI updates
        if !self.carousel_promises.is_empty() || self.bing_api_promise.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }

        ui.separator();

        if let Some(promise) = &self.promise {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(resource) => {
                        ui_resource(ui, resource, &mut self.wallpaper_status, &mut self.wallpaper_start_time);
                    }
                    Err(error) => {
                        // This should only happen if the fetch API isn't available or something similar.
                        ui.colored_label(
                            ui.visuals().error_fg_color,
                            if error.is_empty() { "Error" } else { error },
                        );
                    }
                }
            } else {
                ui.spinner();
            }
        }

        // Display main panel image (high resolution from selected carousel image)
        let has_main_image = self.main_panel_image.is_some();
        
        if has_main_image {
            // Clone the necessary data to avoid borrowing conflicts
            let main_image = self.main_panel_image.as_ref().unwrap().clone();
            
            ui.separator();
            ui.label(format!("Title: {}", main_image.title));
            // show copyright if available
            if !main_image.copyright.is_empty() {
                ui.label(format!("Copyright: {}", main_image.copyright));
            }
            
            // Add wallpaper button for images
            ui.horizontal(|ui| {
                if ui.button("Set this Wallpaper").clicked() {
                    if let Some(bytes) = &main_image.image_bytes {
                        if !bytes.is_empty() {
                            let image_data = bytes.clone();
                            info!("Starting wallpaper setting with {} bytes", image_data.len());
                            // Start wallpaper setting in background thread using bytes directly
                            std::thread::spawn(move || {
                                log::info!("BingtrayApp: Starting wallpaper setting from bytes in background thread");
                                match crate::set_wallpaper_from_bytes(&image_data) {
                                    Ok(true) => {
                                        log::info!("BingtrayApp: Wallpaper setting from bytes completed successfully");
                                    }
                                    Ok(false) => {
                                        log::error!("BingtrayApp: Wallpaper setting from bytes failed");
                                    }
                                    Err(e) => {
                                        log::error!("BingtrayApp: Error during wallpaper setting from bytes: {}", e);
                                    }
                                }
                            });
                            // Immediately update UI status without waiting
                            self.wallpaper_status = Some("✓ Wallpaper setting started (using bytes)".to_string());
                            self.wallpaper_start_time = Some(SystemTime::now());
                            ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
                            log::info!("BingtrayApp: Finished processing wallpaper setting request from bytes");
                        } else {
                            error!("No image data available");
                            self.wallpaper_status = Some("✗ No image data available".to_string());
                            self.wallpaper_start_time = Some(SystemTime::now());
                        }
                    } else {
                        error!("No image data available");
                        self.wallpaper_status = Some("✗ No image data available".to_string());
                        self.wallpaper_start_time = Some(SystemTime::now());
                    }
                }
                
                // Add cropped wallpaper button
                if ui.button("Set Cropped Wallpaper").clicked() {
                    if let Some(bytes) = &main_image.image_bytes {
                        if !bytes.is_empty() {
                            let image_data = bytes.clone();
                            
                            // Calculate crop rectangle using screen aspect ratio approach (like the Kotlin example)
                            info!("Square corners: {:?}", self.square_corners);
                            info!("Square center: {:?}, size factor: {}, screen ratio: {}", self.square_center, self.square_size_factor, self.screen_ratio);
                            
                            let crop_rect = {
                                // Get actual image dimensions from the image bytes
                                let (bitmap_width, bitmap_height) = if let Some(bytes) = &main_image.image_bytes {
                                    // Decode image to get actual dimensions using the correct API
                                    match image::ImageReader::new(std::io::Cursor::new(bytes))
                                        .with_guessed_format()
                                    {
                                        Ok(reader) => {
                                            match reader.decode() {
                                                Ok(img) => {
                                                    let (w, h) = (img.width() as i32, img.height() as i32);
                                                    info!("Decoded actual image dimensions: {}x{}", w, h);
                                                    (w, h)
                                                }
                                                Err(e) => {
                                                    warn!("Failed to decode image for dimensions: {}, using fallback", e);
                                                    (1920, 1080)
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            warn!("Failed to create image reader for dimensions: {}, using fallback", e);
                                            (1920, 1080)
                                        }
                                    }
                                } else {
                                    // Fallback to typical Bing wallpaper dimensions
                                    (1920, 1080)
                                };
                                
                                info!("Using bitmap {}x{} for selected rectangle cropping", bitmap_width, bitmap_height);
                                info!("Selected rectangle corners: top-left=({:.1},{:.1}), bottom-right=({:.1},{:.1})", 
                                      self.square_corners[0].x, self.square_corners[0].y,
                                      self.square_corners[2].x, self.square_corners[2].y);
                                
                                // Use selected rectangle coordinates, but extend right edge to image right end
                                // square_corners[0] = top-left, square_corners[2] = bottom-right
                                // Convert from image display coordinates to actual bitmap pixel coordinates
                                // The square_corners are in the coordinate system of the displayed image
                                // We need to scale them to match the actual bitmap dimensions
                                
                                let (left, top, bottom, right) = if let Some(display_rect) = self.image_display_rect {
                                    // Transform coordinates from display space to bitmap space
                                    let display_width = display_rect.width();
                                    let display_height = display_rect.height();
                                    
                                    // Calculate the actual image dimensions as displayed (considering aspect ratio)
                                    let image_aspect_ratio = bitmap_width as f32 / bitmap_height as f32;
                                    let display_aspect_ratio = display_width / display_height;
                                    
                                    // Determine actual image display area within the display_rect
                                    let (actual_img_width, actual_img_height, img_offset_x, img_offset_y) = if image_aspect_ratio > display_aspect_ratio {
                                        // Image is wider than display area - letterboxed top/bottom
                                        let actual_width = display_width;
                                        let actual_height = display_width / image_aspect_ratio;
                                        let offset_y = (display_height - actual_height) / 2.0;
                                        (actual_width, actual_height, 0.0, offset_y)
                                    } else {
                                        // Image is taller than display area - letterboxed left/right
                                        let actual_height = display_height;
                                        let actual_width = display_height * image_aspect_ratio;
                                        let offset_x = (display_width - actual_width) / 2.0;
                                        (actual_width, actual_height, offset_x, 0.0)
                                    };
                                    
                                    // Convert square corners from screen coordinates to image display coordinates
                                    // NOTE: The square corners are already relative to the displayed image, not screen
                                    let img_relative_top_left_x = self.square_corners[0].x;
                                    let img_relative_top_left_y = self.square_corners[0].y;
                                    let img_relative_bottom_right_x = self.square_corners[2].x;
                                    let img_relative_bottom_right_y = self.square_corners[2].y;
                                    
                                    // Clamp coordinates to image display area
                                    let clamped_top_left_x = img_relative_top_left_x.max(0.0).min(actual_img_width);
                                    let clamped_top_left_y = img_relative_top_left_y.max(0.0).min(actual_img_height);
                                    let clamped_bottom_right_x = img_relative_bottom_right_x.max(0.0).min(actual_img_width);
                                    let clamped_bottom_right_y = img_relative_bottom_right_y.max(0.0).min(actual_img_height);
                                    
                                    // Convert to relative coordinates within the actual image area (0.0 to 1.0)
                                    let rel_left = if actual_img_width > 0.0 { clamped_top_left_x / actual_img_width } else { 0.0 };
                                    let rel_top = if actual_img_height > 0.0 { clamped_top_left_y / actual_img_height } else { 0.0 };
                                    let rel_right = if actual_img_width > 0.0 { clamped_bottom_right_x / actual_img_width } else { 1.0 };
                                    let rel_bottom = if actual_img_height > 0.0 { clamped_bottom_right_y / actual_img_height } else { 1.0 };
                                    
                                    // Convert relative coordinates to bitmap pixel coordinates
                                    let left = ((rel_left * bitmap_width as f32).max(0.0)).min(bitmap_width as f32 - 1.0) as i32;
                                    let top = ((rel_top * bitmap_height as f32).max(0.0)).min(bitmap_height as f32 - 1.0) as i32;
                                    let right = bitmap_width; // Always extend to image's right edge as requested
                                    let bottom = ((rel_bottom * bitmap_height as f32).max(0.0)).min(bitmap_height as f32) as i32;
                                    
                                    // Ensure bottom is greater than top to avoid zero-height rectangles
                                    let bottom = if bottom <= top {
                                        warn!("Rectangle has zero or negative height! top={}, bottom={}, forcing minimum height", top, bottom);
                                        (top + bitmap_height / 4).min(bitmap_height) // Use 1/4 of image height as minimum
                                    } else {
                                        bottom
                                    };
                                    
                                    info!("Display rect: {}x{} at ({:.1},{:.1})", display_width, display_height, display_rect.min.x, display_rect.min.y);
                                    info!("Image aspect: {:.3}, display aspect: {:.3}", image_aspect_ratio, display_aspect_ratio);
                                    info!("Actual image area: {}x{} at offset ({:.1},{:.1})", actual_img_width, actual_img_height, img_offset_x, img_offset_y);
                                    info!("Original square corners: top-left=({:.1},{:.1}), bottom-right=({:.1},{:.1})", 
                                          self.square_corners[0].x, self.square_corners[0].y, 
                                          self.square_corners[2].x, self.square_corners[2].y);
                                    info!("Clamped square corners: top-left=({:.1},{:.1}), bottom-right=({:.1},{:.1})", 
                                          clamped_top_left_x, clamped_top_left_y, 
                                          clamped_bottom_right_x, clamped_bottom_right_y);
                                    info!("Relative coords: left={:.3}, top={:.3}, right={:.3}, bottom={:.3}", rel_left, rel_top, rel_right, rel_bottom);
                                    info!("Bitmap dimensions: {}x{}, transformed coords: left={}, top={}, right={}, bottom={}", bitmap_width, bitmap_height, left, top, right, bottom);
                                    
                                    (left, top, bottom, right)
                                } else {
                                    // Fallback if display rect is not available (shouldn't happen)
                                    warn!("Image display rect not available, using direct coordinate mapping");
                                    // Assume square corners are already in image coordinate space
                                    let left = (self.square_corners[0].x.max(0.0)).min(bitmap_width as f32 - 1.0) as i32;
                                    let top = (self.square_corners[0].y.max(0.0)).min(bitmap_height as f32 - 1.0) as i32;
                                    let right = bitmap_width; // Always extend to image's right edge as requested
                                    let bottom = (self.square_corners[2].y.max(0.0)).min(bitmap_height as f32) as i32;
                                    (left, top, bottom, right)
                                };
                                
                                // Ensure coordinates are within image bounds
                                let left = left.max(0).min(bitmap_width - 1);
                                let top = top.max(0).min(bitmap_height - 1);
                                let right = right.min(bitmap_width);
                                let bottom = bottom.max(top + 1).min(bitmap_height);
                                
                                let crop = (left, top, right, bottom);
                                info!("Final crop coordinates: left={}, top={}, right={}, bottom={} ({}x{} pixels)", 
                                      crop.0, crop.1, crop.2, crop.3, crop.2 - crop.0, crop.3 - crop.1);
                                info!("Selection represents {:.1}% width and {:.1}% height of original image",
                                      ((crop.2 - crop.0) as f32 / bitmap_width as f32) * 100.0,
                                      ((crop.3 - crop.1) as f32 / bitmap_height as f32) * 100.0);
                                Some(crop)
                            };
                            
                            if let Some(crop) = crop_rect {
                                info!("Starting cropped wallpaper setting with {} bytes, crop: left={}, top={}, right={}, bottom={}", image_data.len(), crop.0, crop.1, crop.2, crop.3);
                            } else {
                                info!("Starting cropped wallpaper setting with {} bytes, crop: None", image_data.len());
                            }
                            
                            // Use Rust image cropping to pre-crop the bytes before sending to Android
                            let final_image_data = if let Some((left, top, right, bottom)) = crop_rect {
                                info!("Attempting to crop image in Rust before sending to Android");
                                match image::ImageReader::new(std::io::Cursor::new(&image_data))
                                    .with_guessed_format()
                                {
                                    Ok(reader) => {
                                        match reader.decode() {
                                            Ok(mut img) => {
                                                info!("Successfully decoded image for cropping: {}x{}", img.width(), img.height());
                                                // Ensure crop coordinates are within image bounds
                                                let img_width = img.width() as i32;
                                                let img_height = img.height() as i32;
                                                let crop_left = left.max(0).min(img_width);
                                                let crop_top = top.max(0).min(img_height);
                                                let crop_right = right.max(crop_left).min(img_width);
                                                let crop_bottom = bottom.max(crop_top).min(img_height);
                                                
                                                info!("Cropping image from {}x{} with rect ({}, {}, {}, {})", 
                                                      img_width, img_height, crop_left, crop_top, crop_right, crop_bottom);
                                                
                                                // Crop the image
                                                let cropped_img = img.crop(
                                                    crop_left as u32, 
                                                    crop_top as u32, 
                                                    (crop_right - crop_left) as u32, 
                                                    (crop_bottom - crop_top) as u32
                                                );
                                                
                                                info!("Cropped image to {}x{}", cropped_img.width(), cropped_img.height());
                                                
                                                // Encode back to JPEG
                                                let mut output = Vec::new();
                                                match cropped_img.write_to(&mut std::io::Cursor::new(&mut output), image::ImageFormat::Jpeg) {
                                                    Ok(_) => {
                                                        info!("Successfully re-encoded cropped image: {} bytes", output.len());
                                                        output
                                                    }
                                                    Err(e) => {
                                                        warn!("Failed to re-encode cropped image: {}, using original", e);
                                                        image_data
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                warn!("Failed to decode image for cropping: {}, using original", e);
                                                image_data
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to create reader for cropping: {}, using original", e);
                                        image_data
                                    }
                                }
                            } else {
                                info!("No crop rectangle provided, using original image");
                                image_data
                            };
                            
                            // Start wallpaper setting in background thread using pre-cropped bytes
                            std::thread::spawn(move || {
                                log::info!("BingtrayApp: Starting wallpaper setting with pre-cropped image bytes");
                                match crate::set_wallpaper_from_bytes(&final_image_data) {
                                    Ok(true) => {
                                        log::info!("BingtrayApp: Cropped wallpaper setting completed successfully");
                                    }
                                    Ok(false) => {
                                        log::error!("BingtrayApp: Cropped wallpaper setting failed");
                                    }
                                    Err(e) => {
                                        log::error!("BingtrayApp: Error during cropped wallpaper setting: {}", e);
                                    }
                                }
                            });
                            
                            // Immediately update UI status without waiting
                            self.wallpaper_status = Some("✓ Cropped wallpaper setting started".to_string());
                            self.wallpaper_start_time = Some(SystemTime::now());
                            ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
                            log::info!("BingtrayApp: Finished processing cropped wallpaper setting request from bytes");
                        } else {
                            error!("No image data available for cropped wallpaper");
                            self.wallpaper_status = Some("✗ No image data available for cropped wallpaper".to_string());
                            self.wallpaper_start_time = Some(SystemTime::now());
                        }
                    } else {
                        error!("No image data available for cropped wallpaper");
                        self.wallpaper_status = Some("✗ No image data available for cropped wallpaper".to_string());
                        self.wallpaper_start_time = Some(SystemTime::now());
                    }
                }
                
                // show copyright button and open copyright_link on click
                if !main_image.copyright.is_empty() && !main_image.copyright_link.is_empty() {
                    if ui.button("More Info").clicked() {
                        match Self::resolve_url(&main_image.copyright_link) {
                            Some(copyright_url) => {
                                info!("Opening copyright URL: {}", copyright_url);
                                if let Err(e) = webbrowser::open(&copyright_url) {
                                    error!("Failed to open copyright URL: {}", e);
                                    self.wallpaper_status = Some(format!("✗ Failed to open copyright URL: {}", e));
                                    self.wallpaper_start_time = Some(SystemTime::now());
                                } else {
                                    self.wallpaper_status = Some("✓ Opened copyright URL".to_string());
                                    self.wallpaper_start_time = Some(SystemTime::now());
                                }
                            }
                            None => {
                                error!("Invalid copyright URL, cannot open: {}", main_image.copyright_link);
                                self.wallpaper_status = Some("✗ Invalid copyright URL".to_string());
                                self.wallpaper_start_time = Some(SystemTime::now());
                            }
                        }
                    }
                }
            });
            
            // Update screen ratio before rendering
            self.update_screen_ratio(ui);
            
            // Display the main panel image with square shape overlay
            if let Some(image) = &main_image.image {
                // Only log when the main panel image changes
                let image_url = &main_image.full_url;
                if self.current_main_image_url.as_ref() != Some(image_url) {
                    info!("Displaying main panel image with square overlay: {}", main_image.title);
                    self.current_main_image_url = Some(image_url.clone());
                }
                
                // Display the background image first
                let image_widget = image.clone().max_width(ui.available_width());
                let image_response = ui.add(image_widget);
                
                // Now overlay the square shape on top  
                let overlay_rect = image_response.rect;
                
                // Store the image display rect for coordinate transformation
                self.image_display_rect = Some(overlay_rect);
                
                // Reset rectangle size for new image if needed
                if self.reset_rectangle_for_new_image {
                    let (actual_screen_width, actual_screen_height) = self.get_actual_screen_size();
                    let actual_screen_size = Vec2::new(actual_screen_width, actual_screen_height);
                    self.initialize_rectangle_for_image(overlay_rect, actual_screen_size);
                    self.reset_rectangle_for_new_image = false;
                }
                
                ui.allocate_new_ui(
                    egui::UiBuilder::new().max_rect(overlay_rect),
                    |ui| {
                        self.render_square_shape(ui, overlay_rect);
                    }
                );
                
            } else {
                // Main panel image is None - could show a placeholder or instructions
            }
        } else if let Some(_selected_image) = &self.selected_carousel_image {
            ui.separator();
            ui.label("Loading high resolution image...");
            ui.spinner();
        }

        // Handle wallpaper status display
        if let Some(status) = &self.wallpaper_status {
            if let Some(start_time) = self.wallpaper_start_time {
                let elapsed = SystemTime::now().duration_since(start_time).unwrap_or_default();
                
                if elapsed.as_secs() < 10 {
                    // Show status for up to 10 seconds
                    ui.horizontal(|ui| {
                        if status.contains("started") {
                            ui.spinner();
                            ui.colored_label(egui::Color32::BLUE, status);
                        } else if status.contains("success") {
                            ui.colored_label(egui::Color32::GREEN, status);
                        } else {
                            ui.colored_label(ui.visuals().error_fg_color, status);
                        }
                    });
                    // Keep requesting repaints to update the spinner and check elapsed time
                    ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
                } else {
                    // Clear status after 10 seconds
                    self.wallpaper_status = None;
                    self.wallpaper_start_time = None;
                }
            }
        }
    }
}

fn ui_url(
    ui: &mut egui::Ui, 
    _url: &mut String, 
    carousel_images: &mut Vec<CarouselImage>, 
    carousel_promises: &mut Vec<Promise<ehttp::Result<CarouselImage>>>, 
    selected_carousel_image: &mut Option<CarouselImage>, 
    main_panel_image: &mut Option<CarouselImage>, 
    main_panel_promise: &mut Option<Promise<ehttp::Result<CarouselImage>>>, 
    image_cache: &mut HashMap<String, CarouselImage>, 
    bing_api_promise: &mut Option<Promise<Result<Vec<BingImage>, String>>>,
    config: &mut Option<Config>,
    market_code_index: &mut usize,
    infinite_scroll_page_index: &mut usize,
    current_market_codes: &mut Vec<String>,
    scroll_position: &mut f32,
    loading_more: &mut bool,
    reset_rectangle_for_new_image: &mut bool,
    current_main_image_url: &mut Option<String>,
    seen_image_names: &mut std::collections::HashSet<String>,
    wallpaper_status: &mut Option<String>,
    wallpaper_start_time: &mut Option<SystemTime>,
    showing_historical: &mut bool,
    market_exhausted: &mut bool,
    market_code_timestamps: &mut std::collections::HashMap<String, i64>,
) -> bool {
    let trigger_fetch = false;
    #[cfg(target_os = "android")]
    ui.add_space(40.0);

    // top panel image carousel
    egui::TopBottomPanel::top("top_panel")
    .min_height(100.0)
    .show_inside(ui, |ui| {
        
        ui.horizontal(|ui| {
            ui.label("Bingtray Wallpapers");
            
            // Add About button after title
            if ui.button("About").clicked() {
                if let Err(e) = webbrowser::open("https://bingtray.pages.dev") {
                    error!("Failed to open About URL: {}", e);
                    *wallpaper_status = Some(format!("✗ Failed to open About page: {}", e));
                    *wallpaper_start_time = Some(SystemTime::now());
                } else {
                    *wallpaper_status = Some("✓ Opened About page".to_string());
                    *wallpaper_start_time = Some(SystemTime::now());
                }
            }
        });
        ui.add_space(5.0);
        ui.separator();
        
        if !carousel_images.is_empty() {
            let loaded_count = carousel_images.iter().filter(|img| img.image.is_some()).count();
            
            // Determine if we're showing market data or historical data
            if *showing_historical {
                // We're showing historical data
                ui.horizontal(|ui| {
                    ui.label(format!("📜 Loaded {}/{} images from historical data successfully", loaded_count, carousel_images.len()));
                    
                    // Add reset button to go back to fresh market codes
                    if ui.button("🔄 Reset to Fresh Markets").clicked() {
                        info!("Resetting to fresh market codes mode");
                        *showing_historical = false;
                        *market_exhausted = false;
                        *market_code_index = 0;
                        *infinite_scroll_page_index = 0;
                        
                        // Clear carousel images to start fresh
                        carousel_images.clear();
                        carousel_promises.clear();
                        *selected_carousel_image = None;
                        *main_panel_image = None;
                        *main_panel_promise = None;
                        seen_image_names.clear();
                        
                        // Clear timestamps to force fresh downloads
                        market_code_timestamps.clear();
                        if let Some(cfg) = config.as_ref() {
                            if let Err(e) = save_market_codes(cfg, market_code_timestamps) {
                                error!("Failed to clear market code timestamps: {}", e);
                            }
                        }
                        
                        *wallpaper_status = Some("✓ Reset to fresh markets mode".to_string());
                        *wallpaper_start_time = Some(SystemTime::now());
                    }
                });
            } else if !current_market_codes.is_empty() {
                // We have market codes available, show market code info
                let current_market_index = (*market_code_index % current_market_codes.len()) + 1; // 1-based for display
                let total_market_codes = current_market_codes.len();
                let current_market_code = &current_market_codes[(*market_code_index) % current_market_codes.len()];
                
                ui.label(format!(
                    "Loaded {}/{} images from market {}/{} ({}) successfully", 
                    loaded_count, 
                    carousel_images.len(),
                    current_market_index,
                    total_market_codes,
                    current_market_code
                ));
            } else {
                // No market codes, showing historical data
                ui.label(format!("Loaded {}/{} images from historical data successfully", loaded_count, carousel_images.len()));
            }
            // Simple horizontal scroll area
            let scroll_response = egui::ScrollArea::horizontal()
                .auto_shrink(false)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Display all carousel images
                        for (i, carousel_image) in carousel_images.iter().enumerate() {
                            ui.vertical(|ui| {
                                if let Some(image) = &carousel_image.image {
                                    // Display the image
                                    let mut sized_image = image.clone();
                                    sized_image = sized_image.fit_to_exact_size(egui::Vec2::new(120.0, 80.0));
                                    let image_button = egui::ImageButton::new(sized_image);
                                    let response = ui.add(image_button);
                                    
                                    if response.clicked() {
                                        info!("Clicked on image {}: {}", i, carousel_image.title);
                                        *selected_carousel_image = Some(carousel_image.clone());
                                        
                                        // Check if we already have this high-res image cached
                                        if let Some(cached_image) = image_cache.get(&carousel_image.full_url) {
                                            info!("Using cached high-res image for: {}", carousel_image.title);
                                            *main_panel_image = Some(cached_image.clone());
                                            *reset_rectangle_for_new_image = true;
                                        } else {
                                            // Fetch full resolution image for main panel
                                            *main_panel_promise = None;
                                            let ctx = ui.ctx().clone();
                                            let full_url = carousel_image.full_url.clone();
                                            let carousel_image_clone = carousel_image.clone();
                                            
                                            info!("Fetching full resolution image for main panel: {}", full_url);
                                            
                                            let (sender, promise) = Promise::new();
                                            let request = ehttp::Request::get(&full_url);
                                            
                                            ehttp::fetch(request, move |response| {
                                                ctx.request_repaint();
                                                let result = response.map(|response| {
                                                    info!("Received full image response: status={}, size={} bytes", response.status, response.bytes.len());
                                                    
                                                    if response.status != 200 {
                                                        // Default fallback - return image with no data
                                                        error!("Failed to fetch full image: status={}", response.status);
                                                        return CarouselImage {
                                                            title: carousel_image_clone.title.clone(),
                                                            copyright: carousel_image_clone.copyright.clone(),
                                                            copyright_link: carousel_image_clone.copyright_link.clone(),
                                                            thumbnail_url: carousel_image_clone.thumbnail_url.clone(),
                                                            full_url: carousel_image_clone.full_url.clone(),
                                                            image: None,
                                                            image_bytes: None,
                                                        };
                                                    }
                                                    
                                                    let image_bytes = response.bytes.to_vec();
                                                    ctx.include_bytes(response.url.clone(), response.bytes.clone());
                                                    ctx.request_repaint();
                                                    let image = Image::from_uri(response.url.clone());
                                                    
                                                    CarouselImage {
                                                        title: carousel_image_clone.title.clone(),
                                                        copyright: carousel_image_clone.copyright.clone(),
                                                        copyright_link: carousel_image_clone.copyright_link.clone(),
                                                        thumbnail_url: carousel_image_clone.thumbnail_url.clone(),
                                                        full_url: carousel_image_clone.full_url.clone(),
                                                        image: Some(image),
                                                        image_bytes: Some(image_bytes),
                                                    }
                                                });
                                                sender.send(result);
                                            });
                                            
                                            *main_panel_promise = Some(promise);
                                        }
                                    }
                                } else {
                                    // Show placeholder while loading
                                    let placeholder = ui.add_sized([120.0, 80.0], egui::Button::new("📷 Loading..."));
                                    if placeholder.clicked() {
                                        info!("Clicked on placeholder for image {}: {}", i, carousel_image.title);
                                        *selected_carousel_image = Some(carousel_image.clone());
                                    }
                                }
                                
                                // Show truncated title
                                let title = if carousel_image.title.chars().count() > 15 {
                                    format!("{}...", carousel_image.title.chars().take(15).collect::<String>())
                                } else {
                                    carousel_image.title.clone()
                                };
                                ui.add_sized([120.0, 20.0], egui::Label::new(title).truncate());
                            });
                            ui.add_space(5.0);
                        }
                    });
                });
            
            // Check scroll position and trigger infinite scroll at 80%
            let scroll_pos = scroll_response.state.offset;
            let available_width = ui.available_width();
            let content_width = carousel_images.len() as f32 * 125.0; // 120px + 5px spacing
            let max_scroll = (content_width - available_width).max(0.0);
            
            // Handle infinite scroll trigger
            let should_load_more = if max_scroll > 0.0 {
                // Normal case: content is scrollable
                let scroll_percentage = scroll_pos.x / max_scroll;
                scroll_percentage > 0.8
            } else {
                // Special case: screen is wider than content, trigger when we have fewer than expected images
                // Assume we want at least enough images to fill 80% more than the current screen width
                let expected_images_for_screen = ((available_width * 1.8) / 125.0).ceil() as usize;
                carousel_images.len() < expected_images_for_screen
            };
            
            if should_load_more && !*loading_more && carousel_images.len() > 0 {
                if max_scroll > 0.0 {
                    let scroll_percentage = scroll_pos.x / max_scroll;
                    info!("Scroll threshold reached: {:.1}% - Loading more images", scroll_percentage * 100.0);
                } else {
                    info!("Screen wider than content ({:.0}px vs {:.0}px) - Loading more images", available_width, content_width);
                }
                *loading_more = true;
                
                // Check if there's already a promise running - if so, don't create a new one
                if bing_api_promise.is_some() {
                    info!("API request already in progress, skipping infinite scroll load");
                    *loading_more = false;
                } else {
                
                    // Load more images like desktop version does (market codes + historical fallback)
                    if let Some(config) = config {
                    info!("Loading additional market codes for infinite scroll (from marketcodes.conf if available)");
                    let market_codes = match load_market_codes(config) {
                        Ok(codes) => {
                            if config.marketcodes_file.exists() {
                                info!("Using existing market codes from local file for infinite scroll");
                            } else {
                                info!("Fetched fresh market codes from internet for infinite scroll");
                            }
                            codes
                        }
                        Err(e) => {
                            warn!("Failed to load market codes for infinite scroll: {}, using defaults", e);
                            HashMap::new()
                        }
                    };
                    let old_codes = get_old_market_codes(&market_codes);
                    
                    // Check if we should switch to historical mode
                    if !*market_exhausted && !old_codes.is_empty() {
                        // Find next market code that hasn't been visited in 7 days
                        let mut found_fresh_market = false;
                        let mut attempts = 0;
                        let max_attempts = old_codes.len();
                        
                        while attempts < max_attempts && !found_fresh_market {
                            let market_code = &old_codes[*market_code_index % old_codes.len()];
                            
                            // Check if this market code was visited recently
                            let now = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64;
                            let seven_days_ago = now - (7 * 24 * 60 * 60);
                            
                            let is_recent = if let Some(&last_visit) = market_code_timestamps.get(market_code) {
                                last_visit > seven_days_ago
                            } else {
                                false
                            };
                            
                            if !is_recent {
                                // This market code is fresh, use it
                                info!("Loading fresh images for market code: {} (not visited in 7+ days)", market_code);
                                
                                // Update timestamp for this market code
                                market_code_timestamps.insert(market_code.clone(), now);
                                
                                // Save the updated timestamps
                                if let Err(e) = save_market_codes(config, market_code_timestamps) {
                                    error!("Failed to save market code timestamps: {}", e);
                                }
                                
                                *market_code_index += 1;
                                *infinite_scroll_page_index += 1;
                                
                                let ctx = ui.ctx().clone();
                                let (sender, promise) = Promise::new();
                                let market_code_for_thread = market_code.clone();
                                
                                std::thread::spawn(move || {
                                    info!("Starting API call for fresh market: {}", market_code_for_thread);
                                    let result = get_bing_images(&market_code_for_thread)
                                        .map_err(|e| format!("Error fetching Bing images: {}", e));
                                    sender.send(result);
                                    ctx.request_repaint();
                                });
                                
                                *bing_api_promise = Some(promise);
                                found_fresh_market = true;
                            } else {
                                info!("Market code {} was visited recently (within 7 days), skipping", market_code);
                                *market_code_index += 1;
                                attempts += 1;
                            }
                        }
                        
                        if !found_fresh_market {
                            info!("All market codes visited within 7 days, switching to historical mode");
                            *market_exhausted = true;
                            *showing_historical = true;
                        }
                    }
                    
                    // If market codes are exhausted or we're in historical mode, load historical images
                    if *market_exhausted || *showing_historical || old_codes.is_empty() {
                        info!("Loading historical images (market_exhausted: {}, showing_historical: {}, old_codes empty: {})", 
                              *market_exhausted, *showing_historical, old_codes.is_empty());
                        
                        *showing_historical = true;
                        
                        let config_clone = config.clone();
                        let ctx = ui.ctx().clone();
                        let (sender, promise) = Promise::new();
                        
                        std::thread::spawn(move || {
                            // First check if we need to do initial historical data download
                            let result = if !config_clone.historical_metadata_file.exists() {
                                info!("No historical metadata found, downloading initial historical data");
                                download_historical_data(&config_clone, 0)
                                    .map_err(|e| format!("Error downloading initial historical data: {}", e))
                                    .map(|historical_images| {
                                        // Convert HistoricalImage to BingImage
                                        let bing_images: Vec<BingImage> = historical_images.iter().map(|h| BingImage {
                                            url: h.url.clone(),
                                            title: h.title.clone(),
                                            copyright: Some(h.copyright.clone()),
                                            copyrightlink: Some(h.copyrightlink.clone()),
                                        }).collect();
                                        bing_images
                                    })
                            } else {
                                info!("Historical metadata exists, getting next page");
                                get_next_historical_page(&config_clone, true)
                                    .map_err(|e| format!("Error fetching historical images: {}", e))
                                    .and_then(|opt| match opt {
                                        Some(historical_images) => {
                                            // Convert HistoricalImage to BingImage
                                            let bing_images: Vec<BingImage> = historical_images.iter().map(|h| BingImage {
                                                url: h.url.clone(),
                                                title: h.title.clone(),
                                                copyright: Some(h.copyright.clone()),
                                                copyrightlink: Some(h.copyrightlink.clone()),
                                            }).collect();
                                            Ok(bing_images)
                                        }
                                        None => Err("No more historical data available".to_string())
                                    })
                            };
                            sender.send(result);
                            ctx.request_repaint();
                        });
                        
                        *bing_api_promise = Some(promise);
                    }
                } else {
                    *loading_more = false;
                }
                }
            }
        } else {
            ui.label("Welcome to Bingtray! Click 'Fetch Bing Daily Image' to load wallpapers.");
            ui.add_space(10.0);
            
            // Show a placeholder card to demonstrate the UI works even without internet
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label("📱 Bingtray Mobile");
                    ui.separator();
                    ui.label("Features:");
                    ui.label("• Download Bing daily wallpapers");
                    ui.label("• Crop and set wallpapers");
                    ui.add_space(5.0);
                    ui.small("Internet connection required for fetching new images.");
                });
            });
        }
    });
    
    ui.horizontal(|ui| {
        if ui.button("Exit").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        
        // Check if there's an active API request
        let is_fetching = bing_api_promise.is_some();
        
        // Create button with appropriate text and state
        let fetch_button = if is_fetching {
            egui::Button::new("⏳ Fetching...").fill(egui::Color32::from_gray(128))
        } else {
            egui::Button::new("Fetch Bing Daily Image")
        };
        
        let fetch_response = ui.add_enabled(!is_fetching, fetch_button);
        
        if fetch_response.clicked() {
            info!("=== Fetch Bing Daily Image button clicked ===");
            // Clear existing images and promises
            carousel_images.clear();
            carousel_promises.clear();
            image_cache.clear();
            *main_panel_image = None;
            *main_panel_promise = None;
            *selected_carousel_image = None;
            *scroll_position = 0.0;
            *loading_more = false;
            *market_code_index = 0;
            *current_main_image_url = None;
            seen_image_names.clear();
            
            // Reset historical mode flags
            *showing_historical = false;
            *market_exhausted = false;

            // Use bingtray-core to fetch images
            if let Some(_config) = config {
                info!("Config is available");
                
                // Ensure we have at least one market code to work with
                if current_market_codes.is_empty() {
                    current_market_codes.push("en-US".to_string());
                    warn!("No market codes available, added en-US as fallback");
                }
                
                let market_code = &current_market_codes[0];
                info!("Fetching Bing images using bingtray-core for market: {}", market_code);
                info!("Available market codes: {:?}", current_market_codes);
                
                // Update timestamp for this market code
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                market_code_timestamps.insert(market_code.clone(), now);
                
                // Save the updated timestamps
                if let Err(e) = save_market_codes(_config, market_code_timestamps) {
                    error!("Failed to save market code timestamps: {}", e);
                }
                
                let ctx = ui.ctx().clone();
                let (sender, promise) = Promise::new();
                let market_code = market_code.clone();
                
                std::thread::spawn(move || {
                    info!("Starting API call in background thread for market: {}", market_code);
                    
                    // Attempt network request without blocking UI
                    let result = match get_bing_images(&market_code) {
                        Ok(images) => {
                            info!("Successfully fetched {} images from network", images.len());
                            Ok(images)
                        }
                        Err(e) => {
                            error!("Network request failed: {}", e);
                            // Don't block UI - just provide user feedback
                            Err(format!("Network unavailable: {}. You can still use the app - fetch will retry when connection is restored.", e))
                        }
                    };
                    
                    // Send result to UI thread (success or failure)
                    sender.send(result);
                    
                    // Request repaint to process the result
                    ctx.request_repaint();
                });
                
                *bing_api_promise = Some(promise);
                *market_code_index = 1; // Next market code for infinite scroll
                info!("Bing API promise created and stored");
                
                // Force immediate UI update to show button state change
                ui.ctx().request_repaint();
                // Also request frequent updates while loading
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
            } else {
                warn!("No config available, creating fallback API call");
                // Even without config, try to fetch images with a default market code
                let ctx = ui.ctx().clone();
                let (sender, promise) = Promise::new();
                
                std::thread::spawn(move || {
                    info!("Starting fallback API call for en-US");
                    let result = match get_bing_images("en-US") {
                        Ok(images) => {
                            info!("Successfully fetched {} images from fallback network call", images.len());
                            Ok(images)
                        }
                        Err(e) => {
                            error!("Fallback network request failed: {}", e);
                            // Don't block UI - just provide user feedback
                            Err(format!("Network unavailable: {}. You can still use the app - fetch will retry when connection is restored.", e))
                        }
                    };
                    sender.send(result);
                    ctx.request_repaint();
                });
                
                *bing_api_promise = Some(promise);
                current_market_codes.push("en-US".to_string());
                *market_code_index = 1;
                info!("Fallback Bing API promise created and stored");
                
                // Force immediate UI update to show button state change
                ui.ctx().request_repaint();
                // Also request frequent updates while loading
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
            }
        } 
        
    });

    trigger_fetch
}

fn ui_resource(ui: &mut egui::Ui, resource: &Resource, wallpaper_status: &mut Option<String>, wallpaper_start_time: &mut Option<SystemTime>) {
    let Resource {
        response,
        text,
        image,
    } = resource;

    ui.monospace(format!("url:          {}", response.url));
    ui.monospace(format!(
        "status:       {} ({})",
        response.status, response.status_text
    ));
    ui.monospace(format!(
        "content-type: {}",
        response.content_type().unwrap_or_default()
    ));
    ui.monospace(format!(
        "size:         {:.1} kB",
        response.bytes.len() as f32 / 1000.0
    ));

    ui.separator();

    // show response body
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            
            ui.separator();

            if let Some(text) = &text {
                let tooltip = "Click to copy the response body";
                if ui.button("📋").on_hover_text(tooltip).clicked() {
                    ui.ctx().copy_text(text.clone());
                }
                ui.separator();
            }

            if let Some(image) = image {
                ui.add(image.clone());
                
                // Add wallpaper button for images
                if ui.button("Set this Wallpaper").clicked() {
                    #[cfg(target_os = "android")]
                    {
                        if !response.bytes.is_empty() {
                            let image_data = response.bytes.clone();
                            info!("Starting wallpaper setting with {} bytes", image_data.len());
                            
                            // Start wallpaper setting in background thread using bytes directly
                            std::thread::spawn(move || {
                                log::info!("BingtrayApp: Starting wallpaper setting from bytes in background thread");
                                match crate::set_wallpaper_from_bytes(&image_data) {
                                    Ok(true) => {
                                        log::info!("BingtrayApp: Wallpaper setting from bytes completed successfully");
                                    }
                                    Ok(false) => {
                                        log::error!("BingtrayApp: Wallpaper setting from bytes failed");
                                    }
                                    Err(e) => {
                                        log::error!("BingtrayApp: Error during wallpaper setting from bytes: {}", e);
                                    }
                                }
                            });
                            
                            // Immediately update UI status without waiting
                            *wallpaper_status = Some("✓ Wallpaper setting started (using bytes)".to_string());
                            *wallpaper_start_time = Some(SystemTime::now());
                            ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
                            log::info!("BingtrayApp: Finished processing wallpaper setting request from bytes");
                        } else {
                            error!("No image data available");
                            *wallpaper_status = Some("✗ No image data available".to_string());
                            *wallpaper_start_time = Some(SystemTime::now());
                        }
                    }
                    #[cfg(not(target_os = "android"))]
                    {
                        warn!("Wallpaper setting is only available on Android");
                        *wallpaper_status = Some("⚠ Wallpaper setting only available on Android".to_string());
                        *wallpaper_start_time = Some(SystemTime::now());
                    }
                }
            } else if let Some(text) = &text {
                ui.add(egui::Label::new(text).selectable(true));
            } else {
                ui.monospace("[binary]");
            }
        });
}

impl BingtrayApp {
    fn update_square_corners(&mut self) {
        // Get actual screen dimensions for rectangle calculation
        let (screen_width, screen_height) = self.get_actual_screen_size();
        info!("Updating square corners with screen: {}x{}, center: {:?}, factor: {}", 
              screen_width, screen_height, self.square_center, self.square_size_factor);
        
        // Ensure screen ratio is valid
        if self.screen_ratio <= 0.0 {
            self.screen_ratio = screen_width / screen_height; // Use actual screen ratio
        }
        
        // Ensure size factor is valid
        if self.square_size_factor <= 0.0 {
            self.square_size_factor = 0.3; // Default to 30% of screen size
        }
        
        // Create a rectangle that matches screen aspect ratio and actual dimensions
        // Size factor now represents the percentage of screen size to use
        let rect_width = screen_width * self.square_size_factor;
        let _rect_height = screen_height * self.square_size_factor;
        
        // Ensure rectangle maintains screen aspect ratio
        let corrected_height = rect_width / self.screen_ratio;
        let final_width = rect_width;
        let final_height = corrected_height;
        
        let half_width = final_width / 2.0;
        let half_height = final_height / 2.0;
        
        self.square_corners = [
            pos2(self.square_center.x - half_width, self.square_center.y - half_height), // Top-left
            pos2(self.square_center.x + half_width, self.square_center.y - half_height), // Top-right
            pos2(self.square_center.x + half_width, self.square_center.y + half_height), // Bottom-right
            pos2(self.square_center.x - half_width, self.square_center.y + half_height), // Bottom-left
        ];
        info!("Updated square corners: {:?}", self.square_corners);
    }
    
    fn update_screen_ratio(&mut self, _ui: &egui::Ui) {
        // Get actual screen ratio from device screen dimensions
        let (screen_width, screen_height) = self.get_actual_screen_size();
        let new_screen_ratio = if screen_height > 0.0 {
            (screen_width / screen_height).max(0.1) // Ensure minimum ratio
        } else {
            16.0 / 9.0 // Fallback ratio
        };
        
        if (new_screen_ratio - self.screen_ratio).abs() > 0.01 {
            info!("Updating screen ratio from {:.3} to {:.3} ({}x{})", 
                  self.screen_ratio, new_screen_ratio, screen_width, screen_height);
            self.screen_ratio = new_screen_ratio;
            self.update_square_corners();
        }
    }
    
    fn render_square_shape(&mut self, ui: &mut egui::Ui, available_rect: Rect) -> egui::Response {
        let (response, painter) = ui.allocate_painter(available_rect.size(), Sense::hover());
        
        let to_screen = emath::RectTransform::from_to(
            Rect::from_min_size(Pos2::ZERO, response.rect.size()),
            response.rect,
        );
        
        // Handle mouse wheel for size adjustment when hovering over the image area
        if response.hovered() {
            let events = ui.ctx().input(|i| i.events.clone());
            for event in &events {
                if let egui::Event::MouseWheel { delta, .. } = event {
                    let zoom = delta.y as f32;
                    if zoom.abs() > 0.0001 {
                        let proposed_size_factor = (self.square_size_factor + zoom * 0.05).max(0.05).min(1.0);
                        
                        // Constrain size to keep rectangle within image bounds
                        // Use actual screen dimensions for rectangle calculation
                        let (screen_width, screen_height) = self.get_actual_screen_size();
                        let proposed_width = screen_width * proposed_size_factor;
                        let proposed_height = proposed_width / self.screen_ratio;
                        
                        // Calculate maximum size that fits within image bounds given current center
                        let _half_width = proposed_width / 2.0;
                        let _half_height = proposed_height / 2.0;
                        
                        // Calculate maximum allowed size based on current center position
                        let max_width_from_center = ((available_rect.width() - self.square_center.x).min(self.square_center.x) * 2.0).max(0.0);
                        let max_height_from_center = ((available_rect.height() - self.square_center.y).min(self.square_center.y) * 2.0).max(0.0);
                        
                        let max_size_by_width = if screen_width > 0.0 { max_width_from_center / screen_width } else { 1.0 };
                        let max_size_by_height = if screen_height > 0.0 { max_height_from_center / screen_height } else { 1.0 };
                        
                        let max_size_factor = max_size_by_width.min(max_size_by_height);
                        let constrained_size_factor = proposed_size_factor.min(max_size_factor);
                        
                        if constrained_size_factor != self.square_size_factor {
                            self.square_size_factor = constrained_size_factor;
                            self.update_square_corners();
                            ui.ctx().request_repaint();
                        }
                    }
                }
            }
        }
        
        // Make the entire square draggable (not just corners)
        let square_rect = Rect::from_two_pos(
            to_screen.transform_pos(self.square_corners[0]), 
            to_screen.transform_pos(self.square_corners[2])
        );
        let square_id = response.id.with("square_drag");
        let square_response = ui.interact(square_rect, square_id, Sense::drag());
        
        if square_response.dragged() && self.dragging_corner.is_none() {
            let delta = square_response.drag_delta();
            let new_center_screen = to_screen.transform_pos(self.square_center) + delta;
            let new_center = to_screen.inverse().transform_pos(new_center_screen);
            
            // Constrain center to keep rectangle within image bounds
            let (screen_width, _screen_height) = self.get_actual_screen_size();
            let rect_width = screen_width * self.square_size_factor;
            let rect_height = rect_width / self.screen_ratio;
            let half_width = rect_width / 2.0;
            let half_height = rect_height / 2.0;
            
            // Calculate valid bounds for the center to keep rectangle inside image
            let min_x = half_width;
            let max_x = (available_rect.width() - half_width).max(min_x);
            let min_y = half_height;
            let max_y = (available_rect.height() - half_height).max(min_y);
            
            let constrained_center = pos2(
                new_center.x.clamp(min_x, max_x),
                new_center.y.clamp(min_y, max_y),
            );
            
            self.square_center = constrained_center;
            self.update_square_corners();
            ui.ctx().request_repaint();
        }
        
        let corner_radius = 8.0;
        let mut corner_shapes = Vec::new();
        let mut needs_update = false;
        let mut new_size_factor = self.square_size_factor;
        
        // Get screen size once before the loop to avoid borrowing conflicts
        let (screen_width, screen_height) = self.get_actual_screen_size();
        
        // Handle corner dragging - similar to paint_bezier.rs
        for (i, corner) in self.square_corners.iter_mut().enumerate() {
            let corner_in_screen = to_screen.transform_pos(*corner);
            let corner_rect = Rect::from_center_size(corner_in_screen, Vec2::splat(2.0 * corner_radius));
            let corner_id = response.id.with(i);
            let corner_response = ui.interact(corner_rect, corner_id, Sense::drag());

            if corner_response.drag_started() {
                self.dragging_corner = Some(i);
            }
            
            if corner_response.dragged() && self.dragging_corner == Some(i) {
                // Calculate new size based on drag distance from center, maintaining screen ratio
                let center_in_screen = to_screen.transform_pos(self.square_center);
                let drag_pos = corner_response.interact_pointer_pos().unwrap_or(corner_in_screen);
                
                // Calculate the current distance and the base distance for the current size
                let current_distance = (drag_pos - center_in_screen).length();
                
                // Calculate base distance from current rectangle size to maintain proportional scaling
                let current_width = screen_width * self.square_size_factor;
                let current_height = current_width / self.screen_ratio;
                let current_base_distance = (current_width * current_width + current_height * current_height).sqrt() / 2.0;
                
                // Scale factor based on ratio of current to base distance
                let scale_ratio = if current_base_distance > 0.0 {
                    current_distance / current_base_distance
                } else {
                    1.0
                };
                
                let proposed_size_factor = (self.square_size_factor * scale_ratio).max(0.05).min(1.0);
                
                // Constrain size to keep rectangle within image bounds
                let proposed_width = screen_width * proposed_size_factor;
                let proposed_height = proposed_width / self.screen_ratio;
                
                // Calculate maximum size that fits within image bounds given current center
                let half_width = proposed_width / 2.0;
                let half_height = proposed_height / 2.0;
                
                // Check if proposed size would go outside bounds
                let _left_bound = self.square_center.x - half_width;
                let _right_bound = self.square_center.x + half_width;
                let _top_bound = self.square_center.y - half_height;
                let _bottom_bound = self.square_center.y + half_height;
                
                // Calculate maximum allowed size based on current center position
                let max_width_from_center = ((available_rect.width() - self.square_center.x).min(self.square_center.x) * 2.0).max(0.0);
                let max_height_from_center = ((available_rect.height() - self.square_center.y).min(self.square_center.y) * 2.0).max(0.0);
                
                let max_size_by_width = if screen_width > 0.0 { max_width_from_center / screen_width } else { 1.0 };
                let max_size_by_height = if screen_height > 0.0 { max_height_from_center / screen_height } else { 1.0 };
                
                let max_size_factor = max_size_by_width.min(max_size_by_height);
                new_size_factor = proposed_size_factor.min(max_size_factor);
                needs_update = true;
                ui.ctx().request_repaint();
            }
            
            if corner_response.drag_stopped() {
                self.dragging_corner = None;
            }

            let corner_in_screen = to_screen.transform_pos(*corner);
            let stroke = ui.style().interact(&corner_response).fg_stroke;

            // Create corner handle shape
            corner_shapes.push(Shape::circle_stroke(corner_in_screen, corner_radius, stroke));
        }
        
        // Apply updates after the loop to avoid borrowing conflicts
        if needs_update {
            self.square_size_factor = new_size_factor;
            
            // After resizing, ensure center is still within bounds
            let (screen_width, _screen_height) = self.get_actual_screen_size();
            let rect_width = screen_width * self.square_size_factor;
            let rect_height = rect_width / self.screen_ratio;
            let half_width = rect_width / 2.0;
            let half_height = rect_height / 2.0;
            
            // Recalculate bounds and constrain center if necessary
            let min_x = half_width;
            let max_x = (available_rect.width() - half_width).max(min_x);
            let min_y = half_height;
            let max_y = (available_rect.height() - half_height).max(min_y);
            
            self.square_center = pos2(
                self.square_center.x.clamp(min_x, max_x),
                self.square_center.y.clamp(min_y, max_y),
            );
            
            self.update_square_corners();
        }

        // Draw the rectangle maintaining screen ratio
        let corners_in_screen: Vec<Pos2> = self.square_corners
            .iter()
            .map(|p| to_screen.transform_pos(*p))
            .collect();

        // Create rectangle shape that maintains screen ratio
        if corners_in_screen.len() == 4 {
            let rect = Rect::from_two_pos(corners_in_screen[0], corners_in_screen[2]);
            let square_fill = egui::Color32::from_rgb(50, 100, 150).linear_multiply(0.3);
            let square_stroke = Stroke::new(2.0, egui::Color32::from_rgb(25, 200, 100));
            
            // Draw filled rectangle
            painter.add(Shape::rect_filled(rect, 2.0, square_fill));
            // Draw stroke rectangle
            painter.add(Shape::rect_stroke(rect, 2.0, square_stroke, StrokeKind::Outside));
        }

        // Draw corner handles on top
        painter.extend(corner_shapes);

        response
    }

    fn initialize_rectangle_for_image(&mut self, image_rect: Rect, screen_size: Vec2) {
        let image_width = image_rect.width();
        let image_height = image_rect.height();
        let screen_aspect_ratio = screen_size.x / screen_size.y;
        
        // Update the screen ratio to use actual screen dimensions
        self.screen_ratio = screen_aspect_ratio;
        
        // Determine if image is bigger than screen (considering display scale)
        let display_scale_factor = 0.8; // Allow rectangle to be 80% of screen size max
        let max_rect_width = screen_size.x * display_scale_factor;
        let max_rect_height = screen_size.y * display_scale_factor;
        
        let image_bigger_than_display = image_width > max_rect_width || image_height > max_rect_height;
        
        let (rect_width, rect_height) = if image_bigger_than_display {
            // Image is bigger than manageable screen area - use screen ratio and fit to display area
            info!("Image ({:.0}x{:.0}) is bigger than display area ({:.0}x{:.0}), using screen aspect ratio", 
                  image_width, image_height, max_rect_width, max_rect_height);
            
            // Fit rectangle to screen ratio within the image bounds
            let screen_ratio_width = max_rect_width.min(image_width);
            let screen_ratio_height = screen_ratio_width / screen_aspect_ratio;
            
            // If height would exceed image bounds, recalculate from height
            if screen_ratio_height > image_height {
                let height = image_height.min(max_rect_height);
                let width = height * screen_aspect_ratio;
                (width, height)
            } else {
                (screen_ratio_width, screen_ratio_height)
            }
        } else {
            // Image is smaller than display area - use full image size with screen ratio constraint
            info!("Image ({:.0}x{:.0}) fits in display area ({:.0}x{:.0}), using image size with screen aspect ratio", 
                  image_width, image_height, max_rect_width, max_rect_height);
            
            // Try to use image width first with screen aspect ratio
            let width_based_height = image_width / screen_aspect_ratio;
            if width_based_height <= image_height {
                (image_width, width_based_height)
            } else {
                // Use image height and calculate width with screen aspect ratio
                let height_based_width = image_height * screen_aspect_ratio;
                (height_based_width, image_height)
            }
        };
        
        // Center the rectangle in the image (use coordinates relative to image rect, not screen)
        let center_x = image_width / 2.0;
        let center_y = image_height / 2.0;
        
        // Update the rectangle center relative to the image area
        self.square_center = pos2(center_x, center_y);
        
        // Update size factor based on new dimensions (relative to actual screen width)
        let (screen_width, _screen_height) = self.get_actual_screen_size();
        self.square_size_factor = rect_width / screen_width;
        
        info!("Initialized rectangle: center=({:.1},{:.1}), size=({:.1}x{:.1}), factor={:.2}",
              self.square_center.x, self.square_center.y, rect_width, rect_height, self.square_size_factor);
        
        self.update_square_corners();
    }

    fn get_initial_screen_size() -> (f32, f32) {
        #[cfg(target_os = "android")]
        {
            match get_screen_size() {
                Ok((width, height)) => (width as f32, height as f32),
                Err(_) => (1080.0, 1920.0), // Default mobile resolution
            }
        }
        
        #[cfg(not(target_os = "android"))]
        {
            match screen_size::get_primary_screen_size() {
                Ok((width, height)) => (width as f32, height as f32),
                Err(_) => (1920.0, 1080.0), // Default desktop resolution
            }
        }
    }

    fn get_actual_screen_size(&mut self) -> (f32, f32) {
        // Return cached value if available
        if let Some(cached) = self.cached_screen_size {
            return cached;
        }
        
        // If screen size detection previously failed, don't retry
        if self.screen_size_failed {
            return (1080.0, 1920.0); // Default mobile resolution
        }
        
        #[cfg(target_os = "android")]
        {
            match get_screen_size() {
                Ok((width, height)) => {
                    let result = (width as f32, height as f32);
                    self.cached_screen_size = Some(result);
                    result
                }
                Err(_e) => {
                    // Mark as failed and use default without logging repeatedly
                    self.screen_size_failed = true;
                    let result = (1080.0, 1920.0); // Default mobile resolution
                    self.cached_screen_size = Some(result);
                    result
                }
            }
        }
        
        #[cfg(not(target_os = "android"))]
        {
            match screen_size::get_primary_screen_size() {
                Ok((width, height)) => {
                    let result = (width as f32, height as f32);
                    self.cached_screen_size = Some(result);
                    result
                }
                Err(_e) => {
                    // Mark as failed and use default without logging repeatedly
                    self.screen_size_failed = true;
                    let result = (1920.0, 1080.0); // Default desktop resolution
                    self.cached_screen_size = Some(result);
                    result
                }
            }
        }
    }

    fn has_next_wallpaper_available(&self) -> bool {
        if let Some(config) = &self.config {
            // Check if there are available market codes to download from (preferably from local file)
            let market_codes = match load_market_codes(config) {
                Ok(codes) => codes,
                Err(_) => HashMap::new(), // Fallback to empty map on error
            };
            let old_codes = get_old_market_codes(&market_codes);
            if !old_codes.is_empty() {
                return true;
            }
            
            // Check if historical data is available when no market codes are available
            if let Ok((current_page, total_pages)) = bingtray_core::get_historical_page_info(config) {
                // If no historical metadata file exists yet, we can still download initial historical data
                if current_page == 0 && total_pages == 0 {
                    return true; // We can download initial historical data
                }
                return current_page < total_pages;
            } else {
                // If there's an error loading historical page info, we can still try to download initial data
                return true;
            }
        }
        
        false
    }
    
    /// Validates and resolves a potentially relative URL to an absolute URL
    /// Returns None if the URL is malformed or invalid
    fn resolve_url(url: &str) -> Option<String> {
        // Check for empty or whitespace-only URLs
        let trimmed = url.trim();
        if trimmed.is_empty() {
            error!("Empty URL provided");
            return None;
        }
        
        // Check for obviously malformed URLs
        if trimmed.contains('\n') || trimmed.contains('\r') || trimmed.contains('\t') {
            error!("Malformed URL contains control characters: {}", trimmed);
            return None;
        }
        
        let resolved_url = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            // Already an absolute URL
            trimmed.to_string()
        } else if trimmed.starts_with("//") {
            // Protocol-relative URL, add https:
            format!("https:{}", trimmed)
        } else if trimmed.starts_with("/") {
            // Absolute path, add Bing domain
            format!("https://www.bing.com{}", trimmed)
        } else {
            // Log suspicious relative paths and reject them
            error!("Suspicious relative URL that may be malformed: {}", trimmed);
            return None;
        };
        
        // Basic validation of the resolved URL
        if resolved_url.len() > 2048 {
            error!("URL too long (>2048 chars): {}", resolved_url);
            return None;
        }
        
        // Check for basic URL structure
        if !resolved_url.contains("://") {
            error!("Malformed URL missing protocol: {}", resolved_url);
            return None;
        }
        
        Some(resolved_url)
    }

    // Check if a market code was visited within the last 7 days
    fn is_market_code_recent(&self, market_code: &str) -> bool {
        if let Some(&last_visit) = self.market_code_timestamps.get(market_code) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let seven_days_ago = now - (7 * 24 * 60 * 60); // 7 days in seconds
            last_visit > seven_days_ago
        } else {
            false
        }
    }

    // Update the timestamp for a market code
    fn update_market_code_timestamp(&mut self, market_code: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.market_code_timestamps.insert(market_code.to_string(), now);
    }

    // Load market code timestamps from bingtray-core
    fn load_market_code_timestamps(&mut self) {
        if let Some(ref config) = self.config {
            match load_market_codes(config) {
                Ok(market_codes) => {
                    self.market_code_timestamps = market_codes;
                    info!("Loaded {} market code timestamps", self.market_code_timestamps.len());
                }
                Err(e) => {
                    warn!("Failed to load market code timestamps: {}", e);
                }
            }
        }
    }

    // Save market code timestamps to bingtray-core
    fn save_market_code_timestamps(&self) {
        if let Some(ref config) = self.config {
            if let Err(e) = save_market_codes(config, &self.market_code_timestamps) {
                error!("Failed to save market code timestamps: {}", e);
            } else {
                info!("Saved {} market code timestamps", self.market_code_timestamps.len());
            }
        }
    }
}
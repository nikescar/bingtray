use egui::Image;
use poll_promise::Promise;
use egui::{Vec2b, Pos2, pos2, Rect, Sense, Shape, Stroke, Vec2, emath};
use egui::epaint::StrokeKind;
use log::{trace, warn, info, error};
use std::time::SystemTime;
use std::collections::HashMap;
use serde::Deserialize;
use bingtray_core::{Config, BingImage, get_bing_images, get_next_historical_page, load_market_codes, get_old_market_codes};
use chrono::Utc;

#[cfg(target_os = "android")]
use crate::android_screensize::get_screen_size;

#[cfg(not(target_os = "android"))]
use screen_size;

#[derive(Deserialize, Debug)]
struct BingImageData {
    images: Vec<BingImageCompat>,
}

#[derive(Deserialize, Debug)]
struct BingImageCompat {
    #[serde(rename = "fullstartdate")]
    #[allow(dead_code)]
    full_start_date: String,
    url: String,
    copyright: String,
    #[serde(rename = "copyrightlink")]
    copyright_link: String,
    title: String,
    #[allow(dead_code)]
    quiz: String,
    #[allow(dead_code)]
    wp: bool,
    #[allow(dead_code)]
    hsh: String,
    #[allow(dead_code)]
    drk: i32,
    #[allow(dead_code)]
    top: i32,
    #[allow(dead_code)]
    bot: i32,
}

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
    colored_text: Option<ColoredText>,
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
                colored_text: None,
                image: Some(image),
            }
        } else {
            let text = response.text();
            let colored_text = text.and_then(|text| syntax_highlighting(ctx, &response, text));
            let text = text.map(|text| text.to_owned());

            Self {
                response,
                text,
                colored_text,
                image: None,
            }
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct HttpApp {
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
}

impl Default for HttpApp {
    fn default() -> Self {
        let config = Config::new().ok();
        info!("Config creation result: {:?}", config.is_some());
        
        let current_market_codes = if let Some(ref config) = config {
            info!("Attempting to load market codes...");
            match load_market_codes(config) {
                Ok(codes) => {
                    info!("Successfully loaded {} market codes", codes.len());
                    let old_codes = get_old_market_codes(&codes);
                    info!("Filtered to {} old market codes: {:?}", old_codes.len(), old_codes);
                    
                    // If no old codes available, use some recent ones or fallback
                    if old_codes.is_empty() {
                        warn!("No old market codes available, using first few available codes");
                        codes.keys().take(5).cloned().collect::<Vec<_>>()
                    } else {
                        old_codes
                    }
                }
                Err(e) => {
                    warn!("Failed to load market codes: {}, using fallback", e);
                    vec!["en-US".to_string()]
                }
            }
        } else {
            warn!("No config available, using fallback market codes");
            vec!["en-US".to_string()]
        };
        
        info!("Final market codes: {:?}", current_market_codes);
        
        // Get actual screen size for rectangle calculation
        let (screen_width, screen_height) = Self::get_actual_screen_size();
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
            title: "🗖 Window Options".to_owned(),
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
        }
    }
}

impl crate::Demo for HttpApp {
    fn name(&self) -> &'static str {
        "🌐 HTTP"
    }

    fn show(&mut self, ctx: &egui::Context, _open: &mut bool) {
        use crate::View as _;

        // please get screen_size from screen_size crate in case of windows,linux,maxos
        // in case of android, use get_screen_size from ../android_screensize.rs
        #[cfg(target_os = "android")]
        let screen_size = get_screen_size().unwrap_or((1920, 1080)); // Default to 1920x1080 if error
        #[cfg(not(target_os = "android"))]
        let screen_size = screen_size::get_primary_screen_size().unwrap_or((1920, 1080)); // Default to 1920x1080 if error

        let mut window = egui::Window::new(&self.title)
            .default_width(screen_size.0 as f32)
            .default_height(screen_size.1 as f32)
            .id(egui::Id::new("demo_window_options")) // required since we change the title
            .resizable(self.resizable)
            .constrain(self.constrain)
            .collapsible(self.collapsible)
            .title_bar(self.title_bar)
            .scroll(self.scroll2);
        if self.anchored {
            window = window.anchor(self.anchor, self.anchor_offset);
        }
        window.show(ctx, |ui| self.ui(ui));
    }
}


impl crate::View for HttpApp {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let prev_url = self.url.clone();
        let trigger_fetch = ui_url(ui, &mut self.url, &mut self.carousel_images, &mut self.carousel_promises, &mut self.selected_carousel_image, &mut self.main_panel_image, &mut self.main_panel_promise, &mut self.image_cache, &mut self.bing_api_promise, &mut self.config, &mut self.market_code_index, &mut self.current_market_codes, &mut self.scroll_position, &mut self.loading_more, &mut self.reset_rectangle_for_new_image, &mut self.current_main_image_url);

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
        if let Some(bing_promise) = &self.bing_api_promise {
            trace!("Checking Bing API promise...");
            if let Some(result) = bing_promise.ready() {
                info!("=== Bing API promise completed ===");
                match result {
                    Ok(bing_images) => {
                        info!("Bing API data received with {} images", bing_images.len());
                        for (i, img) in bing_images.iter().enumerate() {
                            info!("Image {}: title='{}', url='{}'", i, img.title, img.url);
                        }
                        
                        // Process each image from the Bing API response
                        for bing_image in bing_images {
                            // Construct the full URLs - use the URL as-is from Bing API
                            let base_url = if bing_image.url.starts_with("http") {
                                bing_image.url.clone()
                            } else {
                                format!("https://bing.com{}", bing_image.url)
                            };
                            
                            // Add size parameters properly
                            let separator = if base_url.contains('?') { "&" } else { "?" };
                            let thumbnail_url = format!("{}{}w=320&h=240", base_url, separator);
                            let full_url = format!("{}{}w=1920&h=1080", base_url, separator);
                            
                            info!("Base URL: {}", base_url);
                            info!("Thumbnail URL: {}", thumbnail_url);
                            info!("Full URL: {}", full_url);
                            
                            // Extract better title from URL if original title is "Info" or generic
                            let display_title = if bing_image.title == "Info" || bing_image.title.is_empty() {
                                // Extract from URL like bingtray-core does
                                bing_image.url
                                    .split("th?id=")
                                    .nth(1)
                                    .and_then(|s| s.split('_').next())
                                    .map(|s| s.replace("OHR.", "").replace("_", " "))
                                    .unwrap_or_else(|| bing_image.title.clone())
                            } else {
                                bing_image.title.clone()
                            };
                            
                            let carousel_image = CarouselImage {
                                title: display_title,
                                copyright: bing_image.copyright.clone().unwrap_or_default(),
                                copyright_link: bing_image.copyrightlink.clone().unwrap_or_default(),
                                thumbnail_url: thumbnail_url.clone(),
                                full_url: full_url.clone(),
                                image: None,
                                image_bytes: None,
                            };
                            
                            self.carousel_images.push(carousel_image.clone());
                            
                            // Fetch the thumbnail image
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
                                        CarouselImage {
                                            title: carousel_image.title.clone(),
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
                        
                        // Clear the Bing API promise as it's completed
                        self.bing_api_promise = None;
                    }
                    Err(e) => {
                        error!("Failed to fetch Bing API data: {}", e);
                        self.bing_api_promise = None;
                    }
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
                            // Let's also try to match by title as a fallback
                            for existing_img in self.carousel_images.iter_mut() {
                                if existing_img.title == carousel_image.title {
                                    existing_img.image = carousel_image.image.clone();
                                    existing_img.image_bytes = carousel_image.image_bytes.clone();
                                    info!("Updated image in carousel by title for: {}", existing_img.title);
                                    break;
                                }
                            }
                        }
                        // Force a repaint when image is updated
                        ui.ctx().request_repaint();
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
                
                // show copyright button and open copyright_link on click
                if !main_image.copyright.is_empty() && !main_image.copyright_link.is_empty() {
                    if ui.button("More Info").clicked() {
                        let copyright_url = if main_image.copyright_link.starts_with("http") {
                            main_image.copyright_link.clone()
                        } else {
                            format!("https://bing.com{}", main_image.copyright_link)
                        };
                        
                        info!("Opening copyright URL: {}", copyright_url);
                        #[cfg(not(target_os = "android"))]
                        {
                            if let Err(e) = webbrowser::open(&copyright_url) {
                                error!("Failed to open copyright URL: {}", e);
                                self.wallpaper_status = Some(format!("✗ Failed to open copyright URL: {}", e));
                                self.wallpaper_start_time = Some(SystemTime::now());
                            } else {
                                self.wallpaper_status = Some("✓ Opened copyright URL".to_string());
                                self.wallpaper_start_time = Some(SystemTime::now());
                            }
                        }
                        #[cfg(target_os = "android")]
                        {
                            self.wallpaper_status = Some("✓ Copyright URL (webbrowser not available on Android)".to_string());
                            self.wallpaper_start_time = Some(SystemTime::now());
                        }
                    }
                }
                
                
                // Square shape controls
                let reset_clicked = ui.button("Reset Size").clicked();
                
                if reset_clicked {
                    // Reset to match screen size
                    let screen_rect = ui.ctx().screen_rect();
                    let screen_width = screen_rect.width();
                    let screen_height = screen_rect.height();
                    self.screen_ratio = screen_width / screen_height;
                    
                    // Set square size to match actual screen dimensions
                    self.square_size_factor = 1.0;
                    let _square_width = screen_width * 0.3; // 30% of screen width
                    
                    // Center the square
                    let center_x = screen_width / 2.0;
                    let center_y = screen_height / 2.0;
                    self.square_center = pos2(center_x, center_y);
                    
                    self.update_square_corners();
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
                
                // Reset rectangle size for new image if needed
                if self.reset_rectangle_for_new_image {
                    let (actual_screen_width, actual_screen_height) = Self::get_actual_screen_size();
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
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
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
    current_market_codes: &mut Vec<String>,
    scroll_position: &mut f32,
    loading_more: &mut bool,
    reset_rectangle_for_new_image: &mut bool,
    current_main_image_url: &mut Option<String>,
) -> bool {
    let trigger_fetch = false;
    #[cfg(target_os = "android")]
    ui.add_space(40.0);

    // top panel image carousel
    egui::TopBottomPanel::top("top_panel")
    .min_height(100.0)
    .show_inside(ui, |ui| {
        ui.label("Bingtray Wallpapers");
        ui.add_space(5.0);
        ui.separator();
        
        if !carousel_images.is_empty() {
            ui.label(format!("Loaded {} images", carousel_images.len()));
            // Create scroll area with proper scroll detection for infinite loading
            let scroll_area = egui::ScrollArea::horizontal()
                .auto_shrink(false)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Get actual scroll position from the UI
                        let available_width = ui.available_width();
                        let total_content_width = carousel_images.len() as f32 * 130.0; // 120px image + 10px spacing
                        let scroll_rect = ui.clip_rect();
                        let content_rect = ui.max_rect();
                        
                        // Calculate how much we've scrolled from the left
                        let scroll_offset = (content_rect.min.x - scroll_rect.min.x).max(0.0);
                        let max_scroll = (total_content_width - available_width).max(0.0);
                        let scroll_percentage = if max_scroll > 0.0 { scroll_offset / max_scroll } else { 0.0 };
                        
                        // Load more when scrolling reaches 80% from left side
                        if scroll_percentage > 0.8 && !*loading_more && carousel_images.len() > 0 {
                            info!("Scroll threshold reached: {:.1}% - Loading more images", scroll_percentage * 100.0);
                            *loading_more = true;
                            
                            // Check if more images are available using bingtray_core functions
                            let has_more = if let Some(config) = config {
                                // Check if there are available market codes to download from
                                let market_codes = load_market_codes(config).unwrap_or_default();
                                let old_codes = get_old_market_codes(&market_codes);
                                if !old_codes.is_empty() && *market_code_index < current_market_codes.len() {
                                    true
                                } else {
                                    // Check if historical data is available
                                    if let Ok((current_page, total_pages)) = bingtray_core::get_historical_page_info(config) {
                                        current_page < total_pages
                                    } else {
                                        false
                                    }
                                }
                            } else {
                                false
                            };
                            
                            if has_more {
                                // Load more images using proper bingtray-core logic
                                if let Some(config) = config {
                                    let mut market_codes = load_market_codes(config).unwrap_or_default();
                                    let old_codes = get_old_market_codes(&market_codes);
                                    
                                    if !old_codes.is_empty() && *market_code_index < current_market_codes.len() {
                                        // Load from market codes
                                        let market_code = &current_market_codes[*market_code_index];
                                        info!("Loading more images for market code: {}", market_code);
                                        
                                        let ctx = ui.ctx().clone();
                                        let (sender, promise) = Promise::new();
                                        let market_code_for_thread = market_code.clone();
                                        let market_code_for_update = market_code.clone();
                                        
                                        std::thread::spawn(move || {
                                            let result = get_bing_images(&market_code_for_thread)
                                                .map_err(|e| format!("Error fetching Bing images: {}", e));
                                            sender.send(result);
                                        });
                                        
                                        *bing_api_promise = Some(promise);
                                        *market_code_index += 1;
                                        
                                        // Update timestamp for this market code
                                        market_codes.insert(market_code_for_update, Utc::now().timestamp());
                                        let _ = bingtray_core::save_market_codes(config, &market_codes);
                                    } else {
                                        // Load historical data
                                        info!("No more market codes available, loading historical data");
                                        let ctx = ui.ctx().clone();
                                        let config_clone = config.clone();
                                        let (sender, promise) = Promise::new();
                                        
                                        std::thread::spawn(move || {
                                            let result = get_next_historical_page(&config_clone)
                                                .map_err(|e| format!("Error fetching historical data: {}", e))
                                                .and_then(|opt_images| {
                                                    opt_images.map(|historical_images| {
                                                    // Convert HistoricalImage to BingImage
                                                    historical_images.into_iter().map(|hist| BingImage {
                                                        url: hist.url,
                                                        title: hist.title,
                                                        copyright: Some(hist.copyright),
                                                        copyrightlink: Some(hist.copyrightlink),
                                                    }).collect()
                                                }).ok_or_else(|| "No more historical data available".to_string())
                                            });
                                            sender.send(result);
                                        });
                                        
                                        *bing_api_promise = Some(promise);
                                    }
                                }
                            } else {
                                info!("No more images available for infinite scroll");
                                *loading_more = false;
                            }
                        }
                        
                        for (i, carousel_image) in carousel_images.iter().enumerate() {
                            ui.vertical(|ui| {
                                trace!("Checking image {} - has image: {}", i, carousel_image.image.is_some());
                                if let Some(image) = &carousel_image.image {
                                    // Only log when image first becomes available, not on every frame
                                    // Try to display the image with explicit sizing
                                    let mut sized_image = image.clone();
                                    sized_image = sized_image.fit_to_exact_size(egui::Vec2::new(120.0, 80.0));
                                    let image_button = egui::ImageButton::new(sized_image);
                                    let response = ui.add(image_button);
                                    
                                    if response.clicked() {
                                        info!("Clicked on image {}: {}", i, carousel_image.title);
                                        // Set this as the selected carousel image (for reference)
                                        *selected_carousel_image = Some(carousel_image.clone());
                                        
                                        // Check if we already have this high-res image cached
                                        if let Some(cached_image) = image_cache.get(&carousel_image.full_url) {
                                            info!("Using cached high-res image for: {}", carousel_image.title);
                                            *main_panel_image = Some(cached_image.clone());
                                            // Reset rectangle size for new image
                                            *reset_rectangle_for_new_image = true;
                                        } else {
                                            // Clear any existing main panel promise
                                            *main_panel_promise = None;
                                            
                                            // Fetch full resolution image for main panel
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
                                                    
                                                    // Include the bytes in the context with original URL
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
                                    // Show placeholder while loading or if image failed to load
                                    let placeholder = ui.add_sized([120.0, 80.0], egui::Button::new("📷 Loading..."));
                                    if placeholder.clicked() {
                                        info!("Clicked on placeholder for image {}: {}", i, carousel_image.title);
                                        *selected_carousel_image = Some(carousel_image.clone());
                                    }
                                    trace!("Image {} still loading or failed to load", i);
                                }
                                
                                // Show truncated title
                                let title = if carousel_image.title.len() > 15 {
                                    format!("{}...", &carousel_image.title[..15])
                                } else {
                                    carousel_image.title.clone()
                                };
                                ui.add_sized([120.0, 20.0], egui::Label::new(title).truncate());
                            });
                            ui.add_space(5.0);
                        }
                        
                        // Reset loading_more flag after images are processed
                        *loading_more = false;
                    });
                    ui.add_space(20.0);
                });
        } else {
            ui.label("Click 'Fetch Bing Daily Image' to load images");
        }
    });
    
    ui.horizontal(|ui| {
        if ui.button("Exit").clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        
        if ui.button("Fetch Bing Daily Image").clicked() {
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

            // Use bingtray-core to fetch images
            if let Some(config) = config {
                info!("Config is available");
                if !current_market_codes.is_empty() {
                    let market_code = &current_market_codes[0];
                    info!("Fetching Bing images using bingtray-core for market: {}", market_code);
                    info!("Available market codes: {:?}", current_market_codes);
                    
                    let ctx = ui.ctx().clone();
                    let (sender, promise) = Promise::new();
                    let market_code = market_code.clone();
                    
                    std::thread::spawn(move || {
                        info!("Starting API call in background thread for market: {}", market_code);
                        let result = get_bing_images(&market_code)
                            .map_err(|e| format!("Error fetching Bing images: {}", e));
                        info!("API call completed with result: {:?}", result.as_ref().map(|imgs| imgs.len()));
                        sender.send(result);
                    });
                    
                    *bing_api_promise = Some(promise);
                    *market_code_index = 1; // Next market code for infinite scroll
                    info!("Bing API promise created and stored");
                } else {
                    warn!("No market codes available!");
                }
            } else {
                warn!("No config available!");
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
        colored_text,
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
            } else if let Some(colored_text) = colored_text {
                colored_text.ui(ui);
            } else if let Some(text) = &text {
                ui.add(egui::Label::new(text).selectable(true));
            } else {
                ui.monospace("[binary]");
            }
        });
    }


    fn syntax_highlighting(
    ctx: &egui::Context,
    response: &ehttp::Response,
    text: &str,
) -> Option<ColoredText> {
    let extension_and_rest: Vec<&str> = response.url.rsplitn(2, '.').collect();
    let extension = extension_and_rest.first()?;
    #[cfg(not(target_os = "android"))]
    {
        let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(&ctx.style());
        Some(ColoredText(egui_extras::syntax_highlighting::highlight(
            ctx,
            &ctx.style(),
            &theme,
            text,
            extension,
        )))
    }
    #[cfg(target_os = "android")]
    {
        // For Android, just return plain text without syntax highlighting
        None
    }
}

struct ColoredText(egui::text::LayoutJob);

impl ColoredText {
    pub fn ui(&self, ui: &mut egui::Ui) {
        let mut job = self.0.clone();
        job.wrap.max_width = ui.available_width();
        let galley = ui.fonts(|f| f.layout_job(job));
        ui.add(egui::Label::new(galley).selectable(true));
    }
}

impl HttpApp {
    fn update_square_corners(&mut self) {
        // Get actual screen dimensions for rectangle calculation
        let (screen_width, screen_height) = Self::get_actual_screen_size();
        
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
    }
    
    fn update_screen_ratio(&mut self, _ui: &egui::Ui) {
        // Get actual screen ratio from device screen dimensions
        let (screen_width, screen_height) = Self::get_actual_screen_size();
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
                        let (screen_width, screen_height) = Self::get_actual_screen_size();
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
            let (screen_width, _screen_height) = Self::get_actual_screen_size();
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
                let (screen_width, screen_height) = Self::get_actual_screen_size();
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
            let (screen_width, _screen_height) = Self::get_actual_screen_size();
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
        
        // Center the rectangle in the image
        let center_x = image_rect.left() + image_width / 2.0;
        let center_y = image_rect.top() + image_height / 2.0;
        
        // Update the rectangle
        self.square_center = pos2(center_x - image_rect.left(), center_y - image_rect.top());
        
        // Update size factor based on new dimensions (relative to actual screen width)
        let (screen_width, _screen_height) = Self::get_actual_screen_size();
        self.square_size_factor = rect_width / screen_width;
        
        info!("Initialized rectangle: center=({:.1},{:.1}), size=({:.1}x{:.1}), factor={:.2}",
              self.square_center.x, self.square_center.y, rect_width, rect_height, self.square_size_factor);
        
        self.update_square_corners();
    }

    fn get_actual_screen_size() -> (f32, f32) {
        #[cfg(target_os = "android")]
        {
            match get_screen_size() {
                Ok((width, height)) => {
                    info!("Android screen size: {}x{}", width, height);
                    (width as f32, height as f32)
                }
                Err(e) => {
                    warn!("Failed to get Android screen size: {}, using default", e);
                    (1080.0, 1920.0) // Default mobile resolution
                }
            }
        }
        
        #[cfg(not(target_os = "android"))]
        {
            match screen_size::get_primary_screen_size() {
                Ok((width, height)) => {
                    info!("Desktop screen size: {}x{}", width, height);
                    (width as f32, height as f32)
                }
                Err(e) => {
                    warn!("Failed to get desktop screen size: {}, using default", e);
                    (1920.0, 1080.0) // Default desktop resolution
                }
            }
        }
    }

    fn has_next_wallpaper_available(&self) -> bool {
        if let Some(config) = &self.config {
            // Check if there are available market codes to download from
            let market_codes = load_market_codes(config).unwrap_or_default();
            let old_codes = get_old_market_codes(&market_codes);
            if !old_codes.is_empty() {
                return true;
            }
            
            // Check if historical data is available when no market codes are available
            if let Ok((current_page, total_pages)) = bingtray_core::get_historical_page_info(config) {
                return current_page < total_pages;
            }
        }
        
        false
    }

    fn load_next_images(&mut self) -> bool {
        if let Some(config) = &mut self.config {
            // First try market codes
            let mut market_codes = load_market_codes(config).unwrap_or_default();
            let old_codes = get_old_market_codes(&market_codes);
            
            if !old_codes.is_empty() {
                // Use the current market index or pick the first available old code
                if self.market_code_index < self.current_market_codes.len() {
                    let market_code = &self.current_market_codes[self.market_code_index];
                    info!("Loading images for market code: {}", market_code);
                    
                    if let Ok(images) = get_bing_images(market_code) {
                        // Increment market index for next time
                        self.market_code_index += 1;
                        
                        // Update timestamp for this market code
                        market_codes.insert(market_code.clone(), Utc::now().timestamp());
                        let _ = bingtray_core::save_market_codes(config, &market_codes);
                        
                        info!("Successfully loaded {} images from market code {}", images.len(), market_code);
                        return true;
                    }
                }
            } else {
                // Try historical data
                info!("No market codes available, trying historical data");
                if let Ok(Some(_historical_images)) = bingtray_core::get_next_historical_page(config) {
                    info!("Successfully loaded historical images");
                    return true;
                }
            }
        }
        
        false
    }
}
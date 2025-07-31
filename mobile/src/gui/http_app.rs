use egui::Image;
use poll_promise::Promise;
use egui::Vec2b;
use log::{trace, warn, info, error};
use std::time::SystemTime;
use std::collections::HashMap;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct BingImageData {
    images: Vec<BingImage>,
}

#[derive(Deserialize, Debug)]
struct BingImage {
    #[serde(rename = "startdate")]
    start_date: String,
    #[serde(rename = "fullstartdate")]
    full_start_date: String,
    #[serde(rename = "enddate")]
    end_date: String,
    url: String,
    #[serde(rename = "urlbase")]
    url_base: String,
    copyright: String,
    #[serde(rename = "copyrightlink")]
    copyright_link: String,
    title: String,
    quiz: String,
    wp: bool,
    hsh: String,
    drk: i32,
    top: i32,
    bot: i32,
}

#[derive(Clone)]
struct CarouselImage {
    title: String,
    copyright: String,
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
            ctx.include_bytes(response.url.clone(), response.bytes.clone());
            let image = Image::from_uri(response.url.clone());
            trace!("Image URL: {}", response.url);

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
    bing_api_promise: Option<Promise<ehttp::Result<BingImageData>>>,
}

impl Default for HttpApp {
    fn default() -> Self {
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
        }
    }
}

impl crate::Demo for HttpApp {
    fn name(&self) -> &'static str {
        "🌐 HTTP"
    }

    fn show(&mut self, ctx: &egui::Context, _open: &mut bool) {
        use crate::View as _;
        let screen_size = ctx.screen_rect().size();

        let mut window = egui::Window::new(&self.title)
            .default_width(screen_size.x)
            .default_height(screen_size.y)
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
        let trigger_fetch = ui_url(ui, &mut self.url, &mut self.carousel_images, &mut self.carousel_promises, &mut self.selected_carousel_image, &mut self.main_panel_image, &mut self.main_panel_promise, &mut self.image_cache, &mut self.bing_api_promise);

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
            if let Some(result) = bing_promise.ready() {
                match result {
                    Ok(bing_data) => {
                        info!("Bing API data received with {} images", bing_data.images.len());
                        
                        // Process each image from the Bing API response
                        for bing_image in &bing_data.images {
                            // Construct the full URLs
                            let thumbnail_url = format!("https://bing.com{}&w=320&h=240", bing_image.url);
                            let full_url = format!("https://bing.com{}&w=1920&h=1080", bing_image.url);
                            
                            let carousel_image = CarouselImage {
                                title: bing_image.title.clone(),
                                copyright: bing_image.copyright.clone(),
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
                                        ctx.include_bytes(response.url.clone(), response.bytes.clone());
                                        ctx.request_repaint();
                                        let image = Image::from_uri(response.url.clone());
                                        
                                        CarouselImage {
                                            title: carousel_image.title.clone(),
                                            copyright: carousel_image.copyright.clone(),
                                            thumbnail_url: carousel_image.thumbnail_url.clone(),
                                            full_url: carousel_image.full_url.clone(),
                                            image: Some(image),
                                            image_bytes: Some(image_bytes),
                                        }
                                    } else {
                                        CarouselImage {
                                            title: carousel_image.title.clone(),
                                            copyright: carousel_image.copyright.clone(),
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
        info!("Processing {} carousel promises", self.carousel_promises.len());
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
        if let Some(main_image) = &self.main_panel_image {
            ui.separator();
            ui.label(format!("Main Panel: {}", main_image.title));
            
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
                
                // if ui.button("Set with Crop").clicked() {
                //     if let Some(bytes) = &main_image.image_bytes {
                //         if !bytes.is_empty() {
                //             let image_data = bytes.clone();
                //             info!("Starting wallpaper setting with crop with {} bytes", image_data.len());
                //             // Start wallpaper setting with crop in background thread
                //             std::thread::spawn(move || {
                //                 log::info!("BingtrayApp: Starting wallpaper setting with crop from bytes in background thread");
                //                 match crate::set_wallpaper_with_crop_from_bytes(&image_data) {
                //                     Ok(true) => {
                //                         log::info!("BingtrayApp: Wallpaper setting with crop from bytes completed successfully");
                //                     }
                //                     Ok(false) => {
                //                         log::error!("BingtrayApp: Wallpaper setting with crop from bytes failed");
                //                     }
                //                     Err(e) => {
                //                         log::error!("BingtrayApp: Error during wallpaper setting with crop from bytes: {}", e);
                //                     }
                //                 }
                //             });
                //             // Immediately update UI status without waiting
                //             self.wallpaper_status = Some("✓ Wallpaper crop setting started".to_string());
                //             self.wallpaper_start_time = Some(SystemTime::now());
                //             ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
                //             log::info!("BingtrayApp: Finished processing wallpaper crop setting request from bytes");
                //         } else {
                //             error!("No image data available");
                //             self.wallpaper_status = Some("✗ No image data available".to_string());
                //             self.wallpaper_start_time = Some(SystemTime::now());
                //         }
                //     } else {
                //         error!("No image data available");
                //         self.wallpaper_status = Some("✗ No image data available".to_string());
                //         self.wallpaper_start_time = Some(SystemTime::now());
                //     }
                // }
            });
            
            // Display the main panel image
            if let Some(image) = &main_image.image {
                ui.add(image.clone().max_width(ui.available_width()));
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

fn ui_url(ui: &mut egui::Ui, _url: &mut String, carousel_images: &mut Vec<CarouselImage>, carousel_promises: &mut Vec<Promise<ehttp::Result<CarouselImage>>>, selected_carousel_image: &mut Option<CarouselImage>, main_panel_image: &mut Option<CarouselImage>, main_panel_promise: &mut Option<Promise<ehttp::Result<CarouselImage>>>, image_cache: &mut HashMap<String, CarouselImage>, bing_api_promise: &mut Option<Promise<ehttp::Result<BingImageData>>>) -> bool {
    let trigger_fetch = false;
    #[cfg(target_os = "android")]
    ui.add_space(40.0);

    // top panel image carousel
    egui::TopBottomPanel::top("top_panel")
    .min_height(100.0)
    .show_inside(ui, |ui| {
        ui.label("Bing Daily Images");
        ui.separator();
        
        if !carousel_images.is_empty() {
            ui.label(format!("Loaded {} images", carousel_images.len()));
            
            // Show debug info
            // let loaded_count = carousel_images.iter().filter(|img| img.image.is_some()).count();
            // ui.label(format!("Images with visuals: {}/{}", loaded_count, carousel_images.len()));
            
            // Debug: show details of first image
            // if let Some(first_img) = carousel_images.first() {
            //     ui.label(format!("First image: '{}' has_image={} url={}", 
            //         first_img.title, first_img.image.is_some(), first_img.thumbnail_url));
            // }
            
            egui::ScrollArea::horizontal()
                .auto_shrink(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (i, carousel_image) in carousel_images.iter().enumerate() {
                            ui.vertical(|ui| {
                                trace!("Checking image {} - has image: {}", i, carousel_image.image.is_some());
                                if let Some(image) = &carousel_image.image {
                                    info!("Attempting to display image {} in carousel", i);
                                    // Try to display the image
                                    let image_button = egui::ImageButton::new(image.clone().fit_to_exact_size(egui::Vec2::new(120.0, 80.0)));
                                    let response = ui.add(image_button);
                                    
                                    if response.clicked() {
                                        info!("Clicked on image {}: {}", i, carousel_image.title);
                                        // Set this as the selected carousel image (for reference)
                                        *selected_carousel_image = Some(carousel_image.clone());
                                        
                                        // Check if we already have this high-res image cached
                                        if let Some(cached_image) = image_cache.get(&carousel_image.full_url) {
                                            info!("Using cached high-res image for: {}", carousel_image.title);
                                            *main_panel_image = Some(cached_image.clone());
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
                                    ui.add_sized([120.0, 80.0], egui::Spinner::new());
                                    ui.label(format!("Loading {}", i));
                                    trace!("Image {} still loading", i);
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
                    });
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
            // Clear existing images and promises
            carousel_images.clear();
            carousel_promises.clear();
            image_cache.clear();
            *main_panel_image = None;
            *main_panel_promise = None;
            *selected_carousel_image = None;

            // Fetch the Bing API data first
            let ctx = ui.ctx().clone();
            let bing_api_url = "https://bing.com/HPImageArchive.aspx?format=js&idx=0&n=8&mkt=en-US".to_string();
            
            info!("Fetching Bing API data from: {}", bing_api_url);
            
            let (sender, promise) = Promise::new();
            let request = ehttp::Request::get(&bing_api_url);
            
            ehttp::fetch(request, move |response| {
                ctx.request_repaint();
                let result = response.and_then(|response| {
                    info!("Bing API response: status={}, size={} bytes", response.status, response.bytes.len());
                    
                    if response.status != 200 {
                        return Err(format!("HTTP {}: {}", response.status, response.status_text));
                    }
                    
                    let json_text = response.text().unwrap_or("");
                    info!("Bing API JSON response (first 200 chars): {}", &json_text[..json_text.len().min(200)]);
                    
                    match serde_json::from_str::<BingImageData>(json_text) {
                        Ok(bing_data) => {
                            info!("Successfully parsed Bing API data with {} images", bing_data.images.len());
                            Ok(bing_data)
                        }
                        Err(e) => {
                            error!("Failed to parse Bing API JSON: {}", e);
                            Err(format!("JSON parse error: {}", e))
                        }
                    }
                });
                sender.send(result);
            });
            
            // Store the promise for the Bing API response
            *bing_api_promise = Some(promise);
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
                
                // No longer need to show wallpaper promise status here since we handle it in the main UI loop
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
    let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(&ctx.style());
    Some(ColoredText(egui_extras::syntax_highlighting::highlight(
        ctx,
        &ctx.style(),
        &theme,
        text,
        extension,
    )))
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
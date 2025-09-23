use eframe::egui::{self, Color32};
use egui_material3::{
    MaterialButton,
    image_list,
    theme::{get_global_theme, ThemeMode, MaterialThemeContext},
};
use webbrowser;
use std::sync::mpsc;

use crate::core::app::App;

#[derive(Clone)]
struct DynamicImageItem {
    _id: usize,
    label: String,
    image_source: String,
}

pub struct Gui {
    // UI state
    is_dark_theme: bool,
    window_title: String,

    // Material3 components state
    switch_state: bool,
    slider_value: f32,
    checkbox_state: bool,

    // Application data
    wallpaper_path: Option<String>,

    // App instance
    app: Option<App>,

    // Tokio runtime for background tasks
    runtime: Option<tokio::runtime::Runtime>,

    // Cached image data to prevent excessive reloading
    cached_image_urls: Vec<String>,
    images_loaded: bool,

    // Standard dialog state
    standard_dialog_open: bool,
    selected_image_url: Option<String>,
    selected_image_title: String,
    selected_image_bytes: Option<Vec<u8>>,

    // Cropper state
    square_corners: [egui::Pos2; 4],
    square_center: egui::Pos2,
    square_size_factor: f32,
    screen_ratio: f32,
    dragging_corner: Option<usize>,
    image_display_rect: Option<egui::Rect>,
}

// desktop tray,cli,gui -> app -> core
// android/ios gui -> app -> core
// main -> gui -> app -> core
impl Gui {
    pub fn new() -> Self {
        Self {
            is_dark_theme: false,
            window_title: "BingTray".to_string(),
            switch_state: false,
            slider_value: 0.5,
            checkbox_state: false,
            wallpaper_path: None,
            app: None,
            runtime: None,
            cached_image_urls: Vec::new(),
            images_loaded: false,
            standard_dialog_open: false,
            selected_image_url: None,
            selected_image_title: String::new(),
            selected_image_bytes: None,
            square_corners: [egui::pos2(0.0, 0.0); 4],
            square_center: egui::pos2(0.0, 0.0),
            square_size_factor: 0.5,
            screen_ratio: 16.0 / 9.0,
            dragging_corner: None,
            image_display_rect: None,
        }
    }

    fn get_theme(&self) -> MaterialThemeContext {
        if let Ok(theme) = get_global_theme().lock() {
            theme.clone()
        } else {
            MaterialThemeContext::default()
        }
    }
    
    fn update_theme<F>(&self, update_fn: F) 
    where 
        F: FnOnce(&mut MaterialThemeContext)
    {
        if let Ok(mut theme) = get_global_theme().lock() {
            update_fn(&mut *theme);
        }
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let theme = self.get_theme();
        
        let mut visuals = match theme.theme_mode {
            ThemeMode::Light => egui::Visuals::light(),
            ThemeMode::Dark => egui::Visuals::dark(),
            ThemeMode::Auto => {
                // Use system preference or default to light
                if ctx.style().visuals.dark_mode {
                    egui::Visuals::dark()
                } else {
                    egui::Visuals::light()
                }
            }
        };
        
        // Apply Material Design 3 colors if theme is loaded
        let primary_color = theme.get_primary_color();
        let on_primary = theme.get_on_primary_color();
        let surface = theme.get_surface_color(visuals.dark_mode);
        let on_surface = theme.get_color_by_name("onSurface");

        // Convert material3 Color32 to egui Color32
        let primary_egui = Color32::from_rgba_unmultiplied(
            primary_color.r(),
            primary_color.g(),
            primary_color.b(),
            primary_color.a(),
        );

        let on_primary_egui = Color32::from_rgba_unmultiplied(
            on_primary.r(),
            on_primary.g(),
            on_primary.b(),
            on_primary.a(),
        );

        let surface_egui = Color32::from_rgba_unmultiplied(
            surface.r(),
            surface.g(),
            surface.b(),
            surface.a(),
        );

        let on_surface_egui = Color32::from_rgba_unmultiplied(
            on_surface.r(),
            on_surface.g(),
            on_surface.b(),
            on_surface.a(),
        );

        // Apply colors to visuals
        visuals.selection.bg_fill = primary_egui;
        visuals.selection.stroke.color = primary_egui;
        visuals.hyperlink_color = primary_egui;
        
        // Button and widget colors
        visuals.widgets.noninteractive.bg_fill = surface_egui;

        visuals.widgets.inactive.bg_fill = Color32::from_rgba_unmultiplied(
            primary_color.r(),
            primary_color.g(),
            primary_color.b(),
            20,
        );

        visuals.widgets.hovered.bg_fill = Color32::from_rgba_unmultiplied(
            primary_color.r(),
            primary_color.g(),
            primary_color.b(),
            40,
        );

        visuals.widgets.active.bg_fill = primary_egui;
        visuals.widgets.active.fg_stroke.color = on_primary_egui;

        // Window background
        visuals.window_fill = surface_egui;

        let surface_container = theme.get_color_by_name("surfaceContainer");
        visuals.panel_fill = Color32::from_rgba_unmultiplied(
            surface_container.r(),
            surface_container.g(),
            surface_container.b(),
            surface_container.a(),
        );

        // Text colors
        visuals.override_text_color = Some(on_surface_egui);

        // Apply surface colors
        let surface_container_lowest = theme.get_color_by_name("surfaceContainerLowest");
        visuals.extreme_bg_color = Color32::from_rgba_unmultiplied(
            surface_container_lowest.r(),
            surface_container_lowest.g(),
            surface_container_lowest.b(),
            surface_container_lowest.a(),
        );
        
        ctx.set_visuals(visuals);
    }

    fn initialize_rectangle_for_image(&mut self, image_rect: egui::Rect, screen_size: egui::Vec2) {
        let image_width = image_rect.width();
        let image_height = image_rect.height();
        let screen_aspect_ratio = screen_size.x / screen_size.y;

        self.screen_ratio = screen_aspect_ratio;

        let display_scale_factor = 0.8;
        let max_rect_width = screen_size.x * display_scale_factor;
        let max_rect_height = screen_size.y * display_scale_factor;

        let image_bigger_than_display = image_width > max_rect_width || image_height > max_rect_height;

        let (rect_width, _rect_height) = if image_bigger_than_display {
            let screen_ratio_width = max_rect_width.min(image_width);
            let screen_ratio_height = screen_ratio_width / screen_aspect_ratio;

            if screen_ratio_height > image_height {
                let height = image_height.min(max_rect_height);
                let width = height * screen_aspect_ratio;
                (width, height)
            } else {
                (screen_ratio_width, screen_ratio_height)
            }
        } else {
            let width_based_height = image_width / screen_aspect_ratio;
            if width_based_height <= image_height {
                (image_width, width_based_height)
            } else {
                let height_based_width = image_height * screen_aspect_ratio;
                (height_based_width, image_height)
            }
        };

        let center_x = image_width / 2.0;
        let center_y = image_height / 2.0;

        self.square_center = egui::pos2(center_x, center_y);
        self.square_size_factor = rect_width / 800.0; // Use a fixed reference width

        self.update_square_corners();
    }

    fn update_square_corners(&mut self) {
        let half_width = (400.0 * self.square_size_factor) / 2.0; // Reference half-width
        let half_height = half_width / self.screen_ratio;

        self.square_corners[0] = egui::pos2(self.square_center.x - half_width, self.square_center.y - half_height); // top-left
        self.square_corners[1] = egui::pos2(self.square_center.x + half_width, self.square_center.y - half_height); // top-right
        self.square_corners[2] = egui::pos2(self.square_center.x + half_width, self.square_center.y + half_height); // bottom-right
        self.square_corners[3] = egui::pos2(self.square_center.x - half_width, self.square_center.y + half_height); // bottom-left
    }

    fn render_square_shape(&mut self, ui: &mut egui::Ui, available_rect: egui::Rect) -> egui::Response {
        let response = ui.allocate_rect(available_rect, egui::Sense::click_and_drag());

        // Store corner data to avoid borrowing issues
        let corners = self.square_corners;
        let dragging_corner = self.dragging_corner;

        // Draw the square selection
        let corner_radius = 5.0;
        let mut corner_responses = Vec::new();

        for (i, corner) in corners.iter().enumerate() {
            let corner_rect = egui::Rect::from_center_size(*corner, egui::Vec2::splat(corner_radius * 2.0));

            let corner_color = if dragging_corner == Some(i) {
                egui::Color32::RED
            } else {
                egui::Color32::BLUE
            };

            ui.painter().circle_filled(*corner, corner_radius, corner_color);

            // Handle corner dragging
            let corner_response = ui.allocate_rect(corner_rect, egui::Sense::click_and_drag());
            corner_responses.push((i, corner_response));
        }

        // Process corner responses after the loop
        for (i, corner_response) in corner_responses {
            if corner_response.drag_started() {
                self.dragging_corner = Some(i);
            }
            if corner_response.dragged() && self.dragging_corner == Some(i) {
                self.square_corners[i] += corner_response.drag_delta();
                self.update_center_from_corners();
            }
        }

        if !response.dragged() && self.dragging_corner.is_some() {
            self.dragging_corner = None;
        }

        // Draw lines connecting corners
        for i in 0..4 {
            let start = self.square_corners[i];
            let end = self.square_corners[(i + 1) % 4];
            ui.painter().line_segment([start, end], egui::Stroke::new(2.0, egui::Color32::BLUE));
        }

        response
    }

    fn update_center_from_corners(&mut self) {
        let sum_x = self.square_corners.iter().map(|p| p.x).sum::<f32>();
        let sum_y = self.square_corners.iter().map(|p| p.y).sum::<f32>();
        self.square_center = egui::pos2(sum_x / 4.0, sum_y / 4.0);
    }
    
    pub fn show(&mut self, ctx: &egui::Context) {
        // Only load images once when app is available and images haven't been loaded yet
        if !self.images_loaded {
            if let Some(app) = &mut self.app {
                if let Some(_runtime) = &self.runtime {
                    // Get current page of images from metadata
                    match app.get_wallpaper_metadata_page(0, 8) {
                        Ok(metadata_list) => {
                            self.cached_image_urls = metadata_list.iter()
                                .map(|metadata| "https://www.bing.com".to_string() + &metadata.thumbnail_url)
                                .collect();
                            self.images_loaded = true;
                        }
                        Err(e) => {
                            log::error!("Failed to load wallpaper metadata: {}", e);
                            // Fallback to dummy data
                            self.cached_image_urls = (1..=8)
                                .map(|_i| "bingtray/resources/320x240.png".to_string())
                                .collect();
                            self.images_loaded = true;
                        }
                    }
                }
            } else {
                // Fallback when app is not initialized yet
                self.cached_image_urls = (1..=8)
                    .map(|_i| "bingtray/resources/320x240.png".to_string())
                    .collect();
                self.images_loaded = true;
            }
        }
        // Apply theme based on settings
        self.apply_theme(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            let theme = self.get_theme();
            // Top bar with title and buttons
            ui.horizontal(|ui| {
                ui.heading("BingTray");
                if ui.selectable_label(false, "About").clicked() {
                    let _ = webbrowser::open("https://bingtray.pages.dev");
                } 
                if ui.selectable_label(false, "Exit").clicked() {
                    std::process::exit(0);
                }   

                // right aligned theme mode selector
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.horizontal(|ui| {
                        // Light mode button
                        let light_selected = theme.theme_mode == ThemeMode::Light;
                        let light_button = ui.selectable_label(light_selected, "☀️ Light");
                        if light_button.clicked() {
                            self.update_theme(|theme| {
                                theme.theme_mode = ThemeMode::Light;
                            });
                        }
                        
                        // Auto mode button  
                        let auto_selected = theme.theme_mode == ThemeMode::Auto;
                        let auto_button = ui.selectable_label(auto_selected, "🌗 Auto");
                        if auto_button.clicked() {
                            self.update_theme(|theme| {
                                theme.theme_mode = ThemeMode::Auto;
                            });
                        }
                        
                        // Dark mode button
                        let dark_selected = theme.theme_mode == ThemeMode::Dark;
                        let dark_button = ui.selectable_label(dark_selected, "🌙 Dark");
                        if dark_button.clicked() {
                            self.update_theme(|theme| {
                                theme.theme_mode = ThemeMode::Dark;
                            });
                        }
                    });
                });
            });
            ui.add_space(20.0);

            // Top Buttons
            ui.horizontal(|ui| {
                let fetch_button = MaterialButton::outlined("Fetch");
                if ui.add(fetch_button).clicked() {
                    // self.add_image();
                }
                let history_button = MaterialButton::outlined("History");
                if ui.add(history_button).clicked() {
                    // self.remove_image();
                }
            });
            ui.add_space(10.0);
            
            // Dynamic image list - use cached URLs to prevent excessive reloading
            egui::ScrollArea::vertical().show(ui, |ui| {
                if !self.cached_image_urls.is_empty() {
                    let mut image_list_builder = image_list()
                        .id_salt("standard_imagelist")
                        .columns(2)
                        .item_spacing(10.0)
                        .text_protected(true);

                    // Use a channel to communicate from callbacks
                    let (sender, receiver) = mpsc::channel::<(String, String)>();

                    for (index, url) in self.cached_image_urls.iter().enumerate() {
                        let title = format!("Image {}", index + 1);
                        let url_clone = url.clone();
                        let title_clone = title.clone();
                        let sender_clone = sender.clone();

                        image_list_builder = image_list_builder.item_with_callback(
                            &title,
                            url,
                            move || {
                                let _ = sender_clone.send((url_clone.clone(), title_clone.clone()));
                            }
                        );
                    }

                    ui.label("Recent Wallpapers:");
                    ui.add(image_list_builder);

                    // Check for any messages from callbacks
                    if let Ok((selected_url, selected_title)) = receiver.try_recv() {
                        self.selected_image_url = Some(selected_url);
                        self.selected_image_title = selected_title;
                        self.selected_image_bytes = None;
                        self.standard_dialog_open = true;
                    }
                }
                
                ui.add_space(20.0);

                // Standard categories list with click callbacks
                // let _standard_list = image_list()
                //     .id_salt("standard_imagelist")
                //     .columns(3)
                //     .item_spacing(8.0)
                //     .text_protected(false);

                // ui.label("Browse Categories:");
                // ui.add_space(10.0);

                // ui.horizontal_wrapped(|ui| {
                //     let categories = [
                //         ("Architecture", "resources/320x240.png"),
                //         ("Nature", "resources/320x240.png"),
                //         ("Abstract Art", "resources/320x240.png"),
                //         ("Street Photo", "resources/320x240.png"),
                //         ("Portrait", "resources/320x240.png"),
                //         ("Landscape", "resources/320x240.png"),
                //     ];

                //     for (category, _image_path) in categories.iter() {
                //         if ui.button(*category).clicked() {
                //             self.selected_image_title = category.to_string();
                //             self.selected_image_url = Some(format!("https://example.com/{}", category.to_lowercase()));
                //             self.standard_dialog_open = true;
                //         }
                //     }
                // });
            });

            // Standard dialog implementation
            if self.standard_dialog_open {
                let dialog_title = self.selected_image_title.clone();
                let selected_image_title = self.selected_image_title.clone();
                let selected_image_url = self.selected_image_url.clone();

                egui::Window::new(&dialog_title)
                    .open(&mut self.standard_dialog_open)
                    .resizable(true)
                    .default_width(ctx.screen_rect().width())
                    .default_height(ctx.screen_rect().height())
                    .show(ctx, |ui| {
                        ui.label(format!("Category: {}", selected_image_title));

                        if let Some(url) = &selected_image_url {
                            ui.label(format!("URL: {}", url));
                        }

                        ui.separator();

                        // Image display area with cropper overlay
                        ui.label("Select cropping area:");

                        // Create a sample image or placeholder
                        let available_width = ui.available_width().min(400.0);
                        let target_height = available_width * 9.0 / 16.0; // 16:9 aspect ratio

                        let image_rect = ui.allocate_response(
                            egui::Vec2::new(available_width, target_height),
                            egui::Sense::hover()
                        );

                        // Draw background image placeholder
                        ui.painter().rect_filled(
                            image_rect.rect,
                            5.0,
                            egui::Color32::from_rgb(100, 150, 200)
                        );

                        ui.separator();

                        // Action buttons
                        ui.horizontal(|ui| {
                            if ui.button("Set this Wallpaper").clicked() {
                                log::info!("Setting wallpaper for: {}", selected_image_title);
                                if let Err(e) = webbrowser::open("https://bingtray.pages.dev") {
                                    log::error!("Failed to open URL: {}", e);
                                }
                            }

                            if ui.button("Set Cropped Wallpaper").clicked() {
                                log::info!("Setting cropped wallpaper for: {}", selected_image_title);
                                if let Err(e) = webbrowser::open("https://bingtray.pages.dev") {
                                    log::error!("Failed to open URL: {}", e);
                                }
                            }

                            if ui.button("More Info").clicked() {
                                log::info!("More info clicked for: {}", selected_image_title);
                                if let Err(e) = webbrowser::open("https://bingtray.pages.dev") {
                                    log::error!("Failed to open URL: {}", e);
                                }
                            }
                        });

                        ui.separator();

                        ui.horizontal(|ui| {
                            if ui.button("OK").clicked() {
                                log::info!("Standard dialog OK clicked!");
                            }

                            if ui.button("Close").clicked() {
                                log::info!("Standard dialog Close clicked!");
                            }
                        });
                    });
            }
        });
    }
}

impl Default for Gui {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Lazy initialization of App and runtime
        if self.app.is_none() {
            // Create and store the runtime
            match tokio::runtime::Runtime::new() {
                Ok(runtime) => {
                    match runtime.block_on(App::new()) {
                        Ok(app) => {
                            self.app = Some(app);
                            self.runtime = Some(runtime);
                            log::info!("App initialized successfully");
                        }
                        Err(e) => {
                            log::error!("Failed to initialize App: {}", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to create tokio runtime: {}", e);
                }
            }
        }

        self.show(ctx);
    }
}
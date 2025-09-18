use eframe::egui::{self, Color32};
use egui_material3::{
    MaterialButton, MaterialCheckbox, MaterialSlider, MaterialSwitch,
    image_list,
    theme::{setup_local_fonts, setup_local_theme, load_fonts, load_themes, update_window_background, get_global_theme, MaterialThemeFile, ThemeMode, ContrastLevel, MaterialThemeContext},
};
use webbrowser;
use egui::Image;

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
                                .map(|i| "bingtray/resources/320x240.png".to_string())
                                .collect();
                            self.images_loaded = true;
                        }
                    }
                }
            } else {
                // Fallback when app is not initialized yet
                self.cached_image_urls = (1..=8)
                    .map(|i| "bingtray/resources/320x240.png".to_string())
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

            // Main content
            // ui.horizontal(|ui| {
            //     let fetch_button = MaterialButton::outlined("Fetch");
            //     if ui.add(fetch_button).clicked() {
            //         // self.add_image();
            //     }
            //     let history_button = MaterialButton::outlined("History");
            //     if ui.add(history_button).clicked() {
            //         // self.remove_image();
            //     }
            // });
            ui.add_space(10.0);
            
            // Dynamic image list - use cached URLs to prevent excessive reloading
            egui::ScrollArea::vertical().show(ui, |ui| {
                if !self.cached_image_urls.is_empty() {
                    ui.add(image_list()
                        .id_salt("interactive_imagelist")
                        .columns(1)
                        .item_spacing(8.0)
                        .items_from_urls(self.cached_image_urls.clone())
                    );
                }
            });
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
use eframe::egui::{self, Color32, ScrollArea, Vec2};

use crate::core::Demo;
use crate::core::app::{App, CarouselImage};

use egui_material3::{
    theme::{get_global_theme, ThemeMode, MaterialThemeContext},
};


#[derive(Clone)]
struct DynamicImageItem {
    _id: usize,
    label: String,
    image_source: String,
}

pub struct Gui {
    // App-related fields
    app: Option<App>,
    runtime: Option<tokio::runtime::Runtime>,

    // Cached image data to prevent excessive reloading
    carousel_image_lists: Vec<CarouselImage>,
    images_loaded: bool,

    // scroll state
    current_page: i32,
    page_size: i32,

    // Page caching for infinite scroll
    previous_page_cache: Option<Vec<CarouselImage>>,
    current_page_cache: Option<Vec<CarouselImage>>,
    next_page_cache: Option<Vec<CarouselImage>>,

    // Dialog state
    dialog_open: bool,
    dialog_image: Option<CarouselImage>,
}

impl Default for Gui {
    fn default() -> Self {
        Self {
            app: None,
            runtime: None,

            carousel_image_lists: Vec::new(),
            images_loaded: false,
            current_page: 0,
            page_size: 8,

            previous_page_cache: None,
            current_page_cache: None,
            next_page_cache: None,

            dialog_open: false,
            dialog_image: None,
        }
    }
}

impl Gui {
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

    fn load_images_if_needed(&mut self) {
        if !self.images_loaded {
            if let Some(ref mut app) = self.app {
                if let Some(ref mut runtime) = self.runtime {
                    match runtime.block_on(async {
                        app.get_wallpaper_metadata_page(self.current_page, self.page_size)
                    }) {
                        Ok(images) => {
                            self.carousel_image_lists = images;
                            self.images_loaded = true;
                            // Cache next page
                            if let Ok(next_images) = runtime.block_on(async {
                                app.get_wallpaper_metadata_page(self.current_page + 1, self.page_size)
                            }) {
                                self.next_page_cache = Some(next_images);
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to load images: {}", e);
                        }
                    }
                }
            }
        }
    }

    fn load_page(&mut self, page: i32) {
        if let Some(ref mut app) = self.app {
            if let Some(ref mut runtime) = self.runtime {
                match runtime.block_on(async {
                    app.get_wallpaper_metadata_page(page, self.page_size)
                }) {
                    Ok(images) => {
                        self.carousel_image_lists = images;
                        self.current_page = page;

                        // Cache management for infinite scroll
                        if page >= 3 {
                            // Load page 4 data to next page cache and page 2 data to previous page cache
                            if let Ok(page_4_images) = runtime.block_on(async {
                                app.get_wallpaper_metadata_page(page + 1, self.page_size)
                            }) {
                                self.next_page_cache = Some(page_4_images);
                            }

                            if let Ok(page_2_images) = runtime.block_on(async {
                                app.get_wallpaper_metadata_page(page - 1, self.page_size)
                            }) {
                                self.previous_page_cache = Some(page_2_images);
                            }
                        } else {
                            // Normal caching
                            if let Ok(next_images) = runtime.block_on(async {
                                app.get_wallpaper_metadata_page(page + 1, self.page_size)
                            }) {
                                self.next_page_cache = Some(next_images);
                            }

                            if page > 0 {
                                if let Ok(prev_images) = runtime.block_on(async {
                                    app.get_wallpaper_metadata_page(page - 1, self.page_size)
                                }) {
                                    self.previous_page_cache = Some(prev_images);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to load page {}: {}", page, e);
                    }
                }
            }
        }
    }
}

impl crate::core::Demo for Gui {
    fn name(&self) -> &'static str {
        "Bingtray"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        self.apply_theme(ctx);

        use crate::core::View as _;
        let mut window = egui::Window::new("bingtray")
            .id(egui::Id::new("bingtray_gui")) // required since we change the title
            .default_width(ctx.screen_rect().width())
            .default_height(ctx.screen_rect().height())
            .resizable(true)
            .constrain(false)
            .collapsible(false)
            .title_bar(false)
            .scroll(true)
            .enabled(true);
        window = window.open(open);
        window = window.anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO);
        window.show(ctx, |ui| self.ui(ui));
    }
}

impl crate::core::View for Gui {

    fn ui(&mut self, ui: &mut egui::Ui) {
        // Load images if needed
        self.load_images_if_needed();
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

        // Image grid with infinite scroll
        ui.label("Recent Wallpapers:");

        // Show images from get_wallpaper_metadata_page in app.rs using a grid layout
        // scroll down shows next page images, making infinite scroll. ie. when you reach 3 page, load page 4 data to next page variable and load page 2 data to previous page variable.
        // scroll up shows previous page images, making infinite scroll.
        if !self.carousel_image_lists.is_empty() {
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    // Create a 4x2 grid layout for 8 images
                    let available_width = ui.available_width();
                    let image_width = (available_width - 40.0) / 4.0; // 4 columns with spacing
                    let image_height = 120.0;

                    // Display images in a 4x2 grid
                    for row in 0..2 {
                        ui.horizontal(|ui| {
                            for col in 0..4 {
                                let index = row * 4 + col;
                                if index < self.carousel_image_lists.len() {
                                    let image = &self.carousel_image_lists[index];

                                    let button = egui::Button::new(&image.title)
                                        .min_size(Vec2::new(image_width, image_height))
                                        .wrap();

                                    let response = ui.add(button);

                                    if response.clicked() {
                                        log::info!("Image clicked: {}", image.title);
                                        self.dialog_open = true;
                                        self.dialog_image = Some(image.clone());
                                    }
                                }
                            }
                        });
                        ui.add_space(10.0);
                    }

                    // Infinite scroll logic with proper input handling
                    ui.input(|i| {
                        if i.raw_scroll_delta.y < -10.0 {
                            // Scrolling down
                            if let Some(next_cache) = &self.next_page_cache {
                                if !next_cache.is_empty() {
                                    self.load_page(self.current_page + 1);
                                }
                            }
                        } else if i.raw_scroll_delta.y > 10.0 {
                            // Scrolling up
                            if self.current_page > 0 {
                                if let Some(prev_cache) = &self.previous_page_cache {
                                    if !prev_cache.is_empty() {
                                        self.load_page(self.current_page - 1);
                                    }
                                }
                            }
                        }
                    });
                });
        } else {
            ui.label("Loading images...");
        }

        // Show dialog if open
        if self.dialog_open {
            if let Some(ref image) = self.dialog_image.clone() {
                let mut open = true;
                //open dialog using Dialog window
                let mut dialog = crate::core::Dialog {
                    title: image.title.clone(),
                    selected_image_title: image.title.clone(),
                    selected_image_url: image.url.clone(),
                    open: true,
                };
                dialog.show(ui.ctx(), &mut open);

                if !open {
                    self.dialog_open = false;
                    self.dialog_image = None;
                }
            }
        }

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

        let mut open = true;
        self.show(ctx, &mut open);
    }
}

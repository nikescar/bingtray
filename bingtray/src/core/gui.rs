use eframe::egui::{self};
use egui::{UiKind, Vec2b};

use crate::core::Demo;

use egui_material3::{
    theme::{get_global_theme, ThemeMode, MaterialThemeContext},
};

use crate::core::app::{App, CarouselImage};


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
}

impl Default for Gui {
    fn default() -> Self {
        Self {
            app: None,
            runtime: None,
        }
    }
}

impl crate::core::Demo for Gui {
    fn name(&self) -> &'static str {
        "Bingtray"
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

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
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
        let Self {
            app: _,
            runtime: _,
        } = self;
        
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

        // Dynamic image list - use cached URLs to prevent excessive reloading
        // if !self.carousel_image_lists.is_empty() {
        //     let mut image_list_builder = image_list()
        //         .id_salt("standard_imagelist")
        //         .columns(2)
        //         .item_spacing(10.0)
        //         .text_protected(true);

        //     // Use a channel to communicate from callbacks
        //     let (sender, receiver) = mpsc::channel::<(String, String)>();

        //     for carousel_image in self.carousel_image_lists.iter() {
        //         let title = &carousel_image.title;
        //         let thumbnail_url = if let Some(ref path) = carousel_image.thumbnail_path {
        //             path.clone()
        //         } else {
        //             carousel_image.thumbnail_url.clone()
        //         };
        //         let full_url_clone = carousel_image.full_url.clone();
        //         let title_clone = title.clone();
        //         let sender_clone = sender.clone();

        //         image_list_builder = image_list_builder.item_with_callback(
        //             title,
        //             &thumbnail_url,
        //             move || {
        //                 let _ = sender_clone.send((full_url_clone.clone(), title_clone.clone()));
        //             }
        //         );
        //     }

        //     ui.label("Recent Wallpapers:");
        //     ui.add(image_list_builder);

        //     // Check for any messages from callbacks
        //     if let Ok((selected_url, selected_title)) = receiver.try_recv() {
        //         self.selected_image_url = Some(selected_url);
        //         self.selected_image_title = selected_title;
        //         self.selected_image_bytes = None;
        //         self.standard_dialog_open = true;
        //     }
        // }
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

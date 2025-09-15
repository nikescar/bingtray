use eframe::egui::{self, Color32};
use egui_material3::{
    MaterialButton, MaterialCheckbox, MaterialSlider, MaterialSwitch,
    image_list,
    theme::{setup_local_fonts, setup_local_theme, load_fonts, load_themes, update_window_background, get_global_theme, MaterialThemeFile, ThemeMode, ContrastLevel, MaterialThemeContext},
};
use webbrowser;

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
        
        // Apply colors to visuals
        visuals.selection.bg_fill = primary_color;
        visuals.selection.stroke.color = primary_color;
        visuals.hyperlink_color = primary_color;
        
        // Button and widget colors
        visuals.widgets.noninteractive.bg_fill = surface;
        
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
        
        visuals.widgets.active.bg_fill = primary_color;
        visuals.widgets.active.fg_stroke.color = on_primary;
        
        // Window background
        visuals.window_fill = surface;
        visuals.panel_fill = theme.get_color_by_name("surfaceContainer");
        
        // Text colors
        visuals.override_text_color = Some(on_surface);
        
        // Apply surface colors
        visuals.extreme_bg_color = theme.get_color_by_name("surfaceContainerLowest");
        
        ctx.set_visuals(visuals);
    }
    
    pub fn show(&mut self, ctx: &egui::Context) {
        let mut dynamic_images = Vec::new();
        for i in 1..=8 {
            dynamic_images.push(DynamicImageItem {
                _id: i,
                label: format!("Photo {:03}", i),
                image_source: format!("photo{}.jpg", i),
            });
        }
        // Apply theme based on settings
        self.apply_theme(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            let theme = self.get_theme();
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
            ui.horizontal(|ui| {
                if ui.add(MaterialButton::outlined("Fetch")).clicked() {
                    // self.add_image();
                }
                if ui.add(MaterialButton::outlined("History")).clicked() {
                    // self.remove_image();
                }
            });

            ui.add_space(10.0);
            
            let mut interactive_list = image_list()
                .id_salt("interactive_imagelist")
                .columns(1)
                // .item_spacing(self.item_spacing)
                .text_protected(true);
                
            // Add dynamic images from vector
            for image in dynamic_images {
                let label = image.label.clone();
                let image_source = image.image_source.clone();
                interactive_list = interactive_list.item_with_callback(
                    label.clone(),
                    image_source,
                    move || println!("{} selected!", label)
                );
            }
            
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add(interactive_list);
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
        self.show(ctx);
    }
}
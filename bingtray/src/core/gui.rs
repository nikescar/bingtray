// gui - egui

use eframe::egui;
use egui_material3::{
    MaterialButton, MaterialCheckbox, MaterialSlider, MaterialSwitch,
    theme::{setup_local_fonts, setup_local_theme, load_fonts, load_themes, update_window_background}
};

use crate::core::app::App;
// Remove unused imports for now - add back when needed with correct module paths

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
    app: App,
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

            app: App::new(),
        };

        // make instance of app
        self
    }
    
    pub fn show(&mut self, ctx: &egui::Context) {
        
    }
}
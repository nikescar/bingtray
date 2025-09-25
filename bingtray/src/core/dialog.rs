use egui::{UiKind, Vec2b};

#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Dialog {
    
}

impl Default for Dialog {
    fn default() -> Self {
        Self {
            
        }
    }
}

impl crate::Demo for Dialog {
    fn name(&self) -> &'static str {
        "Dialog"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        let Self {
            
        } = self.clone();

        use crate::core::View as _;
        let mut window = egui::Window::new(title)
            .id(egui::Id::new("bingtray_dialog")) // required since we change the title
            .default_width(ctx.screen_rect().width())
            .default_height(ctx.screen_rect().height())
            .resizable(false)
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

impl crate::core::View for Dialog {
    fn ui(&mut self, ui: &mut egui::Ui) {
        let Self {
            
        } = self;
        

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


    }
}
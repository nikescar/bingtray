use egui::{Image, Pos2, pos2, Rect, Sense, Stroke, Vec2};
use log::{trace, info, error};

use crate::core::app::BingtrayAppState;

pub struct EguiCarouselState {
    pub scroll_position: f32,
    pub square_corners: [Pos2; 4],
    pub square_size_factor: f32,
    pub square_center: Pos2,
    pub dragging_corner: Option<usize>,
    pub screen_ratio: f32,
    pub reset_rectangle_for_new_image: bool,
    pub current_main_image_url: Option<String>,
    pub image_display_rect: Option<Rect>,
}

impl Default for EguiCarouselState {
    fn default() -> Self {
        let (screen_width, screen_height) = BingtrayAppState::get_initial_screen_size();
        let screen_ratio = screen_width / screen_height;
        
        let rect_scale_factor = 0.3;
        let rect_width = screen_width * rect_scale_factor;
        let rect_height = screen_height * rect_scale_factor;
        
        let center_x = 300.0;
        let center_y = 200.0;
        let half_width = rect_width / 2.0;
        let half_height = rect_height / 2.0;
        
        let square_corners = [
            pos2(center_x - half_width, center_y - half_height),
            pos2(center_x + half_width, center_y - half_height),
            pos2(center_x + half_width, center_y + half_height),
            pos2(center_x - half_width, center_y + half_height),
        ];

        Self {
            scroll_position: 0.0,
            square_corners,
            square_size_factor: 1.0,
            square_center: pos2(center_x, center_y),
            dragging_corner: None,
            screen_ratio,
            reset_rectangle_for_new_image: true,
            current_main_image_url: None,
            image_display_rect: None,
        }
    }
}

pub struct Resource {
    pub response: ehttp::Response,
    pub text: Option<String>,
    pub image: Option<Image<'static>>,
}

impl Resource {
    pub fn from_response(ctx: &egui::Context, response: ehttp::Response) -> Self {
        let content_type = response.content_type().unwrap_or_default();
        if content_type.starts_with("image/") {
            ctx.include_bytes(response.url.clone(), response.bytes.clone());
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

impl EguiCarouselState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update_square_corners(&mut self, app_state: &BingtrayAppState) {
        let (screen_width, screen_height) = match app_state.cached_screen_size {
            Some((w, h)) => (w, h),
            None => BingtrayAppState::get_initial_screen_size(),
        };
        
        let new_screen_ratio = screen_width / screen_height;
        if (new_screen_ratio - self.screen_ratio).abs() > 0.01 {
            self.screen_ratio = new_screen_ratio;
        }
        
        let rect_scale_factor = 0.3 * self.square_size_factor;
        let rect_width = screen_width * rect_scale_factor;
        let rect_height = screen_height * rect_scale_factor;
        
        let half_width = rect_width / 2.0;
        let half_height = rect_height / 2.0;
        
        self.square_corners = [
            pos2(self.square_center.x - half_width, self.square_center.y - half_height),
            pos2(self.square_center.x + half_width, self.square_center.y - half_height),
            pos2(self.square_center.x + half_width, self.square_center.y + half_height),
            pos2(self.square_center.x - half_width, self.square_center.y + half_height),
        ];
    }

    pub fn render_square_shape(&mut self, ui: &mut egui::Ui, available_rect: Rect) -> egui::Response {
        let response = ui.allocate_rect(available_rect, Sense::click_and_drag());
        
        if ui.is_rect_visible(response.rect) {
            let stroke = Stroke::new(2.0, egui::Color32::from_rgb(255, 255, 255));
            
            // Draw rectangle lines
            {
                let painter = ui.painter();
                for i in 0..4 {
                    let start = self.square_corners[i];
                    let end = self.square_corners[(i + 1) % 4];
                    painter.line_segment([start, end], stroke);
                }
            }
            
            // Handle corner dragging
            let corners = self.square_corners;  // Copy the corners to avoid borrowing issues
            for (i, corner) in corners.iter().enumerate() {
                let corner_rect = Rect::from_center_size(*corner, Vec2::splat(10.0));
                let corner_response = ui.allocate_rect(corner_rect, Sense::click_and_drag());
                
                let painter = ui.painter();
                painter.circle_filled(*corner, 5.0, egui::Color32::from_rgb(255, 255, 255));
                
                if corner_response.drag_started() {
                    self.dragging_corner = Some(i);
                } else if corner_response.dragged() && self.dragging_corner == Some(i) {
                    if let Some(pointer_pos) = corner_response.interact_pointer_pos() {
                        self.square_corners[i] = pointer_pos;
                        
                        let center_x = self.square_corners.iter().map(|p| p.x).sum::<f32>() / 4.0;
                        let center_y = self.square_corners.iter().map(|p| p.y).sum::<f32>() / 4.0;
                        self.square_center = pos2(center_x, center_y);
                    }
                } else if corner_response.drag_stopped() {
                    self.dragging_corner = None;
                }
            }
        }
        
        response
    }

    pub fn initialize_rectangle_for_image(&mut self, image_rect: Rect, screen_size: Vec2) {
        if !self.reset_rectangle_for_new_image {
            return;
        }
        
        let image_center = image_rect.center();
        let image_size = image_rect.size();
        
        let default_crop_width = image_size.x * 0.8;
        let default_crop_height = default_crop_width * (screen_size.y / screen_size.x);
        
        let crop_half_width = default_crop_width / 2.0;
        let crop_half_height = default_crop_height / 2.0;
        
        self.square_corners = [
            pos2(image_center.x - crop_half_width, image_center.y - crop_half_height),
            pos2(image_center.x + crop_half_width, image_center.y - crop_half_height),
            pos2(image_center.x + crop_half_width, image_center.y + crop_half_height),
            pos2(image_center.x - crop_half_width, image_center.y + crop_half_height),
        ];
        
        self.square_center = image_center;
        
        info!("Initialized crop rectangle for new image at center: ({:.1}, {:.1}), size: {:.1}x{:.1}",
              image_center.x, image_center.y, default_crop_width, default_crop_height);
        
        self.reset_rectangle_for_new_image = false;
    }

    pub fn render_carousel_images(
        &mut self,
        ui: &mut egui::Ui,
        app_state: &mut BingtrayAppState,
        _ctx: &egui::Context,
    ) {
        ui.horizontal(|ui| {
            let mut should_load_more = false;
            
            egui::ScrollArea::horizontal()
                .max_height(150.0)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Note: Image loading is now handled by ehttp callbacks directly
                        // Images will be added to carousel_images directly in the callbacks
                        
                        for (index, carousel_image) in app_state.carousel_images.iter().enumerate() {
                            let image_url = &carousel_image.thumbnail_url;
                            let image = Image::from_uri(image_url.clone()).max_height(120.0);
                            
                            let response = ui.add(image);
                            
                            if response.clicked() {
                                info!("Selected carousel image: {}", carousel_image.title);
                                app_state.selected_carousel_image = Some(carousel_image.clone());
                                
                                let full_image_url = if std::path::Path::new(&carousel_image.full_url).exists() {
                                    carousel_image.full_url.clone()
                                } else {
                                    if let Some(bing_url) = app_state.get_image_bing_url(&carousel_image.title) {
                                        bing_url
                                    } else {
                                        carousel_image.full_url.clone()
                                    }
                                };
                                
                                let image_clone_for_promise = carousel_image.clone();
                                let ctx_clone = ui.ctx().clone();
                                
                                ehttp::fetch(ehttp::Request::get(&full_image_url), move |result| {
                                    match result {
                                        Ok(response) => {
                                            let mut updated_image = image_clone_for_promise;
                                            updated_image.image_bytes = Some(response.bytes);
                                            ctx_clone.request_repaint();
                                        }
                                        Err(e) => {
                                            error!("Failed to load image: {}", e);
                                        }
                                    }
                                });
                                
                                app_state.main_panel_image = Some(carousel_image.clone());
                                self.reset_rectangle_for_new_image = true;
                                self.current_main_image_url = Some(full_image_url.clone());
                            }
                            
                            if index == app_state.carousel_images.len() - 1 {
                                should_load_more = true;
                            }
                        }
                        
                        if should_load_more && !app_state.loading_more && app_state.carousel_images.len() > 0 && !app_state.all_data_exhausted {
                            info!("Reached end of carousel, loading more images...");
                            app_state.loading_more = true;
                        }
                    });
                });
        });
    }

    pub fn render_main_panel(
        &mut self,
        ui: &mut egui::Ui,
        app_state: &mut BingtrayAppState,
    ) {
        // Note: Main panel image is now set directly when carousel image is selected
        
        if let Some(main_image) = app_state.main_panel_image.clone() {
            let available_rect = ui.available_rect_before_wrap();
            
            let image_url = &main_image.full_url;
            let image = Image::from_uri(image_url.clone())
                .fit_to_exact_size(available_rect.size());
            
            let image_response = ui.add(image);
            self.image_display_rect = Some(image_response.rect);
            
            if self.reset_rectangle_for_new_image {
                let screen_size = Vec2::new(
                    app_state.get_actual_screen_size().0,
                    app_state.get_actual_screen_size().1,
                );
                self.initialize_rectangle_for_image(image_response.rect, screen_size);
            }
            
            self.render_square_shape(ui, image_response.rect);
            
            ui.separator();
            
            ui.horizontal(|ui| {
                ui.label(&main_image.copyright);
                if ui.link(&main_image.copyright_link).clicked() {
                    if let Err(e) = webbrowser::open(&main_image.copyright_link) {
                        error!("Failed to open copyright link: {}", e);
                    }
                }
            });
            
            if ui.button("Set as Wallpaper").clicked() {
                if let Some(ref image_bytes) = main_image.image_bytes {
                    match app_state.set_wallpaper_from_bytes(image_bytes) {
                        Ok(success) => {
                            if success {
                                app_state.wallpaper_status = Some("Wallpaper set successfully".to_string());
                                info!("Wallpaper set successfully for image: {}", main_image.title);
                            } else {
                                app_state.wallpaper_status = Some("Failed to set wallpaper".to_string());
                                error!("Failed to set wallpaper for image: {}", main_image.title);
                            }
                        }
                        Err(e) => {
                            app_state.wallpaper_status = Some(format!("Error setting wallpaper: {}", e));
                            error!("Error setting wallpaper for image {}: {}", main_image.title, e);
                        }
                    }
                    app_state.wallpaper_start_time = Some(std::time::SystemTime::now());
                }
            }
        }
        
        if let Some(ref status) = app_state.wallpaper_status {
            ui.label(status);
            
            if let Some(start_time) = app_state.wallpaper_start_time {
                if start_time.elapsed().unwrap_or_default().as_secs() > 3 {
                    app_state.wallpaper_status = None;
                    app_state.wallpaper_start_time = None;
                }
            }
        }
    }
}
use egui::{
    Color32, Context, Frame, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2,
    Window, emath, pos2,
};
use egui::epaint::StrokeKind;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct ScreenShapeWidget {
    /// Screen aspect ratio
    screen_ratio: f32,
    /// Current size factor (can be adjusted by wheel or drag)
    size_factor: f32,
    /// Corner positions for dragging
    corners: [Pos2; 4],
    /// Stroke for the square outline
    stroke: Stroke,
    /// Fill for the square
    fill: Color32,
    /// Center position
    center: Pos2,
    /// Dragging state
    dragging_corner: Option<usize>,
}

impl Default for ScreenShapeWidget {
    fn default() -> Self {
        // Default to common screen ratio (will be updated when UI is available)
        let screen_ratio = 16.0 / 9.0; // Common widescreen ratio
        let initial_size = 200.0;
        
        // Calculate square dimensions based on screen ratio
        let square_width = 1920.0;
        let square_height = 1080.0;

        let center = pos2(300.0, 200.0);
        let half_width = square_width / 2.0;
        let half_height = square_height / 2.0;
        
        let corners = [
            pos2(center.x - half_width, center.y - half_height), // Top-left
            pos2(center.x + half_width, center.y - half_height), // Top-right
            pos2(center.x + half_width, center.y + half_height), // Bottom-right
            pos2(center.x - half_width, center.y + half_height), // Bottom-left
        ];
        
        Self {
            screen_ratio,
            size_factor: 1.0,
            corners,
            stroke: Stroke::new(2.0, Color32::from_rgb(25, 200, 100)),
            fill: Color32::from_rgb(50, 100, 150).linear_multiply(0.25),
            center,
            dragging_corner: None,
        }
    }
}

impl ScreenShapeWidget {
    pub fn ui_control(&mut self, ui: &mut egui::Ui) {
        // Update screen ratio from UI context
        let screen_rect = ui.ctx().screen_rect();
        let new_screen_ratio = screen_rect.width() / screen_rect.height();
        if (new_screen_ratio - self.screen_ratio).abs() > 0.01 {
            self.screen_ratio = new_screen_ratio;
            self.update_corners();
        }
        
        ui.collapsing("Screen Shape Controls", |ui| {
            ui.label(format!("Screen Size: {:.0}x{:.0}", screen_rect.width(), screen_rect.height()));
            ui.label(format!("Screen Ratio: {:.2}", self.screen_ratio));
            ui.label(format!("Size Factor: {:.2}", self.size_factor));
            
            ui.separator();
            
            ui.label("Appearance:");
            ui.add(&mut self.stroke);
            ui.color_edit_button_srgba(&mut self.fill);
            
            if ui.button("Reset Size").clicked() {
                self.size_factor = 1.0;
                self.update_corners();
            }
            
            if ui.button("Center Shape").clicked() {
                self.center = pos2(300.0, 200.0);
                self.update_corners();
            }
        });
    }
    
    fn update_corners(&mut self) {
        let base_size = 200.0 * self.size_factor;
        let square_width = base_size;
        let square_height = base_size / self.screen_ratio;
        
        let half_width = square_width / 2.0;
        let half_height = square_height / 2.0;
        
        self.corners = [
            pos2(self.center.x - half_width, self.center.y - half_height), // Top-left
            pos2(self.center.x + half_width, self.center.y - half_height), // Top-right
            pos2(self.center.x + half_width, self.center.y + half_height), // Bottom-right
            pos2(self.center.x - half_width, self.center.y + half_height), // Bottom-left
        ];
    }

    pub fn ui_content(&mut self, ui: &mut Ui) -> egui::Response {
        let (response, painter) =
            ui.allocate_painter(Vec2::new(ui.available_width(), 400.0), Sense::hover());

        let to_screen = emath::RectTransform::from_to(
            Rect::from_min_size(Pos2::ZERO, response.rect.size()),
            response.rect,
        );

        // Handle mouse wheel for size adjustment
        let events = ui.ctx().input(|i| i.events.clone());
        for event in &events {
            match event {
                egui::Event::MouseWheel { delta, .. } => {
                    let zoom = delta.y as f32;
                    if zoom.abs() > 0.0001 {
                        self.size_factor = (self.size_factor + zoom * 0.1).max(0.1).min(5.0);
                        self.update_corners();
                        ui.ctx().request_repaint();
                    }
                }
                _ => {}
            }
        }

        let corner_radius = 6.0;
        let mut corner_shapes = Vec::new();
        
        // Handle corner dragging
        for (i, corner) in self.corners.iter_mut().enumerate() {
            let corner_in_screen = to_screen.transform_pos(*corner);
            let corner_rect = Rect::from_center_size(corner_in_screen, Vec2::splat(2.0 * corner_radius));
            let corner_id = response.id.with(i);
            let corner_response = ui.interact(corner_rect, corner_id, Sense::drag());

            if corner_response.drag_started() {
                self.dragging_corner = Some(i);
            }
            
            if corner_response.dragged() && self.dragging_corner == Some(i) {
                // Calculate new size based on drag distance from center
                let center_in_screen = to_screen.transform_pos(self.center);
                let drag_pos = corner_response.interact_pointer_pos().unwrap_or(corner_in_screen);
                let distance_from_center = (drag_pos - center_in_screen).length();
                let base_distance = 200.0; // Base size when size_factor = 1.0
                
                self.size_factor = (distance_from_center / base_distance).max(0.1).min(5.0);
                self.update_corners();
            }
            
            if corner_response.drag_stopped() {
                self.dragging_corner = None;
            }

            *corner = to_screen.from().clamp(*corner);
            let corner_in_screen = to_screen.transform_pos(*corner);
            let stroke = ui.style().interact(&corner_response).fg_stroke;

            corner_shapes.push(Shape::circle_stroke(corner_in_screen, corner_radius, stroke));
        }

        // Draw the rectangle maintaining screen ratio
        let corners_in_screen: Vec<Pos2> = self.corners
            .iter()
            .map(|p| to_screen.transform_pos(*p))
            .collect();

        // Create rectangle shape
        if corners_in_screen.len() == 4 {
            let rect = Rect::from_two_pos(corners_in_screen[0], corners_in_screen[2]);
            painter.add(Shape::rect_filled(rect, 0.0, self.fill));
            painter.add(Shape::rect_stroke(rect, 0.0, self.stroke, StrokeKind::Outside));
        }

        // Draw corner handles
        painter.extend(corner_shapes);

        // Draw info text
        let info_text = format!(
            "Screen Ratio: {:.2}, Size Factor: {:.2}\nUse mouse wheel to resize, drag corners to scale",
            self.screen_ratio, self.size_factor
        );
        painter.text(
            response.rect.left_top() + Vec2::new(10.0, 10.0),
            egui::Align2::LEFT_TOP,
            info_text,
            egui::FontId::default(),
            Color32::WHITE,
        );

        response
    }
}

impl crate::Demo for ScreenShapeWidget {
    fn name(&self) -> &'static str {
        "📐 Screen Shape"
    }

    fn show(&mut self, ctx: &Context, open: &mut bool) {
        use crate::View as _;
        Window::new(self.name())
            .open(open)
            .vscroll(false)
            .resizable(true)
            .default_size([600.0, 500.0])
            .show(ctx, |ui| self.ui(ui));
    }
}

impl crate::View for ScreenShapeWidget {
    fn ui(&mut self, ui: &mut Ui) {
        self.ui_control(ui);
        ui.separator();
        
        Frame::canvas(ui.style()).show(ui, |ui| {
            self.ui_content(ui);
        });
    }
}

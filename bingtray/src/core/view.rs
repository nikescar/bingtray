// View-related utilities and helpers for egui operations

use egui::{Pos2, Rect, Vec2};

/// Helper for calculating screen ratio and rectangle positioning
pub fn calculate_screen_rectangle(screen_width: f32, screen_height: f32, scale_factor: f32) -> [Pos2; 4] {
    let rect_width = screen_width * scale_factor;
    let rect_height = screen_height * scale_factor;
    
    // Center the rectangle
    let center = Pos2::new(400.0, 300.0);
    let half_width = rect_width / 2.0;
    let half_height = rect_height / 2.0;
    
    [
        Pos2::new(center.x - half_width, center.y - half_height), // Top-left
        Pos2::new(center.x + half_width, center.y - half_height), // Top-right
        Pos2::new(center.x + half_width, center.y + half_height), // Bottom-right
        Pos2::new(center.x - half_width, center.y + half_height), // Bottom-left
    ]
}

/// Helper for drawing selection rectangle on images
pub fn draw_selection_rectangle(ui: &mut egui::Ui, corners: &[Pos2; 4]) {
    let painter = ui.painter();
    
    // Draw the rectangle outline
    for i in 0..4 {
        let start = corners[i];
        let end = corners[(i + 1) % 4];
        painter.line_segment(
            [start, end],
            egui::Stroke::new(2.0, egui::Color32::WHITE),
        );
    }
    
    // Draw corner handles
    for &corner in corners {
        painter.circle_filled(corner, 6.0, egui::Color32::WHITE);
        painter.circle_stroke(corner, 6.0, egui::Stroke::new(2.0, egui::Color32::BLACK));
    }
}

/// Helper for image display and cropping calculations
pub fn calculate_image_crop_rect(
    image_rect: Rect,
    selection_corners: &[Pos2; 4],
) -> Option<Rect> {
    // Find bounding box of selection corners
    let min_x = selection_corners.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let max_x = selection_corners.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
    let min_y = selection_corners.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    let max_y = selection_corners.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
    
    let selection_rect = Rect::from_min_max(
        Pos2::new(min_x, min_y),
        Pos2::new(max_x, max_y)
    );
    
    // Convert to image coordinates
    let intersection = image_rect.intersect(selection_rect);
    if intersection.is_positive() {
        // Convert to normalized coordinates (0.0 to 1.0)
        let norm_x = (intersection.min.x - image_rect.min.x) / image_rect.width();
        let norm_y = (intersection.min.y - image_rect.min.y) / image_rect.height();
        let norm_width = intersection.width() / image_rect.width();
        let norm_height = intersection.height() / image_rect.height();
        
        Some(Rect::from_min_size(
            Pos2::new(norm_x, norm_y),
            Vec2::new(norm_width, norm_height)
        ))
    } else {
        None
    }
}

/// Helper for handling corner dragging interactions
pub fn handle_corner_dragging(
    response: &egui::Response,
    corners: &mut [Pos2; 4],
    dragging_corner: &mut Option<usize>,
) {
    if response.dragged() {
        if let Some(pointer_pos) = response.hover_pos() {
            if dragging_corner.is_none() {
                // Find which corner is being dragged
                for (i, &corner) in corners.iter().enumerate() {
                    if (pointer_pos - corner).length() < 10.0 {
                        *dragging_corner = Some(i);
                        break;
                    }
                }
            }
            
            // Update the corner position
            if let Some(corner_idx) = *dragging_corner {
                corners[corner_idx] = pointer_pos;
            }
        }
    } else {
        *dragging_corner = None;
    }
}

/// Helper for centering rectangle on image
pub fn center_rectangle_on_image(
    image_rect: Rect,
    screen_ratio: f32,
    scale_factor: f32
) -> [Pos2; 4] {
    let center = image_rect.center();
    let rect_width = image_rect.width() * scale_factor;
    let rect_height = rect_width / screen_ratio;
    
    let half_width = rect_width / 2.0;
    let half_height = rect_height / 2.0;
    
    [
        Pos2::new(center.x - half_width, center.y - half_height),
        Pos2::new(center.x + half_width, center.y - half_height),
        Pos2::new(center.x + half_width, center.y + half_height),
        Pos2::new(center.x - half_width, center.y + half_height),
    ]
}

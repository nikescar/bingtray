//! Tray module tests

#[cfg(target_os = "linux")]
mod backend_xembed_tests {
    use image::{Rgba, RgbaImage};

    #[test]
    #[ignore] // Requires X11 display
    fn test_atoms_new_interns_required_atoms() {
        use x11rb::rust_connection::RustConnection;

        let (conn, screen_num) = RustConnection::connect(None)
            .expect("X11 not available - run with DISPLAY set or xvfb-run");

        let atoms = bingtray::tray::backend_xembed::Atoms::new(&conn, screen_num)
            .expect("Failed to intern atoms");

        // Verify atoms are non-zero (successfully interned)
        assert_ne!(atoms.tray_selection, 0);
        assert_ne!(atoms.tray_opcode, 0);
        assert_ne!(atoms.xembed_info, 0);
    }

    #[test]
    fn test_rgba_to_x11_format_converts_bgra() {
        let rgba = RgbaImage::from_pixel(1, 1, Rgba([255, 128, 64, 32]));
        let x11_data = bingtray::tray::backend_xembed::rgba_to_x11_format(&rgba);

        // X11 expects BGRA format
        assert_eq!(x11_data.len(), 4);
        assert_eq!(x11_data[0], 64);  // B
        assert_eq!(x11_data[1], 128); // G
        assert_eq!(x11_data[2], 255); // R
        assert_eq!(x11_data[3], 32);  // A
    }

    #[test]
    fn test_rgba_to_x11_format_multiple_pixels() {
        let mut rgba = RgbaImage::new(2, 1);
        rgba.put_pixel(0, 0, Rgba([255, 0, 0, 255])); // Red
        rgba.put_pixel(1, 0, Rgba([0, 255, 0, 255])); // Green

        let x11_data = bingtray::tray::backend_xembed::rgba_to_x11_format(&rgba);

        assert_eq!(x11_data.len(), 8);
        // First pixel: red -> BGRA
        assert_eq!(x11_data[0], 0);   // B
        assert_eq!(x11_data[1], 0);   // G
        assert_eq!(x11_data[2], 255); // R
        assert_eq!(x11_data[3], 255); // A
        // Second pixel: green -> BGRA
        assert_eq!(x11_data[4], 0);   // B
        assert_eq!(x11_data[5], 255); // G
        assert_eq!(x11_data[6], 0);   // R
        assert_eq!(x11_data[7], 255); // A
    }
}

#[cfg(target_os = "linux")]
mod menu_popup_tests {
    use bingtray::tray::menu_popup::{Rect, MenuItem, MenuAction, calculate_menu_size};

    #[test]
    fn test_rect_contains_point_inside() {
        let rect = Rect {
            x: 10,
            y: 20,
            width: 100,
            height: 50,
        };

        assert!(rect.contains(50, 30));  // Center
        assert!(rect.contains(10, 20));  // Top-left corner
        assert!(rect.contains(109, 69)); // Bottom-right (just inside)
    }

    #[test]
    fn test_rect_contains_point_outside() {
        let rect = Rect {
            x: 10,
            y: 20,
            width: 100,
            height: 50,
        };

        assert!(!rect.contains(5, 30));   // Left of rect
        assert!(!rect.contains(150, 30)); // Right of rect
        assert!(!rect.contains(50, 10));  // Above rect
        assert!(!rect.contains(50, 100)); // Below rect
    }

    #[test]
    fn test_calculate_menu_size_single_item() {
        let items = vec![MenuItem::new(MenuAction::Quit, "Quit", true)];

        let (width, height) = calculate_menu_size(&items);

        assert!(width >= 100); // Minimum width
        assert_eq!(height, 30); // 5px top + 25px item
    }

    #[test]
    fn test_calculate_menu_size_with_separator() {
        let items = vec![
            MenuItem::new(MenuAction::ShowApp, "Show App", true),
            MenuItem::separator(),
            MenuItem::new(MenuAction::Quit, "Quit", true),
        ];

        let (width, height) = calculate_menu_size(&items);

        assert_eq!(height, 30 + 10 + 30); // item + separator + item
    }

    #[test]
    fn test_calculate_menu_size_long_label() {
        let long_label = "This is a very long menu item label that should increase width";
        let items = vec![MenuItem::new(MenuAction::ShowApp, long_label, true)];

        let (width, _) = calculate_menu_size(&items);

        assert!(width > 200); // Should be wider than minimum
    }
}

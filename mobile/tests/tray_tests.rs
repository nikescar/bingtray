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

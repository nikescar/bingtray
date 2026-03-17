//! Desktop entry point for Bingtray
//!
//! Detects whether the application is running in a terminal or GUI environment:
//! - Terminal: Runs CLI mode with CalcBingimage
//! - Non-terminal: Runs GUI mode (or tray mode with --tray flag)

#![cfg(not(target_os = "android"))]
#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use std::io::IsTerminal;

#[cfg(target_os = "windows")]
fn hide_console() {
    use windows_sys::Win32::System::Console::GetConsoleWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};

    unsafe {
        let console_window = GetConsoleWindow();
        if !console_window.is_null() {
            ShowWindow(console_window, SW_HIDE);
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
fn hide_console() {
    // No-op on non-Windows platforms
}

fn main() -> Result<()> {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Bingtray v{} starting...", env!("CARGO_PKG_VERSION"));

    // Check if we're running in terminal mode (before parsing args)
    if std::io::stdout().is_terminal() {
        // Terminal mode - run CLI interface
        log::info!("Running in CLI mode (terminal detected)");

        // Initialize i18n for CLI
        if let Err(e) = bingtray::i18n::init_i18n("Auto") {
            log::error!("Failed to initialize i18n: {}", e);
        }

        let mut logic = bingtray::calc_bingimage::CalcBingimage::new()?;
        logic.initialize()?;

        bingtray::cli::run_cli_mode(&mut logic)?;
    } else {
        // Non-terminal mode - parse arguments to determine GUI vs tray mode

        // Initialize i18n EARLY (before tray or GUI)
        // This ensures translations work in both tray menu and GUI
        if let Err(e) = bingtray::i18n::init_i18n("Auto") {
            log::error!("Failed to initialize i18n: {}", e);
        }

        // Parse command-line arguments
        let args: Vec<String> = std::env::args().collect();
        let force_tray = args.iter().any(|arg| arg == "--tray");

        if force_tray {
            // Tray mode (forced by --tray flag)
            log::info!("Running in tray mode");

            // Hide console window on Windows
            #[cfg(target_os = "windows")]
            hide_console();

            // Run tray mode, which may request to open GUI
            loop {
                match bingtray::tray::run_tray_mode()? {
                    bingtray::tray::TrayExitAction::Quit => {
                        log::info!("Exiting application");
                        break;
                    }
                    bingtray::tray::TrayExitAction::OpenGui => {
                        log::info!("Opening GUI window");
                        run_gui_mode()?;
                        log::info!("GUI closed, returning to tray mode");
                    }
                }
            }
        } else {
            // GUI mode (no terminal attached)
            log::info!("Running in GUI mode (no terminal detected)");

            // Hide console window on Windows
            #[cfg(target_os = "windows")]
            hide_console();

            run_gui_mode()?;
        }
    }

    Ok(())
}

fn run_gui_mode() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([800.0, 1200.0])
            .with_title("Bingtray - Bing Wallpaper Manager"),
        ..Default::default()
    };

    eframe::run_native(
        "Bingtray",
        options,
        Box::new(|cc| {
            log::info!("Creating BingtrayApp instance");

            // Load Material3 fonts and theme
            use egui_material3::theme::{
                load_fonts, load_themes,
                setup_local_fonts_from_bytes, setup_local_theme,
            };
            // Prepare local fonts including Material Symbols (using include_bytes!)
            setup_local_fonts_from_bytes(
                "MaterialSymbolsOutlined",
                include_bytes!("../resources/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].ttf"),
            );
            setup_local_fonts_from_bytes("NotoSansKr", include_bytes!("../resources/noto-sans-kr.ttf"));

            // Register Korean font with egui for proper text rendering
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "NotoSansKr".to_owned(),
                std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../resources/noto-sans-kr.ttf"))),
            );
            // Put Korean font first in proportional and monospace families
            fonts.families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "NotoSansKr".to_owned());
            fonts.families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("NotoSansKr".to_owned());
            cc.egui_ctx.set_fonts(fonts);

            // Prepare themes from build-time constants
            setup_local_theme(None);
            // Install image loaders
            egui_extras::install_image_loaders(&cc.egui_ctx);
            // Load all prepared fonts and themes
            load_fonts(&cc.egui_ctx);
            load_themes();

            // Note: i18n is already initialized in main() before GUI starts

            Ok(Box::<bingtray::bingtray::BingtrayApp>::default())
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))?;

    Ok(())
}

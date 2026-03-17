//! Desktop entry point for Bingtray
//!
//! Mode selection logic:
//! - `--gui` flag: Runs GUI mode
//! - `--tray` flag: Runs tray mode
//! - Terminal detected: Runs CLI mode
//! - Default (double-click): Runs tray mode
//!
//! Logging options:
//! - `--log [FILE]`: Enable file logging (defaults to bingtray.log if FILE not specified)

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#![cfg(not(target_os = "android"))]
#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use std::io::IsTerminal;
use std::io::Write;

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
    // Parse command-line arguments FIRST to check for explicit mode flags and log configuration
    let args: Vec<String> = std::env::args().collect();

    // Check for --log argument
    let log_file = {
        let mut log_file_opt: Option<String> = None;
        let mut i = 0;
        while i < args.len() {
            if args[i] == "--log" {
                // Check if there's a value after --log
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    log_file_opt = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    // --log flag present but no value, use default
                    log_file_opt = Some("bingtray.log".to_string());
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        log_file_opt
    };

    // Initialize logger
    if let Some(log_path) = &log_file {
        // File-based logging
        let log_path = if log_path.is_empty() {
            "bingtray.log"
        } else {
            log_path.as_str()
        };

        let target = Box::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
                .unwrap_or_else(|e| {
                    eprintln!("Failed to open log file '{}': {}", log_path, e);
                    std::process::exit(1);
                })
        );

        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
            .target(env_logger::Target::Pipe(target))
            .init();
    } else {
        // Console logging (default)
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }

    log::info!("Bingtray v{} starting...", env!("CARGO_PKG_VERSION"));

    // Initialize i18n EARLY (before any mode starts)
    if let Err(e) = bingtray::i18n::init_i18n("Auto") {
        log::error!("Failed to initialize i18n: {}", e);
    }

    // Initialize global tray event handlers (one-time setup)
    bingtray::tray::init_tray_event_handlers();

    let force_gui = args.iter().any(|arg| arg == "--gui");
    let force_tray = args.iter().any(|arg| arg == "--tray");

    if force_gui {
        // GUI mode (explicitly requested via --gui flag)
        log::info!("Running in GUI mode (--gui flag)");

        // Hide console window on Windows
        #[cfg(target_os = "windows")]
        hide_console();

        run_gui_mode()?;
    } else if force_tray {
        // Tray mode (explicitly requested via --tray flag)
        log::info!("Running in tray mode (--tray flag)");

        // Hide console window on Windows
        #[cfg(target_os = "windows")]
        hide_console();

        // Run tray mode
        loop {
            log::info!("*** Calling run_tray_mode() ***");
            match bingtray::tray::run_tray_mode()? {
                bingtray::tray::TrayExitAction::Quit => {
                    log::info!("*** Received Quit action, exiting application ***");
                    break;
                }
                bingtray::tray::TrayExitAction::OpenGui => {
                    log::info!("*** Received OpenGui action, opening GUI window on main thread ***");
                    run_gui_mode()?;
                    log::info!("*** GUI closed, will return to tray mode ***");
                }
            }
        }
    } else if std::io::stdout().is_terminal() {
        // Terminal mode - run CLI interface
        log::info!("Running in CLI mode (terminal detected)");

        let mut logic = bingtray::calc_bingimage::CalcBingimage::new()?;
        logic.initialize()?;

        bingtray::cli::run_cli_mode(&mut logic)?;
    } else {
        // Tray mode (default for double-click on Windows - no terminal, no flags)
        log::info!("Running in tray mode (default - no terminal detected)");

        // Hide console window on Windows
        #[cfg(target_os = "windows")]
        hide_console();

        // Run tray mode
        loop {
            log::info!("*** Calling run_tray_mode() ***");
            match bingtray::tray::run_tray_mode()? {
                bingtray::tray::TrayExitAction::Quit => {
                    log::info!("*** Received Quit action, exiting application ***");
                    break;
                }
                bingtray::tray::TrayExitAction::OpenGui => {
                    log::info!("*** Received OpenGui action, opening GUI window on main thread ***");
                    run_gui_mode()?;
                    log::info!("*** GUI closed, will return to tray mode ***");
                }
            }
        }
    }

    Ok(())
}

fn run_gui_mode() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([800.0, 1200.0])
            .with_title("Bingtray - Bing Wallpaper Manager")
            .with_icon(
                eframe::icon_data::from_png_bytes(
                    include_bytes!("../../imgs/logo110.png") // Path to your icon
                )
                .expect("Failed to load icon"),
            ),
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

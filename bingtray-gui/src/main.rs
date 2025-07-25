#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::Result;
use bingcli::BingCliApp;
use clap::Parser;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder, TrayIconEvent,
};

#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    System::Console::{AllocConsole, GetConsoleWindow},
    UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOW},
};

#[derive(Parser)]
#[command(name = "bingtray-gui")]
#[command(about = "BingTray - Bing Wallpaper Manager with GUI")]
#[command(version)]
struct Cli {
    /// Run in CLI mode (text-based interface)
    #[arg(long)]
    cli: bool,
    
    /// Show debug console (Windows only)
    #[arg(long)]
    debug: bool,
}

enum UserEvent {
    TrayIconEvent(tray_icon::TrayIconEvent),
    MenuEvent(tray_icon::menu::MenuEvent),
}

struct BingTrayApp {
    cli_app: BingCliApp,
}

impl BingTrayApp {
    fn new() -> Result<Self> {
        let cli_app = BingCliApp::new()?;
        Ok(Self { cli_app })
    }
    
    fn initialize(&mut self) -> Result<()> {
        self.cli_app.initialize()
    }
    
    fn set_next_market_wallpaper(&mut self) -> Result<bool> {
        self.cli_app.set_next_market_wallpaper()
    }
    
    fn get_current_image_title(&self) -> String {
        self.cli_app.get_current_image_title()
    }
    
    fn keep_current_image(&mut self) -> Result<()> {
        self.cli_app.keep_current_image()
    }
    
    fn blacklist_current_image(&mut self) -> Result<()> {
        self.cli_app.blacklist_current_image()
    }
    
    fn set_kept_wallpaper(&mut self) -> Result<bool> {
        self.cli_app.set_kept_wallpaper()
    }

    fn has_next_market_wallpaper_available(&self) -> bool {
        self.cli_app.has_next_market_wallpaper_available()
    }

    fn can_keep_current_image(&self) -> bool {
        self.cli_app.can_keep_current_image()
    }

    fn can_blacklist_current_image(&self) -> bool {
        self.cli_app.can_blacklist_current_image()
    }

    fn has_kept_wallpapers_available(&self) -> bool {
        self.cli_app.has_kept_wallpapers_available()
    }

    fn has_unprocessed_files(&self) -> bool {
        self.cli_app.has_unprocessed_files()
    }

    fn is_current_image_in_favorites(&self) -> bool {
        self.cli_app.is_current_image_in_favorites()
    }

    fn get_status_info(&self) -> (String, String, usize) {
        let title = self.get_current_image_title();
        let (last_tried, available_count) = self.cli_app.get_market_status();
        (title, last_tried, available_count)
    }
    
    fn show_menu(&self) {
        self.cli_app.show_menu()
    }
    
    fn run_cli_mode(&mut self) -> Result<()> {
        self.cli_app.run()
    }
    
    fn get_current_image_copyright(&self) -> (String, String) {
        self.cli_app.get_current_image_copyright()
    }
    
    fn open_cache_directory(&self) -> Result<()> {
        self.cli_app.open_cache_directory()
    }
}

fn create_tray_menu(app: &BingTrayApp) -> (Menu, Vec<tray_icon::menu::MenuId>, Option<String>) {
    let tray_menu = Menu::new();
    let (title, last_tried, available_count) = app.get_status_info();
    let (copyright_text, copyrightlink) = app.get_current_image_copyright();
    
    // Create info items (non-clickable)
    let info_item = MenuItem::new(
        format!("Current: {}", title), 
        false, 
        None
    );
    
    // Make copyright item clickable if there's a link
    let has_copyright_link = !copyrightlink.is_empty() && copyrightlink != "(no copyright info)";
    let copyright_item = MenuItem::new(
        if copyright_text.len() > 50 {
            format!("{}...", &copyright_text[..47])
        } else {
            format!("{}", copyright_text)
        },
        has_copyright_link, 
        None
    );
    
    let status_item = MenuItem::new(
        format!("Last: {} | Available: {}", last_tried, available_count), 
        false, 
        None
    );
    
    // Create action menu items (clickable) - matching bingcli menu structure
    let has_next_available = app.has_next_market_wallpaper_available();
    let can_keep = app.can_keep_current_image();
    let can_blacklist = app.can_blacklist_current_image();
    let has_kept_available = app.has_kept_wallpapers_available();
    
    let cache_item = MenuItem::new("0. Cache Dir Contents", true, None);
    let next_item = MenuItem::new(
        if has_next_available {
            "1. Next Market wallpaper".to_string()
        } else {
            "1. Next Market wallpaper (unavailable)".to_string()
        }, 
        has_next_available, 
        None
    );
    let keep_item = MenuItem::new(
        if can_keep {
            format!("2. Keep \"{}\"", title)
        } else {
            format!("2. Keep \"{}\" (unavailable)", title)
        }, 
        can_keep, 
        None
    );
    let blacklist_item = MenuItem::new(
        if can_blacklist {
            format!("3. Blacklist \"{}\"", title)
        } else {
            format!("3. Blacklist \"{}\" (unavailable)", title)
        }, 
        can_blacklist, 
        None
    );
    let kept_item = MenuItem::new(
        if has_kept_available {
            "4. Next Kept wallpaper".to_string()
        } else {
            "4. Next Kept wallpaper (unavailable)".to_string()
        }, 
        has_kept_available, 
        None
    );
    let quit_item = MenuItem::new("5. Exit", true, None);

    // Store the menu item IDs in consistent order matching bingcli (0-5)
    let menu_item_ids = if has_copyright_link {
        vec![
            copyright_item.id().clone(),  // Special case: copyright link (not in CLI)
            cache_item.id().clone(),      // 0. Cache Dir Contents
            next_item.id().clone(),       // 1. Next Market wallpaper
            keep_item.id().clone(),       // 2. Keep current image
            blacklist_item.id().clone(),  // 3. Blacklist current image
            kept_item.id().clone(),       // 4. Next Kept wallpaper
            quit_item.id().clone(),       // 5. Exit
        ]
    } else {
        vec![
            cache_item.id().clone(),      // 0. Cache Dir Contents
            next_item.id().clone(),       // 1. Next Market wallpaper
            keep_item.id().clone(),       // 2. Keep current image
            blacklist_item.id().clone(),  // 3. Blacklist current image
            kept_item.id().clone(),       // 4. Next Kept wallpaper
            quit_item.id().clone(),       // 5. Exit
        ]
    };

    tray_menu.append_items(&[
        &info_item,
        &copyright_item,
        &status_item,
        &PredefinedMenuItem::separator(),
        &cache_item,
        &next_item,
        &keep_item,
        &blacklist_item,
        &kept_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ]).expect("Failed to append menu items");

    let copyright_link = if has_copyright_link { Some(copyrightlink) } else { None };
    (tray_menu, menu_item_ids, copyright_link)
}

fn update_tray_menu(tray_icon: &tray_icon::TrayIcon, app: &BingTrayApp, menu_items: &mut Vec<tray_icon::menu::MenuId>, copyright_link: &mut Option<String>) {
    let (new_menu, new_menu_ids, new_copyright_link) = create_tray_menu(app);
    *menu_items = new_menu_ids;
    *copyright_link = new_copyright_link;
    tray_icon.set_menu(Some(Box::new(new_menu)));
}

fn load_icon() -> tray_icon::Icon {
    // Create a simple icon programmatically since we don't have an icon file
    let icon_size = 32;
    let mut icon_rgba = vec![0u8; (icon_size * icon_size * 4) as usize];
    
    // Create a simple pattern - blue background with white "B"
    for y in 0..icon_size {
        for x in 0..icon_size {
            let idx = ((y * icon_size + x) * 4) as usize;
            
            // Background: blue
            icon_rgba[idx] = 0;     // R
            icon_rgba[idx + 1] = 100; // G
            icon_rgba[idx + 2] = 200; // B
            icon_rgba[idx + 3] = 255; // A
            
            // Draw a simple "B" pattern in white
            if (x >= 8 && x <= 10) || 
               (y >= 8 && y <= 10 && x >= 8 && x <= 20) ||
               (y >= 15 && y <= 17 && x >= 8 && x <= 18) ||
               (y >= 22 && y <= 24 && x >= 8 && x <= 20) ||
               (x >= 18 && x <= 20 && ((y >= 11 && y <= 14) || (y >= 18 && y <= 21))) {
                icon_rgba[idx] = 255;     // R
                icon_rgba[idx + 1] = 255; // G
                icon_rgba[idx + 2] = 255; // B
                icon_rgba[idx + 3] = 255; // A
            }
        }
    }
    
    tray_icon::Icon::from_rgba(icon_rgba, icon_size, icon_size)
        .expect("Failed to create icon")
}

#[cfg(target_os = "windows")]
fn hide_console() {
    unsafe {
        let console_window = GetConsoleWindow();
        if console_window != std::ptr::null_mut() {
            ShowWindow(console_window, SW_HIDE);
        }
    }
}

#[cfg(target_os = "windows")]
fn show_console() {
    unsafe {
        let console_window = GetConsoleWindow();
        if console_window != std::ptr::null_mut() {
            ShowWindow(console_window, SW_SHOW);
        } else {
            // If no console exists, allocate one
            AllocConsole();
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn hide_console() {
    // No-op on non-Windows platforms
}

#[cfg(not(target_os = "windows"))]
fn show_console() {
    // No-op on non-Windows platforms
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Handle console visibility on Windows
    #[cfg(target_os = "windows")]
    {
        if cli.debug || cli.cli {
            show_console();
        } else {
            hide_console();
        }
    }
    
    // Initialize app but don't run initialize() yet - we'll do that after tray icon is created
    let mut app = BingTrayApp::new()?;
    // app.initialize()?;
    
    // Check if CLI mode is requested
    if cli.cli {
        println!("BingTray CLI mode started successfully!");
        return app.run_cli_mode();
    }
    
    // Otherwise run GUI mode
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // Set up event handlers
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::TrayIconEvent(event));
    }));

    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::MenuEvent(event));
    }));
    
    if cli.debug {
        println!("BingTray GUI started successfully!");
    }
    
    let mut tray_icon = None;
    let mut menu_items = Vec::new();
    let mut copyright_link: Option<String> = None;
    let mut app_initialized = false;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(tao::event::StartCause::Init) => {
                let icon = load_icon();
                let (tray_menu, menu_ids, link) = create_tray_menu(&app);
                
                // Store the menu item IDs and copyright link
                menu_items = menu_ids;
                copyright_link = link;

                tray_icon = Some(
                    TrayIconBuilder::new()
                        .with_menu(Box::new(tray_menu))
                        .with_tooltip("BingTray - Bing Wallpaper Manager")
                        .with_icon(icon)
                        .build()
                        .unwrap(),
                );

                // Initialize app after tray icon is created
                if !app_initialized {
                    app_initialized = true;
                    println!("Tray icon created, starting background initialization...");
                    
                    // Initialize the app
                    if let Err(e) = app.initialize() {
                        eprintln!("Failed to initialize app: {}", e);
                    }
                }

                // Request redraw for macOS
                #[cfg(target_os = "macos")]
                unsafe {
                    use objc2_core_foundation::CFRunLoop;
                    if let Some(rl) = CFRunLoop::main() {
                        rl.wake_up();
                    }
                }
            }

            Event::UserEvent(UserEvent::TrayIconEvent(event)) => {
                println!("Tray event: {:?}", event);
            }

            Event::UserEvent(UserEvent::MenuEvent(event)) => {
                println!("Menu event: {:?}", event);

                if !menu_items.is_empty() {
                    // Determine the offset based on whether copyright link is present
                    let has_copyright_link = copyright_link.is_some();
                    let offset = if has_copyright_link { 1 } else { 0 };
                    
                    if has_copyright_link && event.id == menu_items[0] {
                        // Copyright link clicked - open URL
                        if let Some(ref link) = copyright_link {
                            println!("Opening copyright link: {}", link);
                            // Try to open the URL
                            #[cfg(target_os = "linux")]
                            {
                                let _ = std::process::Command::new("xdg-open").arg(link).spawn();
                            }
                            #[cfg(target_os = "windows")]
                            {
                                let _ = std::process::Command::new("cmd").args(&["/C", "start", link]).spawn();
                            }
                            #[cfg(target_os = "macos")]
                            {
                                let _ = std::process::Command::new("open").arg(link).spawn();
                            }
                        }
                    } else if event.id == menu_items[0 + offset] {
                        // 0. Cache Dir Contents
                        println!("Executing: Cache Dir Contents");
                        if let Err(e) = app.open_cache_directory() {
                            eprintln!("Failed to open cache directory: {}", e);
                        } else {
                            println!("Cache directory opened in file manager");
                        }
                    } else if event.id == menu_items[1 + offset] {
                        // 1. Next Market wallpaper - only execute if available
                        if app.has_next_market_wallpaper_available() {
                            println!("Executing: Next market wallpaper");
                            if let Err(e) = app.set_next_market_wallpaper() {
                                eprintln!("Failed to set next market wallpaper: {}", e);
                            } else if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &app, &mut menu_items, &mut copyright_link);
                            }
                        } else {
                            println!("Next market wallpaper is not available - no images in unprocessed folder and no available market codes");
                        }
                    } else if event.id == menu_items[2 + offset] {
                        // 2. Keep current image - only execute if available
                        if app.can_keep_current_image() {
                            println!("Executing: Keep current image");
                            if let Err(e) = app.keep_current_image() {
                                eprintln!("Failed to keep image: {}", e);
                            } else if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &app, &mut menu_items, &mut copyright_link);
                            }
                        } else {
                            if !app.has_unprocessed_files() {
                                println!("Keep current image is not available - no files in unprocessed folder");
                            } else if app.get_current_image_title() == "(no image)" {
                                println!("Keep current image is not available - no current image");
                            } else if app.is_current_image_in_favorites() {
                                println!("Keep current image is not available - image is already in favorites");
                            } else {
                                println!("Keep current image is not available");
                            }
                        }
                    } else if event.id == menu_items[3 + offset] {
                        // 3. Blacklist current image - only execute if available
                        if app.can_blacklist_current_image() {
                            println!("Executing: Blacklist current image");
                            if let Err(e) = app.blacklist_current_image() {
                                eprintln!("Failed to blacklist image: {}", e);
                            } else if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &app, &mut menu_items, &mut copyright_link);
                            }
                        } else {
                            if !app.has_unprocessed_files() {
                                println!("Blacklist current image is not available - no files in unprocessed folder");
                            } else if app.get_current_image_title() == "(no image)" {
                                println!("Blacklist current image is not available - no current image");
                            } else {
                                println!("Blacklist current image is not available");
                            }
                        }
                    } else if event.id == menu_items[4 + offset] {
                        // 4. Next kept wallpaper - only execute if available
                        if app.has_kept_wallpapers_available() {
                            println!("Executing: Next kept wallpaper");
                            if let Err(e) = app.set_kept_wallpaper() {
                                eprintln!("Failed to set kept wallpaper: {}", e);
                            } else if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &app, &mut menu_items, &mut copyright_link);
                            }
                        } else {
                            println!("Next kept wallpaper is not available - no kept wallpapers in favorites folder");
                        }
                    } else if event.id == menu_items[5 + offset] {
                        // 5. Exit
                        println!("Executing: Exit");
                        tray_icon.take();
                        *control_flow = ControlFlow::Exit;
                    } else {
                        let lazy_init_done = true;
                        if let Some(ref icon) = tray_icon {
                            update_tray_menu(icon, &app, &mut menu_items, &mut copyright_link);
                        }
                        // Return early to let the user click again after initialization
                        return;
                    }
                }

                if !menu_items.is_empty() {
                    // Check if we have a copyright link and if the first menu item was clicked
                    if copyright_link.is_some() && event.id == menu_items[0] {
                        // Copyright link clicked
                        if let Some(ref link) = copyright_link {
                            println!("Opening copyright link: {}", link);
                            if let Err(e) = open::that(link) {
                                eprintln!("Failed to open copyright link: {}", e);
                            }
                        }
                    } else {
                        // Adjust indices based on whether copyright link is present
                        let offset = if copyright_link.is_some() { 1 } else { 0 };
                        
                        if event.id == menu_items[0 + offset] {
                            // Cache Dir Contents
                            println!("Executing: Cache Dir Contents");
                            if let Err(e) = app.open_cache_directory() {
                                eprintln!("Failed to open cache directory: {}", e);
                            }
                        } else if event.id == menu_items[1 + offset] {
                            // Next market wallpaper - only execute if available
                            if app.has_next_market_wallpaper_available() {
                                println!("Executing: Next market wallpaper");
                                if let Err(e) = app.set_next_market_wallpaper() {
                                    eprintln!("Failed to set next market wallpaper: {}", e);
                                } else if let Some(ref icon) = tray_icon {
                                    update_tray_menu(icon, &app, &mut menu_items, &mut copyright_link);
                                }
                            } else {
                                println!("Next market wallpaper is not available - no images in unprocessed folder and no available market codes");
                            }
                        } else if event.id == menu_items[2 + offset] {
                            // Keep current image - only execute if available
                            if app.can_keep_current_image() {
                                println!("Executing: Keep current image");
                                if let Err(e) = app.keep_current_image() {
                                    eprintln!("Failed to keep image: {}", e);
                                } else if let Some(ref icon) = tray_icon {
                                    update_tray_menu(icon, &app, &mut menu_items, &mut copyright_link);
                                }
                            } else {
                                println!("Keep current image is not available - no current image or image is already in favorites");
                            }
                        } else if event.id == menu_items[3 + offset] {
                            // Blacklist current image - only execute if available
                            if app.can_blacklist_current_image() {
                                println!("Executing: Blacklist current image");
                                if let Err(e) = app.blacklist_current_image() {
                                    eprintln!("Failed to blacklist image: {}", e);
                                } else if let Some(ref icon) = tray_icon {
                                    update_tray_menu(icon, &app, &mut menu_items, &mut copyright_link);
                                }
                            } else {
                                println!("Blacklist current image is not available - no current image");
                            }
                        } else if event.id == menu_items[4 + offset] {
                            // Next kept wallpaper
                            println!("Executing: Next kept wallpaper");
                            if let Err(e) = app.set_kept_wallpaper() {
                                eprintln!("Failed to set kept wallpaper: {}", e);
                            } else if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &app, &mut menu_items, &mut copyright_link);
                            }
                        } else if event.id == menu_items[5 + offset] {
                            // Exit
                            println!("Executing: Exit");
                            tray_icon.take();
                            *control_flow = ControlFlow::Exit;
                        } else {
                            println!("Unknown menu item clicked: {:?}", event.id);
                        }
                    }
                }
            }

            _ => {}
        }
 })
}

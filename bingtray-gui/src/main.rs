use anyhow::Result;
use bingtray_core::*;
use chrono::Utc;
use clap::Parser;
use rand::seq::SliceRandom;
use std::io::{self, Write};
use std::path::PathBuf;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder, TrayIconEvent,
};

#[derive(Parser)]
#[command(name = "bingtray-gui")]
#[command(about = "BingTray - Bing Wallpaper Manager with GUI")]
#[command(version)]
struct Cli {
    /// Run in CLI mode (text-based interface)
    #[arg(long)]
    cli: bool,
}

enum UserEvent {
    TrayIconEvent(tray_icon::TrayIconEvent),
    MenuEvent(tray_icon::menu::MenuEvent),
}

struct BingTrayApp {
    config: Config,
    current_image: Option<PathBuf>,
}

impl BingTrayApp {
    fn new() -> Result<Self> {
        let config = Config::new()?;
        
        Ok(Self {
            config,
            current_image: None,
        })
    }
    
    fn initialize(&mut self) -> Result<()> {
        // Load or create market codes
        let mut market_codes = load_market_codes(&self.config)?;
        
        // Check if we need to download images
        if need_more_images(&self.config)? {
            self.download_new_images(&mut market_codes)?;
        }
        
        // Set initial wallpaper
        self.set_next_wallpaper()?;
        
        Ok(())
    }
    
    fn download_new_images(&self, market_codes: &mut std::collections::HashMap<String, i64>) -> Result<()> {
        let old_codes = get_old_market_codes(market_codes);
        
        if let Some(market_code) = old_codes.choose(&mut rand::thread_rng()) {
            println!("Downloading images for market code: {}", market_code);
            let count = download_images_for_market(&self.config, market_code)?;
            println!("Downloaded {} images", count);
            
            // Update timestamp
            market_codes.insert(market_code.clone(), Utc::now().timestamp());
            save_market_codes(&self.config, market_codes)?;
        }
        
        Ok(())
    }
    
    fn set_next_wallpaper(&mut self) -> Result<bool> {
        if let Some(image_path) = get_next_image(&self.config)? {
            if set_wallpaper(&image_path)? {
                self.current_image = Some(image_path.clone());
                println!("Set wallpaper: {}", image_path.display());
                return Ok(true);
            }
        } else {
            // No images available, try to download more
            let mut market_codes = load_market_codes(&self.config)?;
            let old_codes = get_old_market_codes(&market_codes);
            let mut attempts = 0;
            const MAX_ATTEMPTS: usize = 5;
            
            for market_code in old_codes.iter().take(MAX_ATTEMPTS) {
                // Get images list from this market code without downloading
                if let Ok(images) = get_bing_images(market_code) {
                    // Check if any images are not blacklisted
                    let has_valid_images = images.iter().any(|image| {
                        is_blacklisted(&self.config, &image.hsh).unwrap_or(false) == false
                    });
                    
                    if has_valid_images {
                        // Found a market code with non-blacklisted images, download them
                        println!("Downloading images for market code: {}", market_code);
                        let count = download_images_for_market(&self.config, market_code)?;
                        println!("Downloaded {} images", count);
                        
                        // Update timestamp
                        market_codes.insert(market_code.clone(), Utc::now().timestamp());
                        save_market_codes(&self.config, &market_codes)?;
                        
                        // Try to set wallpaper with newly downloaded images
                        if let Some(image_path) = get_next_image(&self.config)? {
                            if set_wallpaper(&image_path)? {
                                self.current_image = Some(image_path.clone());
                                println!("Set wallpaper: {}", image_path.display());
                                return Ok(true);
                            }
                        }
                        break;
                    } else {
                        println!("All images from market code {} are blacklisted, trying next...", market_code);
                    }
                }
                attempts += 1;
            }
            
            if attempts >= MAX_ATTEMPTS {
                println!("Warning: Could not find any valid images after checking {} market codes", MAX_ATTEMPTS);
            }
        }
        Ok(false)
    }
    
    fn get_current_image_title(&self) -> String {
        if let Some(ref image_path) = self.current_image {
            if let Some(filename) = image_path.file_stem().and_then(|s| s.to_str()) {
                // Extract title from filename (before the hash)
                let title = if let Some(dot_pos) = filename.rfind('.') {
                    &filename[..dot_pos]
                } else {
                    filename
                };
                
                if title.len() > 30 {
                    format!("{}...", &title[..30])
                } else {
                    title.to_string()
                }
            } else {
                "(no image)".to_string()
            }
        } else {
            "(no image)".to_string()
        }
    }
    
    fn keep_current_image(&mut self) -> Result<()> {
        if let Some(ref image_path) = self.current_image.clone() {
            move_to_keepfavorite(&self.config, image_path)?;
            println!("Moved to favorites: {}", image_path.display());
            
            // Check if we need more images after moving this one
            if need_more_images(&self.config)? {
                let mut market_codes = load_market_codes(&self.config)?;
                self.download_new_images(&mut market_codes)?;
            }
            
            self.set_next_wallpaper()?;
        }

        Ok(())
    }
    
    fn blacklist_current_image(&mut self) -> Result<()> {
        if let Some(ref image_path) = self.current_image.clone() {
            blacklist_image(&self.config, image_path)?;
            println!("Blacklisted: {}", image_path.display());
            
            // Check if we need more images after blacklisting this one
            if need_more_images(&self.config)? {
                let mut market_codes = load_market_codes(&self.config)?;
                self.download_new_images(&mut market_codes)?;
            }
            
            self.set_next_wallpaper()?;
        }
        
        Ok(())
    }
    
    fn set_kept_wallpaper(&mut self) -> Result<bool> {
        let entries = std::fs::read_dir(&self.config.keepfavorite_dir)?;
        let images: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_lowercase() == "jpg")
                    .unwrap_or(false)
            })
            .collect();
        
        if images.is_empty() {
            println!("No kept wallpapers available in favorites folder.");
            return Ok(false);
        }
        
        // Use a simple pseudo-random selection based on current time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as usize;
        let index = now % images.len();
        let selected_image = &images[index];
        
        if set_wallpaper(selected_image)? {
            self.current_image = Some(selected_image.clone());
            println!("Set kept wallpaper: {}", selected_image.display());
            return Ok(true);
        }
        
        Ok(false)
    }
    
    fn has_next_wallpaper_available(&self) -> bool {
        // Check if there are images in unprocessed folder
        if let Ok(entries) = std::fs::read_dir(&self.config.unprocessed_dir) {
            let unprocessed_count = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.path().extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.to_lowercase() == "jpg")
                        .unwrap_or(false)
                })
                .count();
            
            if unprocessed_count > 0 {
                return true;
            }
        }
        
        // Check if there are available market codes to download from
        let market_codes = load_market_codes(&self.config).unwrap_or_default();
        let old_codes = get_old_market_codes(&market_codes);
        old_codes.len() > 0
    }

    fn can_keep_current_image(&self) -> bool {
        if let Some(ref image_path) = self.current_image {
            // Check if the image is not already in keepfavorite folder
            !image_path.starts_with(&self.config.keepfavorite_dir)
        } else {
            false // No current image
        }
    }

    fn can_blacklist_current_image(&self) -> bool {
        // Can blacklist if there's a current image (regardless of its location)
        self.current_image.is_some()
    }

    fn get_status_info(&self) -> (String, String, usize) {
        let title = self.get_current_image_title();
        
        // Get market codes info
        let market_codes = load_market_codes(&self.config).unwrap_or_default();
        let old_codes = get_old_market_codes(&market_codes);
        let available_count = old_codes.len();
        
        // Find the most recently used market code (highest timestamp)
        let last_tried = market_codes
            .iter()
            .max_by_key(|(_, &timestamp)| timestamp)
            .map(|(code, _)| code.to_string())
            .unwrap_or_else(|| "none".to_string());
        
        (title, last_tried, available_count)
    }
    
    fn show_menu(&self) {
        let title = self.get_current_image_title();
        
        // Get market codes info
        let market_codes = load_market_codes(&self.config).unwrap_or_default();
        let old_codes = get_old_market_codes(&market_codes);
        let available_count = old_codes.len();
        
        // Find the most recently used market code (highest timestamp)
        let last_tried = market_codes
            .iter()
            .max_by_key(|(_, &timestamp)| timestamp)
            .map(|(code, _)| code.as_str())
            .unwrap_or("none");
        
        println!("\n=== BingTray - Bing Wallpaper Manager ===");
        println!("Current wallpaper: {}", title);
        println!("Last tried market: {} | Available markets: {}", last_tried, available_count);
        println!();
        
        let has_next_available = self.has_next_wallpaper_available();
        let can_keep = self.can_keep_current_image();
        let can_blacklist = self.can_blacklist_current_image();
        
        if has_next_available {
            println!("1. Next wallpaper");
        } else {
            println!("1. Next wallpaper (unavailable - no images/markets)");
        }
        
        if can_keep {
            println!("2. Keep \"{}\"", title);
        } else {
            println!("2. Keep \"{}\" (unavailable)", title);
        }
        
        if can_blacklist {
            println!("3. Blacklist \"{}\"", title);
        } else {
            println!("3. Blacklist \"{}\" (unavailable)", title);
        }
        println!("4. Next Kept wallpaper");
        println!("5. Exit");
        print!("\nSelect an option (1-5): ");
        io::stdout().flush().unwrap();
    }
    
    fn run_cli_mode(&mut self) -> Result<()> {
        loop {
            self.show_menu();
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            
            match input.trim() {
                "1" => {
                    if self.has_next_wallpaper_available() {
                        if let Err(e) = self.set_next_wallpaper() {
                            eprintln!("Failed to set next wallpaper: {}", e);
                        }
                    } else {
                        println!("Next wallpaper is not available - no images in unprocessed folder and no available market codes");
                    }
                }
                "2" => {
                    if self.can_keep_current_image() {
                        if let Err(e) = self.keep_current_image() {
                            eprintln!("Failed to keep image: {}", e);
                        }
                    } else {
                        println!("Keep current image is not available - no current image or image is already in favorites");
                    }
                }
                "3" => {
                    if self.can_blacklist_current_image() {
                        if let Err(e) = self.blacklist_current_image() {
                            eprintln!("Failed to blacklist image: {}", e);
                        }
                    } else {
                        println!("Blacklist current image is not available - no current image");
                    }
                }
                "4" => {
                    if let Err(e) = self.set_kept_wallpaper() {
                        eprintln!("Failed to set kept wallpaper: {}", e);
                    }
                }
                "5" => {
                    println!("Exiting BingTray...");
                    break;
                }
                _ => {
                    println!("Invalid option. Please select 1-5.");
                }
            }
        }
        
        Ok(())
    }
}

fn create_tray_menu(app: &BingTrayApp) -> (Menu, Vec<tray_icon::menu::MenuId>) {
    let tray_menu = Menu::new();
    let (title, last_tried, available_count) = app.get_status_info();
    
    // Create info items (non-clickable)
    let info_item = MenuItem::new(
        format!("Current: {}", title), 
        false, 
        None
    );
    let status_item = MenuItem::new(
        format!("Last: {} | Available: {}", last_tried, available_count), 
        false, 
        None
    );
    
    // Create action menu items (clickable)
    let has_next_available = app.has_next_wallpaper_available();
    let can_keep = app.can_keep_current_image();
    let can_blacklist = app.can_blacklist_current_image();
    
    let next_item = MenuItem::new("1. Next wallpaper", has_next_available, None);
    let keep_item = MenuItem::new(
        format!("2. Keep \"{}\"", title), 
        can_keep, 
        None
    );
    let blacklist_item = MenuItem::new(
        format!("3. Blacklist \"{}\"", title), 
        can_blacklist, 
        None
    );
    let kept_item = MenuItem::new("4. Next Kept wallpaper", true, None);
    let quit_item = MenuItem::new("5. Exit", true, None);

    // Store the menu item IDs in order
    let menu_item_ids = vec![
        next_item.id().clone(),
        keep_item.id().clone(), 
        blacklist_item.id().clone(),
        kept_item.id().clone(),
        quit_item.id().clone(),
    ];

    tray_menu.append_items(&[
        &info_item,
        &status_item,
        &PredefinedMenuItem::separator(),
        &next_item,
        &keep_item,
        &blacklist_item,
        &kept_item,
        &PredefinedMenuItem::separator(),
        &quit_item,
    ]).expect("Failed to append menu items");

    (tray_menu, menu_item_ids)
}

fn update_tray_menu(tray_icon: &tray_icon::TrayIcon, app: &BingTrayApp, menu_items: &mut Vec<tray_icon::menu::MenuId>) {
    let (new_menu, new_menu_ids) = create_tray_menu(app);
    *menu_items = new_menu_ids;
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize app
    let mut app = BingTrayApp::new()?;
    app.initialize()?;
    
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
    
    println!("BingTray GUI started successfully!");
    
    let mut tray_icon = None;
    let mut menu_items = Vec::new();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(tao::event::StartCause::Init) => {
                let icon = load_icon();
                let (tray_menu, menu_ids) = create_tray_menu(&app);
                
                // Store the menu item IDs
                menu_items = menu_ids;

                tray_icon = Some(
                    TrayIconBuilder::new()
                        .with_menu(Box::new(tray_menu))
                        .with_tooltip("BingTray - Bing Wallpaper Manager")
                        .with_icon(icon)
                        .build()
                        .unwrap(),
                );

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
                    if event.id == menu_items[0] {
                        // Next wallpaper - only execute if available
                        if app.has_next_wallpaper_available() {
                            println!("Executing: Next wallpaper");
                            if let Err(e) = app.set_next_wallpaper() {
                                eprintln!("Failed to set next wallpaper: {}", e);
                            } else if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &app, &mut menu_items);
                            }
                        } else {
                            println!("Next wallpaper is not available - no images in unprocessed folder and no available market codes");
                        }
                    } else if event.id == menu_items[1] {
                        // Keep current image - only execute if available
                        if app.can_keep_current_image() {
                            println!("Executing: Keep current image");
                            if let Err(e) = app.keep_current_image() {
                                eprintln!("Failed to keep image: {}", e);
                            } else if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &app, &mut menu_items);
                            }
                        } else {
                            println!("Keep current image is not available - no current image or image is already in favorites");
                        }
                    } else if event.id == menu_items[2] {
                        // Blacklist current image - only execute if available
                        if app.can_blacklist_current_image() {
                            println!("Executing: Blacklist current image");
                            if let Err(e) = app.blacklist_current_image() {
                                eprintln!("Failed to blacklist image: {}", e);
                            } else if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &app, &mut menu_items);
                            }
                        } else {
                            println!("Blacklist current image is not available - no current image");
                        }
                    } else if event.id == menu_items[3] {
                        // Next kept wallpaper
                        println!("Executing: Next kept wallpaper");
                        if let Err(e) = app.set_kept_wallpaper() {
                            eprintln!("Failed to set kept wallpaper: {}", e);
                        } else if let Some(ref icon) = tray_icon {
                            update_tray_menu(icon, &app, &mut menu_items);
                        }
                    } else if event.id == menu_items[4] {
                        // Exit
                        println!("Executing: Exit");
                        tray_icon.take();
                        *control_flow = ControlFlow::Exit;
                    } else {
                        println!("Unknown menu item clicked: {:?}", event.id);
                    }
                }
            }

            _ => {}
        }
    })
}

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;

use anyhow::Result;
use app::BingTrayApp;
use std::io::IsTerminal;
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
    UI::WindowsAndMessaging::{ShowWindow, SW_HIDE},
    System::Console::GetConsoleWindow,
};

enum UserEvent {
    TrayIconEvent(tray_icon::TrayIconEvent),
    MenuEvent(tray_icon::menu::MenuEvent),
}

fn create_tray_menu(app: &BingTrayApp) -> (Menu, Vec<tray_icon::menu::MenuId>, Option<String>) {
    let tray_menu = Menu::new();
    let title = app.get_current_image_title();
    let (last_tried, available_count) = app.get_market_status();
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
        if available_count == 0 {
            format!("Status: {}", last_tried)  // last_tried will contain "Historical X/Y" format
        } else {
            format!("Last: {} | Available: {}", last_tried, available_count)
        }, 
        false, 
        None
    );
    
    // Create action menu items (clickable) - matching cli menu structure
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

    // Store the menu item IDs in consistent order matching cli (0-5)
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
    // Embed the icon file at compile time
    let icon_bytes = include_bytes!("../resources/logo.png");
    
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(icon_bytes)
            .expect("Failed to load embedded icon")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to create icon from embedded data")
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

#[cfg(not(target_os = "windows"))]
fn hide_console() {
    // No-op on non-Windows platforms
}

fn main() -> Result<()> {
    // Check if we're running in terminal mode
    if IsTerminal::is_terminal(&std::io::stdout()) {
        // Terminal mode - run CLI interface
        println!("BingTray CLI mode started successfully!");
        let mut app = BingTrayApp::new()?;
        app.initialize()?;
        return app.run_cli_mode();
    }
    
    // Otherwise run GUI mode
    let mut app = BingTrayApp::new()?;
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
    
    // Hide console on Windows for GUI mode
    #[cfg(target_os = "windows")]
    hide_console();
    
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
                            // Use webbrowser crate for cross-platform URL opening
                            if let Err(e) = webbrowser::open(link) {
                                eprintln!("Failed to open copyright link: {}", e);
                            }
                        }
                        return ; // Return early 
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
                        if let Some(ref icon) = tray_icon {
                            update_tray_menu(icon, &app, &mut menu_items, &mut copyright_link);
                        }
                        // Return early to let the user click again after initialization
                        return;
                    }
                }
            }
            _ => {}
        }
    })
}

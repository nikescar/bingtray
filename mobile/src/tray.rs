//! System tray interface for Bingtray (Desktop only)
//!
//! Provides a system tray icon with menu for managing Bing wallpapers

use crate::calc_bingimage::BingTrayLogic;
use anyhow::Result;
use egui_i18n::tr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, MenuId},
    Icon, TrayIconBuilder, TrayIcon,
};

/// Action to take after tray mode exits
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayExitAction {
    Quit,
    OpenGui,
}

/// Menu item identifiers
struct MenuItems {
    show_app: MenuId,
    cache_dir: MenuId,
    next_market: MenuId,
    keep_current: MenuId,
    blacklist_current: MenuId,
    random_favorite: MenuId,
    quit: MenuId,
}

/// Load the embedded icon
fn load_icon() -> Icon {
    let icon_bytes = include_bytes!("../app/src/main/play_store_512.png");
    let image = image::load_from_memory(icon_bytes).expect("Failed to load icon");
    let rgba = image.to_rgba8();

    Icon::from_rgba(rgba.to_vec(), image.width(), image.height())
        .expect("Failed to create icon")
}

/// Create the tray menu based on current application state
fn create_tray_menu(logic: &BingTrayLogic) -> (Menu, MenuItems) {
    let menu = Menu::new();

    let show_app = MenuItem::new(format!("🖼️ {}", tr!("tray-show-app")), true, None);
    let cache_dir = MenuItem::new(format!("📁 {}", tr!("tray-cache-dir")), true, None);
    let next_market = MenuItem::new(format!("🔄 {}", tr!("tray-next-market")), logic.has_next_available(), None);

    let current_title = logic.get_current_image_title();
    let keep_text = if logic.can_keep() {
        format!("⭐ {}", tr!("tray-keep-with-title", { title: current_title.clone() }))
    } else {
        format!("⭐ {}", tr!("tray-keep-current"))
    };
    let keep_current = MenuItem::new(keep_text, logic.can_keep(), None);

    let blacklist_text = if logic.can_blacklist() {
        format!("🚫 {}", tr!("tray-blacklist-with-title", { title: current_title.clone() }))
    } else {
        format!("🚫 {}", tr!("tray-blacklist-current"))
    };
    let blacklist_current = MenuItem::new(blacklist_text, logic.can_blacklist(), None);

    let random_favorite = MenuItem::new(
        format!("🎲 {}", tr!("tray-random-favorite")),
        logic.has_kept_wallpapers(),
        None,
    );

    let quit = MenuItem::new(format!("🚪 {}", tr!("tray-quit")), true, None);

    let menu_items = MenuItems {
        show_app: show_app.id().clone(),
        cache_dir: cache_dir.id().clone(),
        next_market: next_market.id().clone(),
        keep_current: keep_current.id().clone(),
        blacklist_current: blacklist_current.id().clone(),
        random_favorite: random_favorite.id().clone(),
        quit: quit.id().clone(),
    };

    menu.append(&show_app).ok();
    menu.append(&MenuItem::new("", false, None)).ok(); // Separator
    menu.append(&cache_dir).ok();
    menu.append(&next_market).ok();
    menu.append(&keep_current).ok();
    menu.append(&blacklist_current).ok();
    menu.append(&random_favorite).ok();
    menu.append(&MenuItem::new("", false, None)).ok(); // Separator
    menu.append(&quit).ok();

    (menu, menu_items)
}

/// Update the tray menu with new state
fn update_tray_menu(
    tray_icon: &TrayIcon,
    logic: &BingTrayLogic,
    menu_items: &mut MenuItems,
) {
    let (new_menu, new_menu_items) = create_tray_menu(logic);
    tray_icon.set_menu(Some(Box::new(new_menu)));
    *menu_items = new_menu_items;
}

/// Initialize GTK on Linux (required for tray icon)
#[cfg(target_os = "linux")]
fn init_gtk() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        // GTK must be initialized on the main thread
        gtk::init().expect("Failed to initialize GTK");
        log::info!("GTK initialized for tray icon support");
    });
}

/// Run the system tray mode
pub fn run_tray_mode() -> Result<TrayExitAction> {
    log::info!("Starting tray mode...");

    // Initialize GTK on Linux (must be called before creating tray icon)
    #[cfg(target_os = "linux")]
    init_gtk();

    // Create application logic
    let mut app = BingTrayLogic::new()?;
    app.initialize()?;

    // Create tray icon and menu
    let icon = load_icon();
    let (tray_menu, mut menu_items) = create_tray_menu(&app);

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Bingtray - Bing Wallpaper Manager")
        .with_icon(icon)
        .build()
        .expect("Failed to create tray icon");

    log::info!("Tray icon created");

    // Track exit action
    let exit_action = Arc::new(Mutex::new(TrayExitAction::Quit));
    let exit_action_clone = exit_action.clone();

    // Event loop - poll for tray events
    let menu_channel = MenuEvent::receiver();

    loop {
        // Process pending GTK events on Linux
        #[cfg(target_os = "linux")]
        {
            while gtk::events_pending() {
                gtk::main_iteration();
            }
        }

        // Check for menu events (non-blocking with short timeout)
        if let Ok(event) = menu_channel.recv_timeout(Duration::from_millis(100)) {
            log::debug!("Menu event: {:?}", event);

            if event.id == menu_items.show_app {
                // Exit tray mode to open GUI on main thread
                log::info!("Exiting tray mode to open GUI");
                *exit_action_clone.lock().unwrap() = TrayExitAction::OpenGui;
                break;
            } else if event.id == menu_items.cache_dir {
                // Open cache directory
                log::info!("Opening cache directory");
                if let Err(e) = app.open_cache_directory() {
                    log::error!("Failed to open cache directory: {}", e);
                }
            } else if event.id == menu_items.next_market {
                // Set next wallpaper from unprocessed folder
                log::info!("Setting next wallpaper");

                // Try to set next wallpaper
                match app.set_next_wallpaper() {
                    Ok(true) => {
                        log::info!("Wallpaper set successfully");

                        // Auto-download if unprocessed folder is getting low (< 3 images)
                        // This ensures we always have images ready
                        if !app.has_unprocessed_files() || app.count_unprocessed_files() < 3 {
                            log::info!("Unprocessed folder low, downloading next page in background");
                            match app.download_from_next_market() {
                                Ok(count) => {
                                    log::info!("Downloaded {} images for next use", count);
                                }
                                Err(e) => {
                                    log::warn!("Failed to download next page: {}", e);
                                }
                            }
                        }

                        update_tray_menu(&tray_icon, &app, &mut menu_items);
                    }
                    Ok(false) => {
                        log::warn!("No wallpapers available in unprocessed folder");

                        // Try to download
                        log::info!("Attempting to download from next page");
                        match app.download_from_next_market() {
                            Ok(count) => {
                                log::info!("Downloaded {} images", count);
                                // Try setting wallpaper again
                                match app.set_next_wallpaper() {
                                    Ok(true) => {
                                        log::info!("Wallpaper set after download");
                                        update_tray_menu(&tray_icon, &app, &mut menu_items);
                                    }
                                    Ok(false) => {
                                        log::error!("Still no wallpapers after download");
                                    }
                                    Err(e) => {
                                        log::error!("Failed to set wallpaper: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to download: {}. No more images available.", e);
                                update_tray_menu(&tray_icon, &app, &mut menu_items);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to set wallpaper: {}", e);
                    }
                }
            } else if event.id == menu_items.keep_current {
                // Keep current image
                log::info!("Keeping current image");
                if let Err(e) = app.keep_current_image() {
                    log::error!("Failed to keep image: {}", e);
                } else {
                    update_tray_menu(&tray_icon, &app, &mut menu_items);
                }
            } else if event.id == menu_items.blacklist_current {
                // Blacklist current image
                log::info!("Blacklisting current image");
                if let Err(e) = app.blacklist_current_image() {
                    log::error!("Failed to blacklist image: {}", e);
                } else {
                    update_tray_menu(&tray_icon, &app, &mut menu_items);
                }
            } else if event.id == menu_items.random_favorite {
                // Set random favorite wallpaper
                log::info!("Setting random favorite wallpaper");
                match app.set_kept_wallpaper() {
                    Ok(true) => {
                        log::info!("Favorite wallpaper set successfully");
                        update_tray_menu(&tray_icon, &app, &mut menu_items);
                    }
                    Ok(false) => {
                        log::warn!("No favorite wallpapers available");
                    }
                    Err(e) => {
                        log::error!("Failed to set favorite wallpaper: {}", e);
                    }
                }
            } else if event.id == menu_items.quit {
                // Quit application
                log::info!("Quitting application");
                *exit_action_clone.lock().unwrap() = TrayExitAction::Quit;
                break;
            }
        }
    }

    // Explicitly drop the tray icon before returning
    drop(tray_icon);

    let action = *exit_action.lock().unwrap();
    log::info!("Tray mode exited with action: {:?}", action);
    Ok(action)
}

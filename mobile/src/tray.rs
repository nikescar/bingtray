//! System tray interface for Bingtray (Desktop only)
//!
//! Provides a system tray icon with menu for managing Bing wallpapers
//!
//! For tray interface, since there is no ui, set/keep/black operation
//! is based on current wallpaper image on desktop.
//!
//! When "Show App" is clicked, the tray exits and returns TrayExitAction::OpenGui
//! to allow the GUI to be opened on the main thread (required by winit's EventLoop).
//!

use crate::calc_bingimage::CalcBingimage;
use anyhow::Result;
use egui_i18n::tr;
use std::sync::{Arc, Mutex, OnceLock};
use crossbeam_queue::SegQueue;
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::run_return::EventLoopExtRunReturn,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, MenuId},
    Icon, TrayIconBuilder, TrayIcon, TrayIconEvent,
};

/// Global queue for tray icon events (set up once at startup)
static TRAY_ICON_EVENTS: OnceLock<Arc<SegQueue<TrayIconEvent>>> = OnceLock::new();
/// Global queue for menu events (set up once at startup)
static MENU_EVENTS: OnceLock<Arc<SegQueue<MenuEvent>>> = OnceLock::new();

/// Action to take after tray mode exits
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayExitAction {
    Quit,
    OpenGui,
}

/// User events for the event loop
#[derive(Debug)]
enum UserEvent {
    TrayIconEvent(TrayIconEvent),
    MenuEvent(MenuEvent),
}

/// Initialize global event handlers (call once at startup)
pub fn init_tray_event_handlers() {
    log::info!("Initializing global tray event handlers (one-time setup)");

    // Initialize queues
    let tray_queue = Arc::new(SegQueue::new());
    let menu_queue = Arc::new(SegQueue::new());

    TRAY_ICON_EVENTS.get_or_init(|| tray_queue.clone());
    MENU_EVENTS.get_or_init(|| menu_queue.clone());

    // Set up global event handlers that push to queues
    let tray_queue_for_handler = tray_queue.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        log::debug!("TrayIconEvent received, pushing to global queue: {:?}", event);
        tray_queue_for_handler.push(event);
    }));

    let menu_queue_for_handler = menu_queue.clone();
    MenuEvent::set_event_handler(Some(move |event| {
        log::info!(">>> MenuEvent handler fired, pushing to global queue: {:?}", event);
        menu_queue_for_handler.push(event);
    }));

    log::info!("Global tray event handlers initialized successfully");
}

/// Menu item identifiers
struct MenuItems {
    show_app: MenuId,
    cache_dir: MenuId,
    next_market: MenuId,
    current_title: MenuId,  // Display current wallpaper title
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
fn create_tray_menu(logic: &CalcBingimage) -> (Menu, MenuItems) {
    let menu = Menu::new();

    let show_app = MenuItem::new(format!("{}", tr!("tray-show-app")), true, None);
    let cache_dir = MenuItem::new(format!("{}", tr!("tray-cache-dir")), true, None);
    let next_market = MenuItem::new(
        format!("{}\n{}", tr!("tray-next-market"), logic.get_wallpaper_page_status()),
        logic.has_next_available(),
        None
    );

    // Display current wallpaper title (non-clickable)
    let current_title_text = logic.get_current_image_title();
    let current_title_display = if !current_title_text.is_empty() {
        format!("📷 {}", current_title_text)
    } else {
        format!("📷 {}", tr!("tray-no-wallpaper"))
    };
    let current_title_item = MenuItem::new(current_title_display, false, None); // disabled = not clickable

    let current_title = current_title_text;
    let keep_text = if logic.can_keep() {
        format!("{}", tr!("tray-keep-with-title", { title: current_title.clone() }))
    } else {
        format!("{}", tr!("tray-keep-current"))
    };
    let keep_current = MenuItem::new(keep_text, logic.can_keep(), None);

    let blacklist_text = if logic.can_blacklist() {
        format!("{}", tr!("tray-blacklist-with-title", { title: current_title.clone() }))
    } else {
        format!("{}", tr!("tray-blacklist-current"))
    };
    let blacklist_current = MenuItem::new(blacklist_text, logic.can_blacklist(), None);

    let random_favorite = MenuItem::new(
        format!("{}", tr!("tray-random-favorite")),
        logic.has_kept_wallpapers(),
        None,
    );

    let quit = MenuItem::new(format!("{}", tr!("tray-quit")), true, None);

    let menu_items = MenuItems {
        show_app: show_app.id().clone(),
        cache_dir: cache_dir.id().clone(),
        next_market: next_market.id().clone(),
        current_title: current_title_item.id().clone(),
        keep_current: keep_current.id().clone(),
        blacklist_current: blacklist_current.id().clone(),
        random_favorite: random_favorite.id().clone(),
        quit: quit.id().clone(),
    };

    menu.append(&show_app).ok();
    menu.append(&MenuItem::new("", false, None)).ok(); // Separator
    menu.append(&cache_dir).ok();
    menu.append(&next_market).ok();
    menu.append(&current_title_item).ok(); // Current wallpaper title
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
    logic: &CalcBingimage,
    menu_items: &mut MenuItems,
) {
    let (new_menu, new_menu_items) = create_tray_menu(logic);
    *menu_items = new_menu_items;
    tray_icon.set_menu(Some(Box::new(new_menu)));
}

/// Run the system tray mode with proper event loop
pub fn run_tray_mode() -> Result<TrayExitAction> {
    log::info!("=== Starting tray mode ===");

    // Create application logic
    let mut app = CalcBingimage::new()?;
    app.initialize()?;

    // Create event loop (must be mutable for run_return)
    log::info!("Creating new event loop");
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    log::info!("Event loop created successfully");

    // Get references to global event queues
    let tray_queue = TRAY_ICON_EVENTS.get().expect("Tray event handlers not initialized! Call init_tray_event_handlers() first");
    let menu_queue = MENU_EVENTS.get().expect("Menu event handlers not initialized! Call init_tray_event_handlers() first");
    log::info!("Got references to global event queues");

    // Track exit action - use Arc<Mutex<>> to share between closure and return
    let exit_action = Arc::new(Mutex::new(TrayExitAction::Quit));
    let exit_action_for_return = exit_action.clone();

    // Variables to be captured by the event loop
    let mut tray_icon: Option<TrayIcon> = None;
    let mut menu_items: Option<MenuItems> = None;

    // Clone Arc references for the closure
    let tray_queue = tray_queue.clone();
    let menu_queue = menu_queue.clone();

    // Run event loop with run_return (allows returning to caller)
    log::info!("Starting event loop...");
    event_loop.run_return(move |event, _, control_flow| {
        // Use Poll mode to ensure system events are processed immediately
        // We'll manually sleep if there are no events to keep CPU usage low
        *control_flow = ControlFlow::Poll;

        // Process events from global queues
        while let Some(tray_event) = tray_queue.pop() {
            log::debug!("Processing TrayIconEvent from queue: {:?}", tray_event);
        }

        while let Some(menu_event) = menu_queue.pop() {
            log::info!(">>> Menu event received from queue: {:?}", menu_event);

            if let Some(ref items) = menu_items {
                log::debug!("Menu items available, checking which item was clicked");
                if menu_event.id == items.show_app {
                    log::info!(">>> 'Show App' menu item clicked!");
                    // Open GUI window on main thread
                    // EventLoop can only be created once per process, so we exit tray mode
                    // and let main.rs open the GUI on the main thread
                    log::info!("Exiting tray event loop to open GUI on main thread");
                    *exit_action_for_return.lock().unwrap() = TrayExitAction::OpenGui;
                    *control_flow = ControlFlow::Exit;
                    log::info!("Control flow set to Exit");
                    continue; // Skip further processing
                } else if menu_event.id == items.cache_dir {
                    // Open cache directory
                    log::info!("Opening cache directory");
                    if let Err(e) = app.open_cache_directory() {
                        log::error!("Failed to open cache directory: {}", e);
                    }
                } else if menu_event.id == items.next_market {
                    // Next market wallpaper
                    log::info!("Setting next market wallpaper");
                    match app.set_next_market_wallpaper() {
                        Ok(true) => {
                            log::info!("Wallpaper set successfully!");
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &app, &mut menu_items.as_mut().unwrap());
                            }
                        }
                        Ok(false) => {
                            log::warn!("No wallpapers available");
                        }
                        Err(e) => {
                            log::error!("Failed to set wallpaper: {}", e);
                        }
                    }
                } else if menu_event.id == items.keep_current {
                    // Keep current image
                    if app.can_keep() {
                        log::info!("Keeping current image");
                        if let Err(e) = app.keep_current_image() {
                            log::error!("Failed to keep image: {}", e);
                        } else {
                            log::info!("Image moved to favorites!");
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &app, &mut menu_items.as_mut().unwrap());
                            }
                        }
                    }
                } else if menu_event.id == items.blacklist_current {
                    // Blacklist current image
                    if app.can_blacklist() {
                        log::info!("Blacklisting current image");
                        if let Err(e) = app.blacklist_current_image() {
                            log::error!("Failed to blacklist image: {}", e);
                        } else {
                            log::info!("Image blacklisted!");
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &app, &mut menu_items.as_mut().unwrap());
                            }
                        }
                    }
                } else if menu_event.id == items.random_favorite {
                    // Set random favorite wallpaper
                    if app.has_kept_wallpapers() {
                        log::info!("Setting random favorite wallpaper");
                        match app.set_kept_wallpaper() {
                            Ok(true) => {
                                log::info!("Favorite wallpaper set!");
                                if let Some(ref icon) = tray_icon {
                                    update_tray_menu(icon, &app, &mut menu_items.as_mut().unwrap());
                                }
                            }
                            Ok(false) => {
                                log::warn!("No favorite wallpapers available");
                            }
                            Err(e) => {
                                log::error!("Failed to set favorite wallpaper: {}", e);
                            }
                        }
                    }
                } else if menu_event.id == items.quit {
                    // Quit application
                    log::info!("Quitting application");
                    *exit_action_for_return.lock().unwrap() = TrayExitAction::Quit;
                    tray_icon.take(); // Drop tray icon
                    *control_flow = ControlFlow::Exit;
                }
            }
        }

        match event {
            Event::NewEvents(tao::event::StartCause::Init) => {
                log::info!("Event loop Init event - creating tray icon");
                // Create tray icon on initialization
                let icon = load_icon();
                let (tray_menu, items) = create_tray_menu(&app);

                tray_icon = Some(
                    TrayIconBuilder::new()
                        .with_menu(Box::new(tray_menu))
                        .with_tooltip("Bingtray - Bing Wallpaper Manager")
                        .with_icon(icon)
                        .build()
                        .expect("Failed to create tray icon"),
                );

                menu_items = Some(items);
                log::info!("Tray icon created");

                // Wake up macOS run loop if needed
                #[cfg(target_os = "macos")]
                unsafe {
                    use objc2_core_foundation::CFRunLoop;
                    if let Some(rl) = CFRunLoop::main() {
                        rl.wake_up();
                    }
                }
            }

            _ => {}
        }

        // In Poll mode, sleep briefly if we're not exiting to avoid high CPU usage
        // This gives time for system events to be delivered while keeping responsiveness
        if *control_flow != ControlFlow::Exit {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });

    log::info!("Event loop has exited (run_return completed)");
    let action = *exit_action.lock().unwrap();
    log::info!("=== Tray mode exited with action: {:?} ===", action);
    Ok(action)
}

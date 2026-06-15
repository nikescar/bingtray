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
    TRAY_ICON_EVENTS.get_or_init(|| Arc::new(SegQueue::new()));
    MENU_EVENTS.get_or_init(|| Arc::new(SegQueue::new()));

    // Set up global tray icon event handler
    TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
        if let Some(queue) = TRAY_ICON_EVENTS.get() {
            queue.push(event);
        }
    }));

    // Set up global menu event handler
    MenuEvent::set_event_handler(Some(|event: MenuEvent| {
        if let Some(queue) = MENU_EVENTS.get() {
            queue.push(event);
        }
    }));

    log::info!("Global tray event handlers initialized successfully");
}

/// Menu items for tracking clicks
#[derive(Debug)]
struct MenuItems {
    show_app: MenuId,
    cache_dir: MenuId,
    next_market: MenuId,
    current_title: MenuId,
    keep_current: MenuId,
    blacklist_current: MenuId,
    random_favorite: MenuId,
    quit: MenuId,
}

/// Load tray icon from embedded asset
fn load_tray_icon() -> Icon {
    let start_time = std::time::Instant::now();
    log::debug!("Loading tray icon...");

    let t0 = std::time::Instant::now();
    let icon_bytes = include_bytes!("../resources/logo.png");
    log::debug!("  include_bytes!(): {:?}", t0.elapsed());

    let t1 = std::time::Instant::now();
    let image = image::load_from_memory(icon_bytes).expect("Failed to load icon");
    let rgba = image.to_rgba8();
    log::debug!("  to_rgba8(): {:?}", t1.elapsed());

    let t2 = std::time::Instant::now();
    let icon = Icon::from_rgba(rgba.to_vec(), image.width(), image.height())
        .expect("Failed to create icon");
    log::debug!("  Icon::from_rgba(): {:?}", t2.elapsed());

    log::debug!("Icon loaded in {:?}", start_time.elapsed());
    icon
}

/// Tray logic wrapper using ViewModel (sync mode)
struct TrayLogic {
    conn: diesel::SqliteConnection,
}

impl TrayLogic {
    fn new() -> Result<Self> {
        use diesel::Connection;
        let db_path = crate::db::get_database_path()?;
        let mut conn = diesel::SqliteConnection::establish(&db_path.to_string_lossy())?;

        // Run migrations
        use diesel_migrations::MigrationHarness;
        conn.run_pending_migrations(crate::db::MIGRATIONS)
            .map_err(|e| anyhow::anyhow!("Migration failed: {}", e))?;

        Ok(Self { conn })
    }

    fn get_wallpaper_page_status(&mut self) -> String {
        // Simple status - just show count of unprocessed images
        match crate::db::operations::count_by_status(&mut self.conn, crate::db::ImageStatus::Unprocessed) {
            Ok(count) => format!("({} available)", count),
            Err(_) => String::new(),
        }
    }

    fn has_next_available(&mut self) -> bool {
        // Always return true because download_and_set_next_wallpaper_sync will auto-download if needed
        // (when unprocessed count < 7, it fetches new images from sources)
        true
    }

    fn get_current_image_title(&mut self) -> String {
        use crate::viewmodel::commands::get_current_desktop_wallpaper_url_sync;

        if let Ok(Some(url)) = get_current_desktop_wallpaper_url_sync(&mut self.conn) {
            if let Ok(Some(image)) = crate::db::operations::get_image(&mut self.conn, &url) {
                let title = &image.title;
                if title.len() > 40 {
                    format!("{}...", &title[..40])
                } else {
                    title.clone()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    }

    fn can_keep(&mut self) -> bool {
        // Can keep if there's a current wallpaper and it's not already a favorite
        use crate::viewmodel::commands::get_current_desktop_wallpaper_url_sync;

        if let Ok(Some(url)) = get_current_desktop_wallpaper_url_sync(&mut self.conn) {
            if let Ok(Some(image)) = crate::db::operations::get_image(&mut self.conn, &url) {
                image.status != crate::db::ImageStatus::KeepFavorite.as_str()
            } else {
                false
            }
        } else {
            false
        }
    }

    fn can_blacklist(&mut self) -> bool {
        // Can blacklist if there's a current wallpaper
        use crate::viewmodel::commands::get_current_desktop_wallpaper_url_sync;
        get_current_desktop_wallpaper_url_sync(&mut self.conn).ok().flatten().is_some()
    }

    fn has_kept_wallpapers(&mut self) -> bool {
        crate::db::operations::count_by_status(&mut self.conn, crate::db::ImageStatus::KeepFavorite)
            .map(|count| count > 0)
            .unwrap_or(false)
    }

    fn open_cache_directory(&self) -> Result<()> {
        let config = crate::Config::new()?;
        let path = &config.cached_dir;

        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(path)
                .spawn()?;
        }

        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(path)
                .spawn()?;
        }

        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(path)
                .spawn()?;
        }

        log::info!("Opened cache directory: {:?}", path);
        Ok(())
    }

    fn set_next_market_wallpaper(&mut self) -> Result<bool> {
        use crate::viewmodel::commands::download_and_set_next_wallpaper_sync;

        match download_and_set_next_wallpaper_sync(&mut self.conn) {
            Ok(_result) => Ok(true),
            Err(e) => {
                log::error!("Failed to set next wallpaper: {}", e);
                Err(e)
            }
        }
    }

    fn keep_current_image(&mut self) -> Result<()> {
        use crate::viewmodel::commands::keep_current_wallpaper_sync;

        if let Some(_title) = keep_current_wallpaper_sync(&mut self.conn)? {
            log::info!("Kept current image");
            Ok(())
        } else {
            anyhow::bail!("No current wallpaper to keep")
        }
    }

    fn blacklist_current_image(&mut self) -> Result<()> {
        use crate::viewmodel::commands::blacklist_current_wallpaper_sync;

        if let Some(_title) = blacklist_current_wallpaper_sync(&mut self.conn)? {
            log::info!("Blacklisted current image");
            Ok(())
        } else {
            anyhow::bail!("No current wallpaper to blacklist")
        }
    }

    fn set_kept_wallpaper(&mut self) -> Result<bool> {
        use crate::viewmodel::commands::set_random_favorite_wallpaper_sync;

        match set_random_favorite_wallpaper_sync(&mut self.conn) {
            Ok(Some(_title)) => Ok(true),
            Ok(None) => {
                log::warn!("No favorite wallpapers available");
                Ok(false)
            }
            Err(e) => {
                log::error!("Failed to set random favorite: {}", e);
                Err(e)
            }
        }
    }
}

/// Create the tray menu based on current application state
fn create_tray_menu(logic: &mut TrayLogic) -> (Menu, MenuItems) {
    let start_time = std::time::Instant::now();
    log::debug!("=== Creating tray menu ===");

    let menu = Menu::new();

    let show_app = MenuItem::new(format!("{}", tr!("tray-show-app")), true, None);
    let cache_dir = MenuItem::new(format!("{}", tr!("tray-cache-dir")), true, None);

    let wallpaper_status = logic.get_wallpaper_page_status();
    let has_next = logic.has_next_available();
    let next_market = MenuItem::new(
        format!("{}\n{}", tr!("tray-next-market"), wallpaper_status),
        has_next,
        None
    );

    // Display current wallpaper title (non-clickable)
    let current_title_text = logic.get_current_image_title();
    let current_title_display = if !current_title_text.is_empty() {
        format!("📷 {}", current_title_text)
    } else {
        format!("📷 {}", tr!("tray-no-wallpaper"))
    };
    let current_title_item = MenuItem::new(current_title_display, false, None);
    let current_title = current_title_text;

    let can_keep = logic.can_keep();
    let keep_text = if can_keep {
        format!("{}", tr!("tray-keep-with-title", { title: current_title.clone() }))
    } else {
        format!("{}", tr!("tray-keep-current"))
    };
    let keep_current = MenuItem::new(keep_text, can_keep, None);

    let can_blacklist = logic.can_blacklist();
    let blacklist_text = if can_blacklist {
        format!("{}", tr!("tray-blacklist-with-title", { title: current_title.clone() }))
    } else {
        format!("{}", tr!("tray-blacklist-current"))
    };
    let blacklist_current = MenuItem::new(blacklist_text, can_blacklist, None);

    let has_kept = logic.has_kept_wallpapers();
    let random_favorite = MenuItem::new(
        format!("{}", tr!("tray-random-favorite")),
        has_kept,
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
    menu.append(&current_title_item).ok();
    menu.append(&keep_current).ok();
    menu.append(&blacklist_current).ok();
    menu.append(&random_favorite).ok();
    menu.append(&MenuItem::new("", false, None)).ok(); // Separator
    menu.append(&quit).ok();

    log::info!("=== Tray menu created in {:?} ===", start_time.elapsed());

    (menu, menu_items)
}

/// Update the tray menu with new state
fn update_tray_menu(
    tray_icon: &TrayIcon,
    logic: &mut TrayLogic,
    menu_items: &mut MenuItems,
) {
    let start_time = std::time::Instant::now();
    log::info!("=== Updating tray menu ===");

    let (new_menu, new_menu_items) = create_tray_menu(logic);
    *menu_items = new_menu_items;
    tray_icon.set_menu(Some(Box::new(new_menu)));

    log::info!("=== Tray menu updated in {:?} ===", start_time.elapsed());
}

/// Run the system tray mode with proper event loop
pub fn run_tray_mode() -> Result<TrayExitAction> {
    log::info!("=== Starting tray mode ===");

    // Create application logic
    let mut app = TrayLogic::new()?;

    // Create event loop (must be mutable for run_return)
    log::info!("Creating new event loop");
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    log::info!("Event loop created successfully");

    // Get references to global event queues
    let tray_queue = TRAY_ICON_EVENTS.get().expect("Tray event handlers not initialized! Call init_tray_event_handlers() first");
    let menu_queue = MENU_EVENTS.get().expect("Menu event handlers not initialized! Call init_tray_event_handlers() first");
    log::info!("Got references to global event queues");

    // Track exit action
    let exit_action = Arc::new(Mutex::new(TrayExitAction::Quit));
    let exit_action_for_return = exit_action.clone();

    // Variables to be captured by the event loop
    let mut tray_icon: Option<TrayIcon> = None;
    let mut menu_items: Option<MenuItems> = None;

    // Clone Arc references for the closure
    let tray_queue = tray_queue.clone();
    let menu_queue = menu_queue.clone();

    // Run event loop with run_return
    log::info!("Starting event loop...");
    event_loop.run_return(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        // Process events from global queues
        while let Some(tray_event) = tray_queue.pop() {
            log::debug!("Processing TrayIconEvent from queue: {:?}", tray_event);
        }

        while let Some(menu_event) = menu_queue.pop() {
            log::info!(">>> Menu event received from queue: {:?}", menu_event);

            if let Some(ref items) = menu_items {
                if menu_event.id == items.show_app {
                    log::info!(">>> 'Show App' menu item clicked!");
                    *exit_action_for_return.lock().unwrap() = TrayExitAction::OpenGui;
                    *control_flow = ControlFlow::Exit;
                    continue;
                } else if menu_event.id == items.cache_dir {
                    log::info!("Opening cache directory");
                    if let Err(e) = app.open_cache_directory() {
                        log::error!("Failed to open cache directory: {}", e);
                    }
                } else if menu_event.id == items.next_market {
                    log::info!("Setting next market wallpaper");
                    match app.set_next_market_wallpaper() {
                        Ok(true) => {
                            log::info!("Wallpaper set successfully!");
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &mut app, &mut menu_items.as_mut().unwrap());
                            }
                        }
                        Ok(false) => log::warn!("No wallpapers available"),
                        Err(e) => log::error!("Failed to set wallpaper: {}", e),
                    }
                } else if menu_event.id == items.keep_current {
                    if app.can_keep() {
                        log::info!("Keeping current image");
                        if let Err(e) = app.keep_current_image() {
                            log::error!("Failed to keep image: {}", e);
                        } else {
                            log::info!("Image moved to favorites!");
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &mut app, &mut menu_items.as_mut().unwrap());
                            }
                        }
                    }
                } else if menu_event.id == items.blacklist_current {
                    if app.can_blacklist() {
                        log::info!("Blacklisting current image");
                        if let Err(e) = app.blacklist_current_image() {
                            log::error!("Failed to blacklist image: {}", e);
                        } else {
                            log::info!("Image blacklisted!");
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &mut app, &mut menu_items.as_mut().unwrap());
                            }
                        }
                    }
                } else if menu_event.id == items.random_favorite {
                    log::info!("Setting random favorite wallpaper");
                    match app.set_kept_wallpaper() {
                        Ok(true) => {
                            log::info!("Random favorite set successfully!");
                            if let Some(ref icon) = tray_icon {
                                update_tray_menu(icon, &mut app, &mut menu_items.as_mut().unwrap());
                            }
                        }
                        Ok(false) => log::warn!("No favorite wallpapers available"),
                        Err(e) => log::error!("Failed to set favorite: {}", e),
                    }
                } else if menu_event.id == items.quit {
                    log::info!("Quit menu item clicked");
                    *control_flow = ControlFlow::Exit;
                }
            }
        }

        // Handle window events
        match event {
            Event::NewEvents(_) => {
                // Lazy initialization of tray icon
                if tray_icon.is_none() {
                    log::info!("Lazy initialization of tray icon...");
                    let icon = load_tray_icon();
                    let (menu, items) = create_tray_menu(&mut app);

                    let new_tray_icon = TrayIconBuilder::new()
                        .with_menu(Box::new(menu))
                        .with_tooltip("BingTray")
                        .with_icon(icon)
                        .build()
                        .expect("Failed to build tray icon");

                    tray_icon = Some(new_tray_icon);
                    menu_items = Some(items);
                    log::info!("Tray icon initialized successfully");
                }

                // Sleep briefly to reduce CPU usage
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            _ => {}
        }
    });

    log::info!("Event loop exited");
    let result = *exit_action.lock().unwrap();
    log::info!("=== Tray mode exiting with action: {:?} ===", result);
    Ok(result)
}

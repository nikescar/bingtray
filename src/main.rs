use bingtray::{BingTray, set_wallpaper};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tray_icon::{TrayIcon, TrayIconBuilder, menu::{Menu, MenuItem, MenuEvent}};
use winit::event_loop::{EventLoop, ControlFlow};
use winit::event::{Event, WindowEvent};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting Bing Wallpaper Tray...");
    
    let bing_tray = Arc::new(Mutex::new(BingTray::new()?));
    let current_wallpaper = Arc::new(Mutex::new(None::<(PathBuf, String)>));
    
    // Initialize the application
    {
        let bing_tray_lock = bing_tray.lock().await;
        println!("Initializing configuration...");
        bing_tray_lock.initialize().await?;
        
        // Set initial wallpaper
        if let Some((wallpaper_path, title)) = bing_tray_lock.get_current_wallpaper_info()? {
            set_wallpaper(&wallpaper_path)?;
            println!("Set initial wallpaper: {}", wallpaper_path.display());
            
            let mut current = current_wallpaper.lock().await;
            *current = Some((wallpaper_path, title));
        } else {
            println!("No wallpapers available, downloading...");
            bing_tray_lock.download_images_from_random_market().await?;
            if let Some((wallpaper_path, title)) = bing_tray_lock.get_current_wallpaper_info()? {
                set_wallpaper(&wallpaper_path)?;
                println!("Set wallpaper: {}", wallpaper_path.display());
                
                let mut current = current_wallpaper.lock().await;
                *current = Some((wallpaper_path, title));
            }
        }
    }

    // Create event loop
    let event_loop = EventLoop::new()?;
    
    // Create tray icon
    let icon_rgba = vec![255u8; 32 * 32 * 4]; // 32x32 white icon
    let icon = tray_icon::Icon::from_rgba(icon_rgba, 32, 32)?;
    
    // Create menu items
    let next_wallpaper = MenuItem::new("Next wallpaper", true, None);
    let keep_wallpaper = MenuItem::new("Keep current wallpaper", true, None);
    let blacklist_wallpaper = MenuItem::new("Blacklist current wallpaper", true, None);
    let separator = MenuItem::separator();
    let exit = MenuItem::new("Exit", true, None);
    
    let menu = Menu::new();
    menu.append(&next_wallpaper)?;
    menu.append(&keep_wallpaper)?;
    menu.append(&blacklist_wallpaper)?;
    menu.append(&separator)?;
    menu.append(&exit)?;

    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Bing Wallpaper Manager")
        .with_icon(icon)
        .build()?;

    let menu_channel = MenuEvent::receiver();
    
    // Handle menu events
    let bing_tray_clone = Arc::clone(&bing_tray);
    let current_wallpaper_clone = Arc::clone(&current_wallpaper);
    
    tokio::spawn(async move {
        loop {
            if let Ok(event) = menu_channel.try_recv() {
                match event.id {
                    id if id == next_wallpaper.id() => {
                        handle_next_wallpaper(&bing_tray_clone, &current_wallpaper_clone).await;
                    }
                    id if id == keep_wallpaper.id() => {
                        handle_keep_wallpaper(&bing_tray_clone, &current_wallpaper_clone).await;
                    }
                    id if id == blacklist_wallpaper.id() => {
                        handle_blacklist_wallpaper(&bing_tray_clone, &current_wallpaper_clone).await;
                    }
                    id if id == exit.id() => {
                        std::process::exit(0);
                    }
                    _ => {}
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    });

    // Run event loop
    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);
        
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => elwt.exit(),
            _ => {}
        }
    })?;

    Ok(())
}

async fn handle_next_wallpaper(
    bing_tray: &Arc<Mutex<BingTray>>, 
    current_wallpaper: &Arc<Mutex<Option<(PathBuf, String)>>>
) {
    let bing_tray_lock = bing_tray.lock().await;
    
    // First consume the current wallpaper if any
    {
        let current = current_wallpaper.lock().await;
        if let Some((ref wallpaper_path, _)) = *current {
            let _ = bing_tray_lock.consume_current_wallpaper(wallpaper_path);
        }
    }
    
    match bing_tray_lock.get_current_wallpaper_info() {
        Ok(Some((wallpaper_path, title))) => {
            if let Ok(()) = set_wallpaper(&wallpaper_path) {
                let mut current = current_wallpaper.lock().await;
                *current = Some((wallpaper_path.clone(), title.clone()));
                println!("Set wallpaper: {} ({})", wallpaper_path.display(), title);
            }
        }
        Ok(None) => {
            println!("No more wallpapers available, downloading...");
            if let Ok(()) = bing_tray_lock.check_and_download_more_images().await {
                if let Ok(Some((wallpaper_path, title))) = bing_tray_lock.get_current_wallpaper_info() {
                    if let Ok(()) = set_wallpaper(&wallpaper_path) {
                        let mut current = current_wallpaper.lock().await;
                        *current = Some((wallpaper_path.clone(), title.clone()));
                        println!("Set wallpaper: {} ({})", wallpaper_path.display(), title);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error getting next wallpaper: {}", e);
        }
    }
}

async fn handle_keep_wallpaper(
    bing_tray: &Arc<Mutex<BingTray>>, 
    current_wallpaper: &Arc<Mutex<Option<(PathBuf, String)>>>
) {
    let current = current_wallpaper.lock().await;
    if let Some((ref wallpaper_path, ref title)) = *current {
        let bing_tray_lock = bing_tray.lock().await;
        if let Ok(()) = bing_tray_lock.keep_current_wallpaper(wallpaper_path) {
            println!("Kept wallpaper: {}", title);
            
            // Release locks before calling handle_next_wallpaper
            drop(current);
            drop(bing_tray_lock);
            
            // Set next wallpaper
            handle_next_wallpaper(bing_tray, current_wallpaper).await;
        }
    }
}

async fn handle_blacklist_wallpaper(
    bing_tray: &Arc<Mutex<BingTray>>, 
    current_wallpaper: &Arc<Mutex<Option<(PathBuf, String)>>>
) {
    let current = current_wallpaper.lock().await;
    if let Some((ref wallpaper_path, ref title)) = *current {
        let wallpaper_path_clone = wallpaper_path.clone();
        let title_clone = title.clone();
        
        let bing_tray_lock = bing_tray.lock().await;
        if let Ok(()) = bing_tray_lock.blacklist_current_wallpaper(&wallpaper_path_clone) {
            println!("Blacklisted wallpaper: {}", title_clone);
            
            // Release locks before calling handle_next_wallpaper
            drop(current);
            drop(bing_tray_lock);
            
            // Set next wallpaper
            handle_next_wallpaper(bing_tray, current_wallpaper).await;
        }
    }
}

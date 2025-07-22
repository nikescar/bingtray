use bingtray::{BingTray, set_wallpaper, get_desktop_environment};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Testing Bing Wallpaper functionality");
    println!("====================================");
    
    // Test desktop environment detection
    let desktop_env = get_desktop_environment();
    println!("Detected desktop environment: {}", desktop_env);
    
    // Test BingTray initialization
    let bing_tray = BingTray::new()?;
    println!("BingTray initialized successfully");
    
    // Initialize configuration
    bing_tray.initialize().await?;
    println!("Configuration initialized");
    
    // Check for wallpapers
    if let Some(wallpaper_path) = bing_tray.get_next_wallpaper()? {
        println!("Found wallpaper: {}", wallpaper_path.display());
        
        // Test wallpaper setting
        match set_wallpaper(&wallpaper_path) {
            Ok(_) => println!("Successfully set wallpaper"),
            Err(e) => println!("Failed to set wallpaper: {}", e),
        }
    } else {
        println!("No wallpapers found, downloading...");
        bing_tray.download_images_from_random_market().await?;
        
        if let Some(wallpaper_path) = bing_tray.get_next_wallpaper()? {
            println!("Downloaded and found wallpaper: {}", wallpaper_path.display());
            
            match set_wallpaper(&wallpaper_path) {
                Ok(_) => println!("Successfully set wallpaper"),
                Err(e) => println!("Failed to set wallpaper: {}", e),
            }
        } else {
            println!("Failed to download wallpapers");
        }
    }
    
    Ok(())
}

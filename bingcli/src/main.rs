use anyhow::Result;
use bingtray_core::*;
use chrono::Utc;
use rand::seq::SliceRandom;
use std::io::{self, Write};
use std::path::PathBuf;

struct BingCliApp {
    config: Config,
    current_image: Option<PathBuf>,
}

impl BingCliApp {
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
        println!("1. Next wallpaper");
        println!("2. Keep \"{}\"", title);
        println!("3. Blacklist \"{}\"", title);
        println!("4. Next Kept wallpaper");
        println!("5. Exit");
        print!("\nSelect an option (1-5): ");
        io::stdout().flush().unwrap();
    }
    
    fn run(&mut self) -> Result<()> {
        loop {
            self.show_menu();
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            
            match input.trim() {
                "1" => {
                    if let Err(e) = self.set_next_wallpaper() {
                        eprintln!("Failed to set next wallpaper: {}", e);
                    }
                }
                "2" => {
                    if let Err(e) = self.keep_current_image() {
                        eprintln!("Failed to keep image: {}", e);
                    }
                }
                "3" => {
                    if let Err(e) = self.blacklist_current_image() {
                        eprintln!("Failed to blacklist image: {}", e);
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

fn main() -> Result<()> {
    // Initialize app
    let mut app = BingCliApp::new()?;
    app.initialize()?;
    
    println!("BingTray started successfully!");
    
    // Run the CLI menu
    app.run()?;
    
    Ok(())
}

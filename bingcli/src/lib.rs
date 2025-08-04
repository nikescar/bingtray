pub use crate::app::BingCliApp;

mod app {
    use anyhow::Result;
    use bingtray_core::*;
    use chrono::Utc;
    use rand::seq::SliceRandom;
    use std::io::{self, Write};
    use std::path::PathBuf;

    pub struct BingCliApp {
        config: Config,
        current_image: Option<PathBuf>,
    }

    impl BingCliApp {
        pub fn new() -> Result<Self> {
            let config = Config::new()?;
            
            Ok(Self {
                config,
                current_image: None,
            })
        }
        
        pub fn initialize(&mut self) -> Result<()> {
            // Load or create market codes
            let mut market_codes = load_market_codes(&self.config)?;
            
            // Check if we need to download images
            if need_more_images(&self.config)? {
                self.download_new_images(&mut market_codes)?;
            }
            
            // Set initial wallpaper
            self.set_next_market_wallpaper()?;

            // Set kept wallpaper when program loads
            // if self.has_kept_wallpapers_available() {
            //     if let Err(e) = self.set_kept_wallpaper() {
            //         eprintln!("Failed to set kept wallpaper: {}", e);
            //     }
            // }
            
            Ok(())
        }
        
        fn download_new_images(&mut self, market_codes: &mut std::collections::HashMap<String, i64>) -> Result<()> {
            let old_codes = get_old_market_codes(market_codes);
            
            if let Some(market_code) = old_codes.choose(&mut rand::thread_rng()) {
                println!("Downloading images for market code: {}", market_code);
                let (count, _downloaded_images) = download_images_for_market(&self.config, market_code)?;
                println!("Downloaded {} images", count);
                
                // Update timestamp
                market_codes.insert(market_code.clone(), Utc::now().timestamp());
                save_market_codes(&self.config, market_codes)?;
            }
            
            Ok(())
        }
        
        pub fn set_next_market_wallpaper(&mut self) -> Result<bool> {
            let mut market_codes = load_market_codes(&self.config)?;
            self.download_new_images(&mut market_codes)?;

            if let Some(image_path) = get_next_image(&self.config)? {
                if set_wallpaper(&image_path)? {
                    self.current_image = Some(image_path.clone());
                    println!("Set wallpaper: {}", image_path.display());
                    return Ok(true);
                }
            }

            Ok(true)
        }
        
        pub fn get_current_image_title(&self) -> String {
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
        
        pub fn keep_current_image(&mut self) -> Result<()> {
            if let Some(ref image_path) = self.current_image.clone() {
                move_to_keepfavorite(&self.config, image_path)?;
                println!("Moved to favorites: {}", image_path.display());
                
                // Check if we need more images after moving this one
                if need_more_images(&self.config)? {
                    let mut market_codes = load_market_codes(&self.config)?;
                    self.download_new_images(&mut market_codes)?;
                }
                
                self.set_next_market_wallpaper()?;
            }

            Ok(())
        }
        
        pub fn blacklist_current_image(&mut self) -> Result<()> {
            if let Some(ref image_path) = self.current_image.clone() {
                blacklist_image(&self.config, image_path)?;
                println!("Blacklisted: {}", image_path.display());
                
                // Check if we need more images after blacklisting this one
                if need_more_images(&self.config)? {
                    let mut market_codes = load_market_codes(&self.config)?;
                    self.download_new_images(&mut market_codes)?;
                }
                
                self.set_next_market_wallpaper()?;
            }
            
            Ok(())
        }
        
        pub fn set_kept_wallpaper(&mut self) -> Result<bool> {
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

        pub fn has_next_market_wallpaper_available(&self) -> bool {
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

        pub fn can_keep_current_image(&self) -> bool {
            if let Some(ref image_path) = self.current_image {
                // Check if the image is not already in keepfavorite folder
                // AND check if there are files in unprocessed folder
                !image_path.starts_with(&self.config.keepfavorite_dir) && 
                !need_more_images(&self.config).unwrap_or(true)
            } else {
                false // No current image
            }
        }

        pub fn can_blacklist_current_image(&self) -> bool {
            // Can blacklist if there's a current image AND there are files in unprocessed folder
            self.current_image.is_some() && !need_more_images(&self.config).unwrap_or(true)
        }

        pub fn has_kept_wallpapers_available(&self) -> bool {
            if let Ok(entries) = std::fs::read_dir(&self.config.keepfavorite_dir) {
                let kept_count = entries
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| {
                        entry.path().extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| ext.to_lowercase() == "jpg")
                            .unwrap_or(false)
                    })
                    .count();
                
                kept_count > 0
            } else {
                false
            }
        }

        pub fn has_unprocessed_files(&self) -> bool {
            !need_more_images(&self.config).unwrap_or(true)
        }

        pub fn is_current_image_in_favorites(&self) -> bool {
            if let Some(ref current_image) = self.current_image {
                current_image.starts_with(&self.config.keepfavorite_dir)
            } else {
                false
            }
        }

        pub fn get_status_info(&self) -> (String, String, usize) {
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
        
        pub fn get_market_status(&self) -> (String, usize) {
            // Get market codes info
            let market_codes = bingtray_core::load_market_codes(&self.config).unwrap_or_default();
            let old_codes = bingtray_core::get_old_market_codes(&market_codes);
            let available_count = old_codes.len();
            
            // Find the most recently used market code (highest timestamp)
            let last_tried = market_codes
                .iter()
                .max_by_key(|(_, &timestamp)| timestamp)
                .map(|(code, _)| code.to_string())
                .unwrap_or_else(|| "none".to_string());
            
            (last_tried, available_count)
        }
        
        pub fn show_menu(&self) {
            let title = self.get_current_image_title();
            let (copyright_text, copyrightlink) = self.get_current_image_copyright();
            
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
            println!("{}", copyright_text);
            println!("{}", copyrightlink);
            println!("Last tried market: {} | Available markets: {}", last_tried, available_count);
            println!();
            
            let has_next_available = self.has_next_market_wallpaper_available();
            let can_keep = self.can_keep_current_image();
            let can_blacklist = self.can_blacklist_current_image();
            let has_kept_available = self.has_kept_wallpapers_available();
            
            println!("0. Cache Dir Contents");
            if has_next_available {
                println!("1. Next Market wallpaper");
            } else {
                println!("1. Next Market wallpaper (unavailable - no images/markets)");
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
            
            if has_kept_available {
                println!("4. Next Kept wallpaper");
            } else {
                println!("4. Next Kept wallpaper (unavailable - no kept wallpapers)");
            }
            println!("5. Exit");
            print!("\nSelect an option (0-5): ");
            io::stdout().flush().unwrap();
        }

        pub fn run(&mut self) -> Result<()> {
            loop {
                self.show_menu();
                
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                
                match input.trim() {
                    "0" => {
                        if let Err(e) = self.open_cache_directory() {
                            eprintln!("Failed to open cache directory: {}", e);
                        } else {
                            println!("Cache directory opened in file manager");
                        }
                    }
                    "1" => {
                        if self.has_next_market_wallpaper_available() {
                            if let Err(e) = self.set_next_market_wallpaper() {
                                eprintln!("Failed to set next market wallpaper: {}", e);
                            }
                        } else {
                            println!("Next market wallpaper is not available - no images in unprocessed folder and no available market codes");
                        }
                    }
                    "2" => {
                        if self.can_keep_current_image() {
                            if let Err(e) = self.keep_current_image() {
                                eprintln!("Failed to keep image: {}", e);
                            }
                        } else {
                            if self.current_image.is_none() {
                                println!("Keep current image is not available - no current image");
                            } else if let Some(ref image_path) = self.current_image {
                                if image_path.starts_with(&self.config.keepfavorite_dir) {
                                    println!("Keep current image is not available - image is already in favorites");
                                } else {
                                    println!("Keep current image is not available - no files in unprocessed folder");
                                }
                            }
                        }
                    }
                    "3" => {
                        if self.can_blacklist_current_image() {
                            if let Err(e) = self.blacklist_current_image() {
                                eprintln!("Failed to blacklist image: {}", e);
                            }
                        } else {
                            if self.current_image.is_none() {
                                println!("Blacklist current image is not available - no current image");
                            } else {
                                println!("Blacklist current image is not available - no files in unprocessed folder");
                            }
                        }
                    }
                    "4" => {
                        if self.has_kept_wallpapers_available() {
                            if let Err(e) = self.set_kept_wallpaper() {
                                eprintln!("Failed to set kept wallpaper: {}", e);
                            }
                        } else {
                            println!("Next kept wallpaper is not available - no kept wallpapers in favorites folder");
                        }
                    }
                    "5" => {
                        println!("Exiting BingTray...");
                        break;
                    }
                    _ => {
                        println!("Invalid option. Please select 0-5.");
                    }
                }
            }
            
            Ok(())
        }
        

        pub fn get_current_image_copyright(&self) -> (String, String) {
            if let Some(ref image_path) = self.current_image {
                if let Some(filename) = image_path.file_stem().and_then(|s| s.to_str()) {
                    // Get from metadata.conf
                    if let Some((copyright_text, copyrightlink)) = get_image_metadata(&self.config, filename) {
                        return (copyright_text, copyrightlink);
                    }
                }
            }
            ("(no copyright info)".to_string(), "".to_string())
        }

        pub fn open_cache_directory(&self) -> Result<()> {
            bingtray_core::open_config_directory(&self.config)
        }
    }
}

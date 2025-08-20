use anyhow::{Result, Context};
use std::fs;
use std::path::{Path, PathBuf};
use crate::{FileSystemService, DefaultServiceProvider};
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use std::process::Command;

#[cfg(not(target_arch = "wasm32"))]
use crate::core::request::BingImage;
#[cfg(not(target_arch = "wasm32"))]
use crate::core::database::is_blacklisted;

#[derive(Debug, Clone)]
pub struct Config {
    pub config_dir: PathBuf,
    pub unprocessed_dir: PathBuf,
    pub keepfavorite_dir: PathBuf,
    pub cached_dir: PathBuf,
    pub blacklist_file: PathBuf,
    pub marketcodes_file: PathBuf,
    pub metadata_file: PathBuf,
    pub historical_metadata_file: PathBuf,
}

impl Config {
    pub fn new() -> Result<Self> {
        Self::new_with_service(&DefaultServiceProvider)
    }
    
    pub fn new_with_service<S: FileSystemService>(service: &S) -> Result<Self> {
        #[cfg(target_os = "android")]
        {
            // Android-specific paths
            let config_dir = PathBuf::from("/data/data/pe.nikescar.bingtray/files");
            let cache_dir = PathBuf::from("/data/data/pe.nikescar.bingtray/cache");
            
            log::info!("Android config paths - config_dir: {:?}, cache_dir: {:?}", config_dir, cache_dir);
            
            let unprocessed_dir = cache_dir.join("unprocessed");
            let keepfavorite_dir = cache_dir.join("keepfavorite");
            let cached_dir = cache_dir.join("cached");
            let blacklist_file = config_dir.join("blacklist.conf");
            let marketcodes_file = config_dir.join("marketcodes.conf");
            let metadata_file = config_dir.join("metadata.conf");
            let historical_metadata_file = config_dir.join("historical.metadata.conf");
            
            // Create directories if they don't exist
            match fs::create_dir_all(&config_dir) {
                Ok(()) => log::info!("Successfully created config_dir: {:?}", config_dir),
                Err(e) => log::error!("Failed to create config_dir: {:?} - Error: {}", config_dir, e),
            }
            match fs::create_dir_all(&cache_dir) {
                Ok(()) => log::info!("Successfully created cache_dir: {:?}", cache_dir),
                Err(e) => log::error!("Failed to create cache_dir: {:?} - Error: {}", cache_dir, e),
            }
            match fs::create_dir_all(&unprocessed_dir) {
                Ok(()) => log::info!("Successfully created unprocessed_dir: {:?}", unprocessed_dir),
                Err(e) => log::error!("Failed to create unprocessed_dir: {:?} - Error: {}", unprocessed_dir, e),
            }
            match fs::create_dir_all(&keepfavorite_dir) {
                Ok(()) => log::info!("Successfully created keepfavorite_dir: {:?}", keepfavorite_dir),
                Err(e) => log::error!("Failed to create keepfavorite_dir: {:?} - Error: {}", keepfavorite_dir, e),
            }
            match fs::create_dir_all(&cached_dir) {
                Ok(()) => log::info!("Successfully created cached_dir: {:?}", cached_dir),
                Err(e) => log::error!("Failed to create cached_dir: {:?} - Error: {}", cached_dir, e),
            }
            
            // Create config files if they don't exist
            if !blacklist_file.exists() {
                match fs::write(&blacklist_file, "") {
                    Ok(()) => log::info!("Successfully created blacklist.conf: {:?}", blacklist_file),
                    Err(e) => log::error!("Failed to create blacklist.conf: {:?} - Error: {}", blacklist_file, e),
                }
            } else {
                log::info!("blacklist.conf already exists: {:?}", blacklist_file);
            }
            if !metadata_file.exists() {
                match fs::write(&metadata_file, "") {
                    Ok(()) => log::info!("Successfully created metadata.conf: {:?}", metadata_file),
                    Err(e) => log::error!("Failed to create metadata.conf: {:?} - Error: {}", metadata_file, e),
                }
            } else {
                log::info!("metadata.conf already exists: {:?}", metadata_file);
            }
            
            // Automatically generate market codes on Android initialization
            let should_generate_codes = if !marketcodes_file.exists() {
                log::info!("Marketcodes file doesn't exist, will generate market codes for Android");
                true
            } else {
                // Check if file is empty
                let file_size = fs::metadata(&marketcodes_file).map(|m| m.len()).unwrap_or(0);
                if file_size == 0 {
                    log::info!("Marketcodes file exists but is empty ({}bytes), will generate market codes for Android", file_size);
                    true
                } else {
                    log::info!("Marketcodes file already exists with {} bytes", file_size);
                    false
                }
            };
            
            if should_generate_codes {
                log::info!("Automatically generating market codes for Android...");
                match get_market_codes() {
                    Ok(codes) => {
                        log::info!("Successfully fetched {} market codes from web", codes.len());
                        let mut market_map = std::collections::HashMap::new();
                        for code in codes {
                            market_map.insert(code, 0);
                        }
                        match save_market_codes(&Config {
                            config_dir: config_dir.clone(),
                            unprocessed_dir: unprocessed_dir.clone(),
                            keepfavorite_dir: keepfavorite_dir.clone(),
                            cached_dir: cached_dir.clone(),
                            blacklist_file: blacklist_file.clone(),
                            marketcodes_file: marketcodes_file.clone(),
                            metadata_file: metadata_file.clone(),
                            historical_metadata_file: historical_metadata_file.clone(),
                        }, &market_map) {
                            Ok(()) => log::info!("Successfully saved {} market codes to Android config", market_map.len()),
                            Err(e) => log::error!("Failed to save market codes to Android config: {}", e),
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to fetch market codes from web for Android: {}, creating fallback codes", e);
                        let mut market_map = std::collections::HashMap::new();
                        let fallback_codes = vec![
                            "en-US".to_string(),
                            "en-GB".to_string(),
                            "de-DE".to_string(),
                            "fr-FR".to_string(),
                            "ja-JP".to_string(),
                            "zh-CN".to_string(),
                        ];
                        for code in fallback_codes {
                            market_map.insert(code, 0);
                        }
                        match save_market_codes(&Config {
                            config_dir: config_dir.clone(),
                            unprocessed_dir: unprocessed_dir.clone(),
                            keepfavorite_dir: keepfavorite_dir.clone(),
                            cached_dir: cached_dir.clone(),
                            blacklist_file: blacklist_file.clone(),
                            marketcodes_file: marketcodes_file.clone(),
                            metadata_file: metadata_file.clone(),
                            historical_metadata_file: historical_metadata_file.clone(),
                        }, &market_map) {
                            Ok(()) => log::info!("Successfully saved fallback market codes to Android config"),
                            Err(e) => log::error!("Failed to save fallback market codes to Android config: {}", e),
                        }
                    }
                }
            }
            
            log::info!("Config created with marketcodes_file: {:?}", marketcodes_file);
            
            Ok(Config {
                config_dir,
                unprocessed_dir,
                keepfavorite_dir,
                cached_dir,
                blacklist_file,
                marketcodes_file,
                metadata_file,
                historical_metadata_file,
            })
        }
        
        #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
        {
            let proj_dirs = service.get_project_dirs()
                .context("Failed to get project directories")?;
            
            let config_dir = proj_dirs.config_dir().clone();
            let unprocessed_dir = config_dir.join("unprocessed");
            let keepfavorite_dir = config_dir.join("keepfavorite");
            let cached_dir = config_dir.join("cached");
            let blacklist_file = config_dir.join("blacklist.conf");
            let marketcodes_file = config_dir.join("marketcodes.conf");
            let metadata_file = config_dir.join("metadata.conf");
            let historical_metadata_file = config_dir.join("historical.metadata.conf");

            // Create directories if they don't exist
            fs::create_dir_all(&config_dir)?;
            fs::create_dir_all(&unprocessed_dir)?;
            fs::create_dir_all(&keepfavorite_dir)?;
            fs::create_dir_all(&cached_dir)?;

            // Create blacklist.conf if it doesn't exist
            if !blacklist_file.exists() {
                fs::write(&blacklist_file, "")?;
            }

            // Create metadata.conf if it doesn't exist
            if !metadata_file.exists() {
                fs::write(&metadata_file, "")?;
            }

            Ok(Config {
                config_dir,
                unprocessed_dir,
                keepfavorite_dir,
                cached_dir,
                blacklist_file,
                marketcodes_file,
                metadata_file,
                historical_metadata_file,
            })
        }
        
        // WASM configuration - use in-memory paths
        #[cfg(target_arch = "wasm32")]
        {
            let config_dir = PathBuf::from("/tmp/bingtray");
            let unprocessed_dir = config_dir.join("unprocessed");
            let keepfavorite_dir = config_dir.join("keepfavorite");
            let cached_dir = config_dir.join("cached");
            let blacklist_file = config_dir.join("blacklist.conf");
            let marketcodes_file = config_dir.join("marketcodes.conf");
            let metadata_file = config_dir.join("metadata.conf");
            let historical_metadata_file = config_dir.join("historical.metadata.conf");

            Ok(Config {
                config_dir,
                unprocessed_dir,
                keepfavorite_dir,
                cached_dir,
                blacklist_file,
                marketcodes_file,
                metadata_file,
                historical_metadata_file,
            })
        }
    }
}

pub fn sanitize_filename(filename: &str) -> String {
    let sanitized = filename
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_string();
    
    // Limit filename length to avoid filesystem issues
    if sanitized.len() > 100 {
        sanitized.chars().take(100).collect()
    } else {
        sanitized
    }
}

pub fn get_next_image(config: &Config) -> Result<Option<PathBuf>> {
    loop {
        let entries = fs::read_dir(&config.unprocessed_dir)?;
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
            return Ok(None);
        }
        
        // Use a simple pseudo-random selection based on current time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as usize;
        let index = now % images.len();
        let selected_image = &images[index];
        
        return Ok(Some(selected_image.clone()));
    }
}

pub fn move_to_keepfavorite(config: &Config, image_path: &Path) -> Result<()> {
    if let Some(filename) = image_path.file_name() {
        let target_path = config.keepfavorite_dir.join(filename);
        fs::rename(image_path, target_path)?;
    }
    Ok(())
}

pub fn blacklist_image(config: &Config, image_path: &Path) -> Result<()> {
    // Extract hash from filename
    if let Some(filename) = image_path.file_stem().and_then(|s| s.to_str()) {
        // Add the full filename to blacklist
        let mut blacklist = fs::read_to_string(&config.blacklist_file).unwrap_or_default();
        blacklist.push_str(&format!("{}\n", filename));
        fs::write(&config.blacklist_file, blacklist)?;
    }
    
    // Remove the file
    fs::remove_file(image_path)?;
    Ok(())
}

pub fn need_more_images(config: &Config) -> Result<bool> {
    let unprocessed_count = fs::read_dir(&config.unprocessed_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_lowercase() == "jpg")
                .unwrap_or(false)
        })
        .count();
    
    Ok(unprocessed_count == 0)
}

#[cfg(not(target_os = "android"))]
pub fn open_config_directory(config: &Config) -> Result<()> {
    let config_path = &config.config_dir;
    
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(config_path)
            .spawn()?;
    }
    
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(config_path)
            .spawn()?;
    }
    
    #[cfg(target_os = "linux")]
    {
        // Try different file managers in order of preference
        let file_managers = ["xdg-open", "nautilus", "dolphin", "thunar", "pcmanfm", "nemo"];
        let mut opened = false;
        
        for fm in &file_managers {
            if let Ok(_child) = Command::new(fm)
                .arg(config_path)
                .spawn() 
            {
                opened = true;
                break;
            }
        }
        
        if !opened {
            eprintln!("Could not find a suitable file manager to open {}", config_path.display());
        }
    }
    
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn download_image(image: &BingImage, target_dir: &Path, config: &Config) -> Result<PathBuf> {
    use crate::core::request::run_async;
    use crate::core::database::save_image_metadata;
    
    let url = if image.url.starts_with("http") {
        image.url.clone()
    } else {
        format!("https://bing.com{}", image.url)
    };

    // Extract display_name from URL
    let display_name = image.url
        .split("th?id=")
        .nth(1)
        .and_then(|s| s.split('_').next())
        .unwrap_or(&image.title)
        .to_string();
    let filename = format!("{}.jpg", sanitize_filename(&display_name));
    let filepath = target_dir.join(&filename);
    
    // Check if file exists in keepfavorite folder
    let keepfavorite_path = target_dir.parent()
        .map(|parent| parent.join("keepfavorite").join(&filename));
    
    if let Some(keepfavorite_file) = keepfavorite_path {
        if keepfavorite_file.exists() {
            return Ok(filepath);
        }
    }
    
    if !filepath.exists() {
        let response = run_async(async move {
            reqwest::Client::new()
                .get(&url)
                .timeout(std::time::Duration::from_secs(30))
                .header("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")
                .header("Accept", "image/webp,image/apng,image/*,*/*;q=0.8")
                .header("Referer", "https://www.bing.com/")
                .send()
                .await
        })?;
        let bytes = run_async(async { response.bytes().await })?;
        fs::write(&filepath, bytes)?;
    }
    
    // Save metadata if available
    if let (Some(copyright), Some(copyrightlink)) = (&image.copyright, &image.copyrightlink) {
        save_image_metadata(config, &sanitize_filename(&display_name), copyright, copyrightlink)?;
    }
    
    Ok(filepath)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn download_thumbnail_image(image: &BingImage, config: &Config) -> Result<PathBuf> {
    use crate::core::request::run_async;
    use crate::core::database::save_image_metadata;
    
    // Convert the URL to a thumbnail URL with 320x240 dimensions
    let base_url = if image.url.starts_with("http") {
        image.url.clone()
    } else {
        format!("https://bing.com{}", image.url)
    };
    
    // Create thumbnail URL by adding w=320&h=240 parameters
    let thumbnail_url = if base_url.contains('?') {
        format!("{}&w=320&h=240", base_url)
    } else {
        format!("{}?w=320&h=240", base_url)
    };

    // Extract display_name from URL
    let display_name = image.url
        .split("th?id=")
        .nth(1)
        .and_then(|s| s.split('_').next())
        .unwrap_or(&image.title)
        .to_string();
    let filename = format!("{}_thumb.jpg", sanitize_filename(&display_name));
    let filepath = config.cached_dir.join(&filename);
    
    // Check if file exists in keepfavorite folder
    let keepfavorite_path = config.keepfavorite_dir.join(&filename);
    
    if keepfavorite_path.exists() {
        return Ok(filepath);
    }
    
    if !filepath.exists() {
        let response = run_async(async move {
            reqwest::Client::new()
                .get(&thumbnail_url)
                .timeout(std::time::Duration::from_secs(30))
                .header("User-Agent", "Mozilla/5.0 (Android 13; Mobile; rv:109.0) Gecko/111.0 Firefox/117.0")
                .header("Accept", "image/webp,image/apng,image/*,*/*;q=0.8")
                .header("Referer", "https://www.bing.com/")
                .send()
                .await
        })?;
        let bytes = run_async(async { response.bytes().await })?;
        fs::write(&filepath, bytes)?;
    }
    
    // Save metadata if available
    if let (Some(copyright), Some(copyrightlink)) = (&image.copyright, &image.copyrightlink) {
        save_image_metadata(config, &sanitize_filename(&display_name), copyright, copyrightlink)?;
    }
    
    Ok(filepath)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn download_images_for_market(config: &Config, market_code: &str, thumb_mode: bool) -> Result<(usize, Vec<BingImage>)> {
    use crate::core::request::get_bing_images;
    
    let images = get_bing_images(market_code)?;
    let mut downloaded_count = 0;
    let mut downloaded_images = Vec::new();
    
    for image in images.iter() {
        let mut display_name = image.url
            .split("th?id=")
            .nth(1)
            .and_then(|s| s.split('_').next())
            .unwrap_or(&image.title)
            .to_string();
        display_name = sanitize_filename(&display_name);

        let filename_suffix = if thumb_mode { "_thumb.jpg" } else { ".jpg" };
        let target_dir = if thumb_mode { &config.cached_dir } else { &config.unprocessed_dir };
        
        let target_path = target_dir.join(format!("{}{}", display_name, filename_suffix));
        let keepfavorite_path = config.keepfavorite_dir.join(format!("{}{}", display_name, filename_suffix));
        
        if !target_path.exists() && !keepfavorite_path.exists() && !is_blacklisted(config, &display_name)? {
            let download_result = if thumb_mode {
                download_thumbnail_image(&image, config)
            } else {
                download_image(&image, &config.unprocessed_dir, config)
            };
            
            match download_result {
                Ok(filepath) => {
                    let download_type = if thumb_mode { "thumbnail" } else { "image" };
                    println!("Downloaded {}: {}", download_type, filepath.display());
                    downloaded_count += 1;
                    downloaded_images.push((*image).clone());
                }
                Err(e) => {
                    let download_type = if thumb_mode { "thumbnail" } else { "image" };
                    eprintln!("Failed to download {} {}: {}", download_type, display_name, e);
                }
            }
        } else {
            let download_type = if thumb_mode { "thumbnail" } else { "image" };
            println!("Skipping already downloaded or blacklisted {}: {}", download_type, display_name);
        }
    }
    
    Ok((downloaded_count, downloaded_images))
}

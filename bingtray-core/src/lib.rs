use anyhow::{Context, Result};
use chrono::Utc;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingImage {
    pub url: String,
    pub title: String,
    pub copyright: Option<String>,
    pub copyrightlink: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BingResponse {
    pub images: Vec<BingImage>,
}

#[derive(Debug)]
pub struct Config {
    pub config_dir: PathBuf,
    pub unprocessed_dir: PathBuf,
    pub keepfavorite_dir: PathBuf,
    pub blacklist_file: PathBuf,
    pub marketcodes_file: PathBuf,
    pub metadata_file: PathBuf,
}

impl Config {
    pub fn new() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("com", "bingtray", "bingtray")
            .context("Failed to get project directories")?;
        
        let config_dir = proj_dirs.config_dir().to_path_buf();
        let unprocessed_dir = config_dir.join("unprocessed");
        let keepfavorite_dir = config_dir.join("keepfavorite");
        let blacklist_file = config_dir.join("blacklist.conf");
        let marketcodes_file = config_dir.join("marketcodes.conf");
        let metadata_file = config_dir.join("metadata.conf");

        // Create directories if they don't exist
        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&unprocessed_dir)?;
        fs::create_dir_all(&keepfavorite_dir)?;

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
            blacklist_file,
            marketcodes_file,
            metadata_file,
        })
    }
}

pub fn get_market_codes() -> Result<Vec<String>> {
    let url = "https://learn.microsoft.com/en-us/bing/search-apis/bing-web-search/reference/market-codes";
    let response = attohttpc::get(url).send()?;
    let html = response.text()?;
    
    let document = scraper::Html::parse_document(&html);
    let table_selector = scraper::Selector::parse("table").unwrap();
    let row_selector = scraper::Selector::parse("tr").unwrap();
    let cell_selector = scraper::Selector::parse("td").unwrap();
    
    let mut market_codes = Vec::new();
    
    for table in document.select(&table_selector) {
        for row in table.select(&row_selector).skip(1) { // Skip header row
            let cells: Vec<_> = row.select(&cell_selector).collect();
            if cells.len() >= 2 {
                if let Some(market_code) = cells.last() {
                    let code = market_code.text().collect::<String>().trim().to_string();
                    if !code.is_empty() && code.contains("-") {
                        market_codes.push(code);
                    }
                }
            }
        }
    }
    
    Ok(market_codes)
}

pub fn load_market_codes(config: &Config) -> Result<HashMap<String, i64>> {
    if !config.marketcodes_file.exists() {
        let codes = get_market_codes()?;
        let mut market_map = HashMap::new();
        for code in codes {
            market_map.insert(code, 0);
        }
        save_market_codes(config, &market_map)?;
        return Ok(market_map);
    }
    
    let content = fs::read_to_string(&config.marketcodes_file)?;
    let mut market_map = HashMap::new();
    
    for line in content.lines() {
        if let Some((code, timestamp)) = line.split_once('|') {
            if let Ok(ts) = timestamp.parse::<i64>() {
                market_map.insert(code.to_string(), ts);
            }
        }
    }
    
    Ok(market_map)
}

pub fn save_market_codes(config: &Config, market_codes: &HashMap<String, i64>) -> Result<()> {
    let mut content = String::new();
    for (code, timestamp) in market_codes {
        content.push_str(&format!("{}|{}\n", code, timestamp));
    }
    fs::write(&config.marketcodes_file, content)?;
    Ok(())
}

pub fn get_bing_images(market_code: &str) -> Result<Vec<BingImage>> {
    let url = format!("https://bing.com/HPImageArchive.aspx?format=js&idx=0&n=8&mkt={}", market_code);
    let response = attohttpc::get(&url).send()?;
    let text = response.text()?;
    let bing_response: BingResponse = serde_json::from_str(&text)?;
    Ok(bing_response.images)
}

pub fn download_image(image: &BingImage, target_dir: &Path, config: &Config) -> Result<PathBuf> {
    let url = if image.url.starts_with("http") {
        image.url.clone()
    } else {
        format!("https://bing.com{}", image.url)
    };

    // image.url looks like this "/th?id=OHR.TemplePhilae_EN-US5062419351_1920x1080.jpg&rf=LaDigue_1920x1080.jpg&pid=hp"
    // please extract "OHR.TemplePhilae" part and set it to display_name
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
            // File already exists in keepfavorite, skip download
            return Ok(filepath);
        }
    }
    
    if !filepath.exists() {
        let response = attohttpc::get(&url).send()?;
        let bytes = response.bytes()?;
        fs::write(&filepath, bytes)?;
    }
    
    // Save metadata if available
    if let (Some(copyright), Some(copyrightlink)) = (&image.copyright, &image.copyrightlink) {
        save_image_metadata(config, &sanitize_filename(&display_name), copyright, copyrightlink)?;
    }
    
    Ok(filepath)
}

pub fn sanitize_filename(filename: &str) -> String {
    let sanitized = filename
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_string();
    
    // Limit filename length to avoid filesystem issues
    // Keep it reasonable while preserving readability
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
        
        // // Check if the image is blacklisted
        // if let Some(filename) = selected_image.file_stem().and_then(|s| s.to_str()) {
        //     if is_blacklisted(config, filename)? {
        //         // Remove the blacklisted file and continue searching
        //         if let Err(e) = fs::remove_file(selected_image) {
        //             eprintln!("Warning: Failed to remove blacklisted file {}: {}", 
        //                      selected_image.display(), e);
        //         }
        //         continue; // Try again with remaining files
        //     }
        // }
        
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

pub fn is_blacklisted(config: &Config, filename: &str) -> Result<bool> {
    let blacklist = fs::read_to_string(&config.blacklist_file).unwrap_or_default();
    println!("Checking if {} is blacklisted : {}", filename, blacklist.lines().any(|line| line.trim() == filename));
    Ok(blacklist.lines().any(|line| line.trim() == filename))
}

pub fn get_desktop_environment() -> String {
    if let Ok(desktop_session) = std::env::var("DESKTOP_SESSION") {
        let session = desktop_session.to_lowercase();
        if ["gnome", "unity", "cinnamon", "mate", "xfce4", "lxde", "fluxbox", 
            "blackbox", "openbox", "icewm", "jwm", "afterstep", "trinity", "kde"].contains(&session.as_str()) {
            return session;
        }
        
        if session.contains("xfce") || session.starts_with("xubuntu") {
            return "xfce4".to_string();
        } else if session.starts_with("ubuntustudio") {
            return "kde".to_string();
        } else if session.starts_with("ubuntu") {
            return "gnome".to_string();
        } else if session.starts_with("lubuntu") {
            return "lxde".to_string();
        } else if session.starts_with("kubuntu") {
            return "kde".to_string();
        }
    }
    
    if std::env::var("KDE_FULL_SESSION").unwrap_or_default() == "true" {
        return "kde".to_string();
    }
    
    if std::env::var("GNOME_DESKTOP_SESSION_ID").is_ok() {
        return "gnome".to_string();
    }
    
    "unknown".to_string()
}

pub fn set_wallpaper(file_path: &Path) -> Result<bool> {
    let file_loc = file_path.to_string_lossy();
    
    // Use wallpaper crate for cross-platform wallpaper setting
    match wallpaper::set_from_path(&file_loc) {
        Ok(_) => {
            println!("Wallpaper set successfully to: {}", file_loc);
            Ok(true)
        }
        Err(e) => {
            eprintln!("Failed to set wallpaper: {}", e);
            
            // Fallback to platform-specific methods for Linux if wallpaper crate fails
            if cfg!(target_os = "linux") {
                return set_wallpaper_linux_fallback(file_path);
            }
            
            Ok(false)
        }
    }
}

fn set_wallpaper_linux_fallback(file_path: &Path) -> Result<bool> {
    let file_loc = file_path.to_string_lossy();
    let desktop_env = get_desktop_environment();
    
    match desktop_env.as_str() {
        "gnome" | "unity" | "cinnamon" => {
            let uri = format!("file://{}", file_loc);
            let output = Command::new("gsettings")
                .args(&["set", "org.gnome.desktop.background", "picture-uri", &uri])
                .output()?;
            Ok(output.status.success())
        }
        "mate" => {
            let output = Command::new("gsettings")
                .args(&["set", "org.mate.background", "picture-filename", &file_loc])
                .output()?;
            Ok(output.status.success())
        }
        "xfce4" => {
            // Get all monitor paths that contain "workspace0/last-image"
            let list_output = Command::new("xfconf-query")
                .args(&["-c", "xfce4-desktop", "-l"])
                .output()?;
            
            if list_output.status.success() {
                let paths = String::from_utf8_lossy(&list_output.stdout);
                let monitor_paths: Vec<&str> = paths
                    .lines()
                    .filter(|line| line.contains("workspace0/last-image"))
                    .collect();
                
                // Set wallpaper for each monitor
                for path in monitor_paths {
                    if !path.trim().is_empty() {
                        Command::new("xfconf-query")
                            .args(&["-c", "xfce4-desktop", "-p", path.trim(), "-s", &file_loc])
                            .output()?;
                    }
                }
            }
            
            // Set default properties for the primary monitor as fallback
            Command::new("xfconf-query")
                .args(&["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/image-path", "-s", &file_loc])
                .output()?;
            Command::new("xfconf-query")
                .args(&["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/image-style", "-s", "3"])
                .output()?;
            Command::new("xfconf-query")
                .args(&["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/image-show", "-s", "true"])
                .output()?;
            
            let output = Command::new("xfdesktop")
                .args(&["--reload"])
                .output()?;
            Ok(output.status.success())
        }
        "lxde" => {
            let cmd = format!("pcmanfm --set-wallpaper {} --wallpaper-mode=scaled", file_loc);
            let output = Command::new("sh")
                .args(&["-c", &cmd])
                .output()?;
            Ok(output.status.success())
        }
        "fluxbox" | "jwm" | "openbox" | "afterstep" => {
            let output = Command::new("fbsetbg")
                .arg(file_loc.as_ref())
                .output()?;
            Ok(output.status.success())
        }
        "icewm" => {
            let output = Command::new("icewmbg")
                .arg(file_loc.as_ref())
                .output()?;
            Ok(output.status.success())
        }
        "blackbox" => {
            let output = Command::new("bsetbg")
                .args(&["-full", &file_loc])
                .output()?;
            Ok(output.status.success())
        }
        _ => {
            eprintln!("Desktop environment '{}' not supported", desktop_env);
            Ok(false)
        }
    }
}

pub fn download_images_for_market(config: &Config, market_code: &str) -> Result<(usize, Vec<BingImage>)> {
    let images = get_bing_images(market_code)?;
    let mut downloaded_count = 0;
    let mut downloaded_images = Vec::new();
    
    for (_, image) in images.iter().enumerate() {
        let mut display_name = image.url
            .split("th?id=")
            .nth(1)
            .and_then(|s| s.split('_').next())
            .unwrap_or(&image.title)
            .to_string();
        display_name = sanitize_filename(&display_name);

        let unprocessed_path = config.unprocessed_dir.join(format!("{}.jpg", display_name));
        let keepfavorite_path = config.keepfavorite_dir.join(format!("{}.jpg", display_name));
        if !unprocessed_path.exists() && !keepfavorite_path.exists() && !is_blacklisted(config, &display_name)? {
            match download_image(&image, &config.unprocessed_dir, config) {
                Ok(filepath) => {
                    println!("Downloaded image: {}", filepath.display());
                    downloaded_count += 1;
                    downloaded_images.push((*image).clone());
                }
                Err(e) => {
                    eprintln!("Failed to download image {}: {}", display_name, e);
                }
            }
        } else {
            println!("Skipping already downloaded or blacklisted image: {}", display_name);
        }
    }
    
    Ok((downloaded_count, downloaded_images))
}

pub fn save_image_metadata(config: &Config, filename: &str, copyright: &str, copyrightlink: &str) -> Result<()> {
    let metadata = fs::read_to_string(&config.metadata_file).unwrap_or_default();
    
    // Extract text in parentheses from copyright
    let copyright_text = if let Some(start) = copyright.find('(') {
        if let Some(end) = copyright.find(')') {
            if end > start {
                copyright[start+1..end].to_string()
            } else {
                copyright.to_string()
            }
        } else {
            copyright.to_string()
        }
    } else {
        copyright.to_string()
    };
    
    // Check if entry already exists and replace it
    let mut lines: Vec<String> = metadata.lines().map(|s| s.to_string()).collect();
    let mut found = false;
    
    for line in &mut lines {
        if line.starts_with(&format!("{}|", filename)) {
            *line = format!("{}|{}|{}", filename, copyright_text, copyrightlink);
            found = true;
            break;
        }
    }
    
    if !found {
        lines.push(format!("{}|{}|{}", filename, copyright_text, copyrightlink));
    }
    
    let content = lines.join("\n");
    if !content.is_empty() {
        fs::write(&config.metadata_file, content + "\n")?;
    }
    
    Ok(())
}

pub fn get_image_metadata(config: &Config, filename: &str) -> Option<(String, String)> {
    let metadata = fs::read_to_string(&config.metadata_file).unwrap_or_default();
    for line in metadata.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 3 && parts[0] == filename {
            return Some((parts[1].to_string(), parts[2].to_string()));
        }
    }
    None
}

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

pub fn get_old_market_codes(market_codes: &HashMap<String, i64>) -> Vec<String> {
    let now = Utc::now().timestamp();
    let seven_days_ago = now - (7 * 24 * 60 * 60);
    
    market_codes
        .iter()
        .filter(|(_, &timestamp)| timestamp < seven_days_ago)
        .map(|(code, _)| code.clone())
        .collect()
}

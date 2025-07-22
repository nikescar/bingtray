use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct BingImage {
    pub url: String,
    pub title: String,
    pub hsh: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BingResponse {
    pub images: Vec<BingImage>,
}

#[derive(Debug)]
pub struct MarketCode {
    pub code: String,
    pub last_visit: u64,
}

pub struct BingTray {
    config_dir: PathBuf,
    unprocessed_dir: PathBuf,
    keepfavorite_dir: PathBuf,
    blacklist_file: PathBuf,
    marketcodes_file: PathBuf,
}

impl BingTray {
    pub fn new() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("com", "bingtray", "bingtray")
            .ok_or_else(|| anyhow!("Failed to get project directories"))?;
        
        let config_dir = proj_dirs.config_dir().to_path_buf();
        let unprocessed_dir = config_dir.join("unprocessed");
        let keepfavorite_dir = config_dir.join("keepfavorite");
        let blacklist_file = config_dir.join("blacklist.conf");
        let marketcodes_file = config_dir.join("marketcodes.conf");

        Ok(BingTray {
            config_dir,
            unprocessed_dir,
            keepfavorite_dir,
            blacklist_file,
            marketcodes_file,
        })
    }

    pub async fn initialize(&self) -> Result<()> {
        // Create config directory if not exists
        fs::create_dir_all(&self.config_dir)?;
        
        // Create blacklist.conf if not exists
        if !self.blacklist_file.exists() {
            fs::write(&self.blacklist_file, "")?;
        }

        // Create directories if not exists
        fs::create_dir_all(&self.unprocessed_dir)?;
        fs::create_dir_all(&self.keepfavorite_dir)?;

        // Initialize market codes if not exists
        if !self.marketcodes_file.exists() {
            self.initialize_market_codes().await?;
        }

        // Download images if unprocessed directory is empty
        if self.is_unprocessed_empty()? {
            self.download_images_from_random_market().await?;
        }

        Ok(())
    }

    async fn initialize_market_codes(&self) -> Result<()> {
        // Hardcoded market codes based on Bing API documentation
        let market_codes = vec![
            "ar-XA", "bg-BG", "cs-CZ", "da-DK", "de-AT", "de-CH", "de-DE",
            "el-GR", "en-AU", "en-CA", "en-GB", "en-ID", "en-IE", "en-IN",
            "en-MY", "en-NZ", "en-PH", "en-SG", "en-US", "en-XA", "en-ZA",
            "es-AR", "es-CL", "es-ES", "es-MX", "es-US", "es-XL", "et-EE",
            "fi-FI", "fr-BE", "fr-CA", "fr-CH", "fr-FR", "he-IL", "hr-HR",
            "hu-HU", "it-IT", "ja-JP", "ko-KR", "lt-LT", "lv-LV", "nb-NO",
            "nl-BE", "nl-NL", "pl-PL", "pt-BR", "pt-PT", "ro-RO", "ru-RU",
            "sk-SK", "sl-SL", "sv-SE", "th-TH", "tr-TR", "uk-UA", "zh-CN",
            "zh-HK", "zh-TW"
        ];

        let mut content = String::new();
        for code in market_codes {
            content.push_str(&format!("{}|0\n", code));
        }

        fs::write(&self.marketcodes_file, content)?;
        Ok(())
    }

    fn is_unprocessed_empty(&self) -> Result<bool> {
        let entries = fs::read_dir(&self.unprocessed_dir)?;
        Ok(entries.count() == 0)
    }

    pub async fn download_images_from_random_market(&self) -> Result<()> {
        let market_codes = self.load_market_codes()?;
        let mut rng = rand::thread_rng();
        
        if let Some(market_code) = market_codes.choose(&mut rng) {
            self.download_images(&market_code.code).await?;
            self.update_market_code_timestamp(&market_code.code)?;
        }

        Ok(())
    }

    pub async fn download_images(&self, market_code: &str) -> Result<()> {
        let url = format!("https://bing.com/HPImageArchive.aspx?format=js&idx=0&n=8&mkt={}", market_code);
        
        let client = reqwest::Client::new();
        let response = client.get(&url).send().await?;
        let bing_response: BingResponse = response.json().await?;

        let blacklist = self.load_blacklist()?;

        for image in bing_response.images {
            // Skip if in blacklist
            if blacklist.contains(&image.hsh) {
                continue;
            }

            let filename = format!("{}.{}.jpg", 
                image.title.replace("/", "_").replace("\\", "_"), 
                image.hsh);
            let filepath = self.unprocessed_dir.join(&filename);

            // Skip if already exists
            if filepath.exists() {
                continue;
            }

            // Download image
            let mut download_url = image.url;
            if !download_url.starts_with("http") {
                download_url = format!("https://bing.com{}", download_url);
            }

            let image_response = client.get(&download_url).send().await?;
            let image_bytes = image_response.bytes().await?;
            fs::write(&filepath, &image_bytes)?;
        }

        Ok(())
    }

    fn load_market_codes(&self) -> Result<Vec<MarketCode>> {
        let content = fs::read_to_string(&self.marketcodes_file)?;
        let mut market_codes = Vec::new();

        for line in content.lines() {
            if let Some((code, timestamp_str)) = line.split_once('|') {
                if let Ok(timestamp) = timestamp_str.parse::<u64>() {
                    market_codes.push(MarketCode {
                        code: code.to_string(),
                        last_visit: timestamp,
                    });
                }
            }
        }

        Ok(market_codes)
    }

    fn update_market_code_timestamp(&self, market_code: &str) -> Result<()> {
        let market_codes = self.load_market_codes()?;
        let current_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();

        let mut content = String::new();
        for mc in market_codes {
            if mc.code == market_code {
                content.push_str(&format!("{}|{}\n", mc.code, current_timestamp));
            } else {
                content.push_str(&format!("{}|{}\n", mc.code, mc.last_visit));
            }
        }

        fs::write(&self.marketcodes_file, content)?;
        Ok(())
    }

    fn load_blacklist(&self) -> Result<Vec<String>> {
        if !self.blacklist_file.exists() {
            return Ok(Vec::new());
        }
        
        let content = fs::read_to_string(&self.blacklist_file)?;
        Ok(content.lines().map(|s| s.to_string()).collect())
    }

    pub fn get_next_wallpaper(&self) -> Result<Option<PathBuf>> {
        let entries = fs::read_dir(&self.unprocessed_dir)?;
        let mut files: Vec<_> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().map_or(false, |ext| ext == "jpg"))
            .collect();

        files.sort_by_key(|entry| entry.file_name());
        Ok(files.first().map(|entry| entry.path()))
    }

    pub fn consume_next_wallpaper(&self) -> Result<Option<PathBuf>> {
        if let Some(wallpaper_path) = self.get_next_wallpaper()? {
            // Move to a temporary consumed directory to avoid re-using
            let consumed_dir = self.config_dir.join("consumed");
            fs::create_dir_all(&consumed_dir)?;
            
            if let Some(filename) = wallpaper_path.file_name() {
                let dest = consumed_dir.join(filename);
                fs::rename(&wallpaper_path, dest)?;
            }
            
            Ok(Some(wallpaper_path))
        } else {
            Ok(None)
        }
    }

    pub fn keep_current_wallpaper(&self, filepath: &Path) -> Result<()> {
        if let Some(filename) = filepath.file_name() {
            let dest = self.keepfavorite_dir.join(filename);
            fs::rename(filepath, dest)?;
        }
        Ok(())
    }

    pub fn blacklist_current_wallpaper(&self, filepath: &Path) -> Result<()> {
        // Extract hash from filename
        if let Some(filename) = filepath.file_name().and_then(|f| f.to_str()) {
            // Expected format: "title.hash.jpg"
            let parts: Vec<&str> = filename.rsplitn(3, '.').collect();
            if parts.len() >= 3 {
                let hash_part = parts[1]; // The hash is the second part from the end
                
                // Add to blacklist
                let mut blacklist = self.load_blacklist()?;
                if !blacklist.contains(&hash_part.to_string()) {
                    blacklist.push(hash_part.to_string());
                    let content = blacklist.join("\n");
                    fs::write(&self.blacklist_file, content)?;
                }
            }
        }
        
        // Remove file
        fs::remove_file(filepath)?;
        Ok(())
    }

    pub async fn check_and_download_more_images(&self) -> Result<()> {
        if self.is_unprocessed_empty()? {
            let market_codes = self.load_market_codes()?;
            let current_timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs();

            // Find market codes not visited in 7 days
            let seven_days_ago = current_timestamp - (7 * 24 * 60 * 60);
            
            for market_code in market_codes {
                if market_code.last_visit < seven_days_ago {
                    self.download_images(&market_code.code).await?;
                    self.update_market_code_timestamp(&market_code.code)?;
                    break; // Download from one market at a time
                }
            }
        }
        Ok(())
    }

    pub fn get_current_wallpaper_info(&self) -> Result<Option<(PathBuf, String)>> {
        if let Some(wallpaper_path) = self.get_next_wallpaper()? {
            let title = self.extract_title_from_filename(&wallpaper_path)?;
            Ok(Some((wallpaper_path, title)))
        } else {
            Ok(None)
        }
    }

    pub fn consume_current_wallpaper(&self, wallpaper_path: &Path) -> Result<()> {
        // Move to a temporary consumed directory to avoid re-using
        let consumed_dir = self.config_dir.join("consumed");
        fs::create_dir_all(&consumed_dir)?;
        
        if let Some(filename) = wallpaper_path.file_name() {
            let dest = consumed_dir.join(filename);
            if wallpaper_path.exists() {
                fs::rename(wallpaper_path, dest)?;
            }
        }
        
        Ok(())
    }

    fn extract_title_from_filename(&self, filepath: &Path) -> Result<String> {
        if let Some(filename) = filepath.file_name().and_then(|f| f.to_str()) {
            // Expected format: "title.hash.jpg"
            let parts: Vec<&str> = filename.rsplitn(3, '.').collect();
            if parts.len() >= 3 {
                let title_part = parts[2]; // The title is the third part from the end
                return Ok(title_part.replace("_", " "));
            }
        }
        Ok("Unknown".to_string())
    }
}

pub fn get_desktop_environment() -> String {
    if let Ok(desktop_session) = env::var("DESKTOP_SESSION") {
        let desktop_session = desktop_session.to_lowercase();
        
        if ["gnome", "unity", "cinnamon", "mate", "xfce4", "lxde", "fluxbox",
            "blackbox", "openbox", "icewm", "jwm", "afterstep", "trinity", "kde"]
            .contains(&desktop_session.as_str()) {
            return desktop_session;
        }
        
        if desktop_session.contains("xfce") || desktop_session.starts_with("xubuntu") {
            return "xfce4".to_string();
        } else if desktop_session.starts_with("ubuntustudio") {
            return "kde".to_string();
        } else if desktop_session.starts_with("ubuntu") {
            return "gnome".to_string();
        } else if desktop_session.starts_with("lubuntu") {
            return "lxde".to_string();
        } else if desktop_session.starts_with("kubuntu") {
            return "kde".to_string();
        } else if desktop_session.starts_with("razor") {
            return "razor-qt".to_string();
        } else if desktop_session.starts_with("wmaker") {
            return "windowmaker".to_string();
        }
    }

    if env::var("KDE_FULL_SESSION").map_or(false, |v| v == "true") {
        return "kde".to_string();
    }

    if env::var("GNOME_DESKTOP_SESSION_ID").is_ok() {
        return "gnome2".to_string();
    }

    "unknown".to_string()
}

pub fn set_wallpaper(file_path: &Path) -> Result<()> {
    let desktop_env = get_desktop_environment();
    let file_loc = file_path.to_str()
        .ok_or_else(|| anyhow!("Invalid file path"))?;

    match desktop_env.as_str() {
        "gnome" | "unity" | "cinnamon" => {
            let uri = format!("file://{}", file_loc);
            let output = Command::new("gsettings")
                .args(["set", "org.gnome.desktop.background", "picture-uri", &uri])
                .output()?;
            
            if !output.status.success() {
                return Err(anyhow!("Failed to set wallpaper via gsettings"));
            }
        }
        "mate" => {
            let output = Command::new("gsettings")
                .args(["set", "org.mate.background", "picture-filename", file_loc])
                .output()?;
            
            if !output.status.success() {
                // Try older MATE version
                Command::new("mateconftool-2")
                    .args(["-t", "string", "--set", "/desktop/mate/background/picture_filename", file_loc])
                    .output()?;
            }
        }
        "xfce4" => {
            Command::new("xfconf-query")
                .args(["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/image-path", "-s", file_loc])
                .output()?;
            Command::new("xfconf-query")
                .args(["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/image-style", "-s", "3"])
                .output()?;
            Command::new("xfconf-query")
                .args(["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/image-show", "-s", "true"])
                .output()?;
            Command::new("xfdesktop")
                .args(["--reload"])
                .output()?;
        }
        "kde" => {
            // KDE wallpaper setting is complex, skipping for now
            return Err(anyhow!("KDE wallpaper setting not implemented"));
        }
        "lxde" => {
            let command = format!("pcmanfm --set-wallpaper {} --wallpaper-mode=scaled", file_loc);
            Command::new("sh")
                .args(["-c", &command])
                .output()?;
        }
        "fluxbox" | "jwm" | "openbox" | "afterstep" => {
            Command::new("fbsetbg")
                .args([file_loc])
                .output()?;
        }
        "icewm" => {
            Command::new("icewmbg")
                .args([file_loc])
                .output()?;
        }
        "blackbox" => {
            Command::new("bsetbg")
                .args(["-full", file_loc])
                .output()?;
        }
        "windowmaker" => {
            let command = format!("wmsetbg -s -u {}", file_loc);
            Command::new("sh")
                .args(["-c", &command])
                .output()?;
        }
        _ => {
            return Err(anyhow!("Unsupported desktop environment: {}", desktop_env));
        }
    }

    Ok(())
}

#![allow(clippy::float_cmp)]
#![allow(clippy::manual_range_contains)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[cfg(not(target_os = "android"))]
use directories::ProjectDirs;

// Core modules
pub mod shared_store;
pub mod bingtray;
pub mod api_bingimage;
pub mod datafusion_bingimage;
pub mod calc_bingimage; // Now available on all platforms (contains cross-platform functions + desktop-only struct)
pub mod dlg_settings_stt;
pub mod dlg_settings;
pub mod dlg_about_stt;
pub mod dlg_about;
pub mod i18n;
pub mod ehttp_cache;

// Installation management (available on all platforms, but some functions desktop-only)
pub mod install_stt;
pub mod install;

// Desktop-only modules
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub mod api_setwallpaper;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub mod cli;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub mod tray;

// Android-only modules
#[cfg(target_os = "android")]
pub mod android_wallpaper;
#[cfg(target_os = "android")]
pub mod android_screensize;
#[cfg(target_os = "android")]
pub mod main_android;

// WASM-only modules
#[cfg(target_arch = "wasm32")]
pub mod main_wasm;

// Export main app
pub use bingtray::BingtrayApp;

/// Configuration for Bing wallpaper directories and files
#[derive(Debug, Clone)]
pub struct Config {
    pub config_dir: PathBuf,
    pub unprocessed_dir: PathBuf,
    pub keepfavorite_dir: PathBuf,
    pub cached_dir: PathBuf,
    pub image_cached_dir: PathBuf,
    pub db_path: PathBuf,
}

impl Config {
    pub fn new() -> Result<Self> {
        #[cfg(target_os = "android")]
        {
            // Android-specific paths
            let config_dir = PathBuf::from("/data/data/pe.nikescar.bingtray/files");
            let cache_dir = PathBuf::from("/data/data/pe.nikescar.bingtray/cache");

            log::info!(
                "Android config paths - config_dir: {:?}, cache_dir: {:?}",
                config_dir,
                cache_dir
            );

            let unprocessed_dir = cache_dir.join("unprocessed");
            let keepfavorite_dir = cache_dir.join("keepfavorite");
            let cached_dir = cache_dir.join("cached");
            let image_cached_dir = cache_dir.join("image_cache");

            // Create directories if they don't exist
            for dir in [&config_dir, &cache_dir, &unprocessed_dir, &keepfavorite_dir, &cached_dir, &image_cached_dir] {
                match fs::create_dir_all(dir) {
                    Ok(()) => log::info!("Successfully created directory: {:?}", dir),
                    Err(e) => log::error!("Failed to create directory: {:?} - Error: {}", dir, e),
                }
            }

            Ok(Config {
                config_dir: config_dir.clone(),
                unprocessed_dir,
                keepfavorite_dir,
                cached_dir,
                image_cached_dir,
                db_path: config_dir.join("bingtray.db"),
            })
        }

        #[cfg(target_arch = "wasm32")]
        {
            // WASM: No filesystem, return empty paths
            log::info!("WASM config - no filesystem access");

            Ok(Config {
                config_dir: PathBuf::new(),
                unprocessed_dir: PathBuf::new(),
                keepfavorite_dir: PathBuf::new(),
                cached_dir: PathBuf::new(),
                image_cached_dir: PathBuf::new(),
                db_path: PathBuf::new(),
            })
        }

        #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
        {
            // Desktop platforms (Linux, Windows, macOS)
            let proj_dirs = ProjectDirs::from("pe", "nikescar", "bingtray")
                .context("Failed to get project directories")?;

            let config_dir = proj_dirs.config_dir().to_path_buf();
            let cache_dir = proj_dirs.cache_dir().to_path_buf();

            let unprocessed_dir = cache_dir.join("unprocessed");
            let keepfavorite_dir = cache_dir.join("keepfavorite");
            let cached_dir = cache_dir.join("cached");
            let image_cached_dir = cache_dir.join("image_cache");

            // Create directories if they don't exist
            fs::create_dir_all(&config_dir)?;
            fs::create_dir_all(&unprocessed_dir)?;
            fs::create_dir_all(&keepfavorite_dir)?;
            fs::create_dir_all(&cached_dir)?;
            fs::create_dir_all(&image_cached_dir)?;

            Ok(Config {
                config_dir: config_dir.clone(),
                unprocessed_dir,
                keepfavorite_dir,
                cached_dir,
                image_cached_dir,
                db_path: config_dir.join("bingtray.db"),
            })
        }
    }
}

/// Bing image record from API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingImage {
    pub url: String,
    pub title: String,
    pub copyright: Option<String>,
    pub copyright_link: Option<String>,
}

/// Bing API response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BingResponse {
    pub images: Vec<BingImage>,
}

/// Historical image record from GitHub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalImage {
    pub fullstartdate: String,      // YYYYMMDDHHMM format
    pub url: String,
    pub copyright: String,
    pub copyrightlink: String,
    pub title: String,
}

/// Log level enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Info
    }
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub virustotal_apikey: String,
    #[serde(default)]
    pub hybridanalysis_apikey: String,
    #[serde(default)]
    pub virustotal_submit: bool,
    #[serde(default)]
    pub hybridanalysis_submit: bool,
    #[serde(default)]
    pub hybridanalysis_tag_ignorelist: String,
    #[serde(default)]
    pub show_logs: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_theme_mode")]
    pub theme_mode: String,
    #[serde(default = "default_display_size")]
    pub display_size: String,
    #[serde(default)]
    pub apkmirror_renderer: bool,
    #[serde(default)]
    pub apkmirror_email: String,
    #[serde(default)]
    pub apkmirror_name: String,
    #[serde(default)]
    pub apkmirror_auto_upload: bool,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub font_path: String,
    #[serde(default)]
    pub override_text_style: String,
    #[serde(default = "default_theme_name")]
    pub theme_name: String,
    #[serde(default)]
    pub unsafe_app_remove: bool,
    #[serde(default)]
    pub autoupdate: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            virustotal_apikey: String::new(),
            hybridanalysis_apikey: String::new(),
            virustotal_submit: false,
            hybridanalysis_submit: false,
            hybridanalysis_tag_ignorelist: String::new(),
            show_logs: false,
            log_level: default_log_level(),
            theme_mode: default_theme_mode(),
            display_size: default_display_size(),
            apkmirror_renderer: false,
            apkmirror_email: String::new(),
            apkmirror_name: String::new(),
            apkmirror_auto_upload: false,
            language: default_language(),
            font_path: String::new(),
            override_text_style: String::new(),
            theme_name: default_theme_name(),
            unsafe_app_remove: false,
            autoupdate: false,
        }
    }
}

fn default_language() -> String {
    "Auto".to_string()
}

fn default_log_level() -> String {
    "Error".to_string()
}

fn default_theme_mode() -> String {
    "Auto".to_string()
}

fn default_display_size() -> String {
    "Desktop (1024x768)".to_string()
}

fn default_theme_name() -> String {
    "default".to_string()
}
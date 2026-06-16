/// Utility functions migrated from calc_bingimage.rs

use crate::Config;
use anyhow::Result;

/// Clean and sanitize a filename to ensure filesystem compatibility
pub fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .chars()
        .take(100)
        .collect()
}

/// Desktop wallpaper setter (cross-platform)
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub struct DesktopWallpaperSetter;

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
impl crate::bingtray::WallpaperSetter for DesktopWallpaperSetter {
    fn set_wallpaper_from_bytes(&self, bytes: &[u8]) -> std::io::Result<bool> {
        log::info!(
            "DesktopWallpaperSetter: Setting wallpaper from {} bytes",
            bytes.len()
        );

        match crate::api_setwallpaper::set_wallpaper_from_bytes(bytes) {
            Ok(()) => {
                log::info!("DesktopWallpaperSetter: Wallpaper set successfully");
                Ok(true)
            }
            Err(e) => {
                log::error!("DesktopWallpaperSetter: Failed to set wallpaper: {}", e);
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to set wallpaper: {}", e),
                ))
            }
        }
    }
}

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
impl DesktopWallpaperSetter {
    pub fn new() -> Self {
        DesktopWallpaperSetter
    }
}

/// Cached main panel image data
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct CachedMainPanelImage {
    pub title: String,
    pub copyright: String,
    pub copyright_link: String,
    pub thumbnail_url: String,
    pub full_url: String,
    pub status: Option<String>,
}

/// Save main panel selection to cache
pub fn save_main_panel_selection(
    config: &Config,
    title: &str,
    copyright: &str,
    copyright_link: &str,
    thumbnail_url: &str,
    full_url: &str,
    status: Option<String>,
) -> Result<()> {
    let cache_file = config.config_dir.join("main_panel_selection.json");

    let cached = CachedMainPanelImage {
        title: title.to_string(),
        copyright: copyright.to_string(),
        copyright_link: copyright_link.to_string(),
        thumbnail_url: thumbnail_url.to_string(),
        full_url: full_url.to_string(),
        status,
    };

    #[cfg(feature = "serde")]
    {
        let json = serde_json::to_string_pretty(&cached)?;
        std::fs::write(&cache_file, json)?;
        log::debug!("Saved main panel selection: {}", title);
    }

    Ok(())
}

/// Load main panel selection from cache
pub fn load_main_panel_selection(config: &Config) -> Option<CachedMainPanelImage> {
    let cache_file = config.config_dir.join("main_panel_selection.json");

    if !cache_file.exists() {
        return None;
    }

    #[cfg(feature = "serde")]
    {
        match std::fs::read_to_string(&cache_file) {
            Ok(json) => match serde_json::from_str(&json) {
                Ok(cached) => {
                    log::debug!("Loaded main panel selection from cache");
                    Some(cached)
                }
                Err(e) => {
                    log::warn!("Failed to parse main panel cache: {}", e);
                    None
                }
            },
            Err(e) => {
                log::warn!("Failed to read main panel cache: {}", e);
                None
            }
        }
    }

    #[cfg(not(feature = "serde"))]
    None
}

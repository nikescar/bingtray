//! Tray business logic shared between backends

use anyhow::Result;

pub struct TrayLogic {
    conn: diesel::SqliteConnection,
}

impl TrayLogic {
    pub fn new() -> Result<Self> {
        use diesel::Connection;
        let db_path = crate::db::get_database_path()?;
        let mut conn = diesel::SqliteConnection::establish(&db_path.to_string_lossy())?;

        // Run migrations
        use diesel_migrations::MigrationHarness;
        conn.run_pending_migrations(crate::db::MIGRATIONS)
            .map_err(|e| anyhow::anyhow!("Migration failed: {}", e))?;

        Ok(Self { conn })
    }

    pub fn get_wallpaper_page_status(&mut self) -> String {
        match crate::db::operations::count_by_status(&mut self.conn, crate::db::ImageStatus::Unprocessed) {
            Ok(count) => format!("({} available)", count),
            Err(_) => String::new(),
        }
    }

    pub fn has_next_available(&mut self) -> bool {
        true
    }

    pub fn get_current_image_title(&mut self) -> String {
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

    pub fn can_keep(&mut self) -> bool {
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

    pub fn can_blacklist(&mut self) -> bool {
        use crate::viewmodel::commands::get_current_desktop_wallpaper_url_sync;
        get_current_desktop_wallpaper_url_sync(&mut self.conn).ok().flatten().is_some()
    }

    pub fn has_kept_wallpapers(&mut self) -> bool {
        crate::db::operations::count_by_status(&mut self.conn, crate::db::ImageStatus::KeepFavorite)
            .map(|count| count > 0)
            .unwrap_or(false)
    }

    pub fn open_cache_directory(&self) -> Result<()> {
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

    pub fn set_next_market_wallpaper(&mut self) -> Result<bool> {
        use crate::viewmodel::commands::download_and_set_next_wallpaper_sync;

        match download_and_set_next_wallpaper_sync(&mut self.conn) {
            Ok(_result) => Ok(true),
            Err(e) => {
                log::error!("Failed to set next wallpaper: {}", e);
                Err(e)
            }
        }
    }

    pub fn keep_current_image(&mut self) -> Result<()> {
        use crate::viewmodel::commands::keep_current_wallpaper_sync;

        if let Some(_title) = keep_current_wallpaper_sync(&mut self.conn)? {
            log::info!("Kept current image");
            Ok(())
        } else {
            anyhow::bail!("No current wallpaper to keep")
        }
    }

    pub fn blacklist_current_image(&mut self) -> Result<()> {
        use crate::viewmodel::commands::blacklist_current_wallpaper_sync;

        if let Some(_title) = blacklist_current_wallpaper_sync(&mut self.conn)? {
            log::info!("Blacklisted current image");
            Ok(())
        } else {
            anyhow::bail!("No current wallpaper to blacklist")
        }
    }

    pub fn set_kept_wallpaper(&mut self) -> Result<bool> {
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

impl Clone for TrayLogic {
    fn clone(&self) -> Self {
        Self::new().expect("Failed to clone TrayLogic")
    }
}

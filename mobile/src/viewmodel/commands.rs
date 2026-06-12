use diesel::prelude::*;
use anyhow::Result;
use crate::db::ImageStatus;

/// Download images for a market code (stub for now)
pub fn download_images_sync(_conn: &mut SqliteConnection, _market_code: &str) -> Result<usize> {
    // TODO: Implement actual download logic using api_bingimage.rs
    // For now, return 0 to make compilation work
    log::info!("download_images_sync called (stub)");
    Ok(0)
}

/// Set wallpaper from URL (stub for now)
pub fn set_wallpaper_sync(_conn: &mut SqliteConnection, url: &str) -> Result<bool> {
    // TODO: Implement actual wallpaper setting using api_setwallpaper.rs
    log::info!("set_wallpaper_sync called for: {}", url);
    Ok(true)
}

/// Toggle favorite status for an image
pub fn toggle_favorite_sync(conn: &mut SqliteConnection, url: &str) -> Result<()> {
    use crate::db::operations;

    // Get current image
    let img = operations::get_image(conn, url)?;

    if let Some(image) = img {
        let current_status = crate::db::ImageStatus::from_str(&image.status)
            .unwrap_or(ImageStatus::Unprocessed);

        let new_status = match current_status {
            ImageStatus::KeepFavorite => ImageStatus::Unprocessed,
            _ => ImageStatus::KeepFavorite,
        };

        operations::update_image_status(conn, url, new_status)?;
    }

    Ok(())
}

/// Blacklist an image
pub fn blacklist_image_sync(conn: &mut SqliteConnection, url: &str) -> Result<()> {
    use crate::db::operations;
    operations::update_image_status(conn, url, ImageStatus::Blacklisted)?;
    Ok(())
}

use bingtray::datafusion_bingimage::{BingImageDb, ImageStatus};
use bingtray::Config;

fn main() -> anyhow::Result<()> {
    // Get config
    let config = Config::new()?;

    println!("Opening database at: {:?}", config.data_dir);

    // Open the database
    let db = BingImageDb::new(config.data_dir)?;

    println!("\nQuerying latest image from each status...\n");

    // Try each status to find records
    for status in [ImageStatus::Unprocessed, ImageStatus::KeepFavorite, ImageStatus::Blacklisted] {
        let images = db.get_images_by_status(status.clone())?;
        if !images.is_empty() {
            println!("=== Latest {:?} image ===", status);
            let latest = &images[0]; // Already sorted by fetched_at DESC
            println!("URL: {}", latest.url);
            println!("Title: {}", latest.title);
            println!("Copyright: {}", latest.copyright.as_deref().unwrap_or("N/A"));
            println!("Market Code: {}", latest.market_code);

            // Convert timestamp to readable date
            use std::time::{UNIX_EPOCH, Duration};
            let datetime = UNIX_EPOCH + Duration::from_secs(latest.fetched_at as u64);
            println!("Fetched At: {:?}", datetime);
            println!();
        }
    }

    // Also show total counts
    println!("=== Database Stats ===");
    println!("Unprocessed: {}", db.count_by_status(ImageStatus::Unprocessed)?);
    println!("Keep Favorite: {}", db.count_by_status(ImageStatus::KeepFavorite)?);
    println!("Blacklisted: {}", db.count_by_status(ImageStatus::Blacklisted)?);

    Ok(())
}

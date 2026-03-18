#!/usr/bin/env rust-script
//! ```cargo
//! [dependencies]
//! bingtray = { path = "./mobile" }
//! anyhow = "1.0"
//! ```

use bingtray::datafusion_bingimage::BingImageDb;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    // Get the database path
    let config_dir = directories::ProjectDirs::from("pe", "nikescar", "bingtray")
        .expect("Failed to get project directories")
        .config_dir()
        .to_path_buf();

    let data_dir = config_dir.join("datafusion_data");

    println!("Opening database at: {:?}", data_dir);

    // Open the database
    let db = BingImageDb::new(data_dir)?;

    // Get all images (we'll sort by fetched_at to get latest)
    println!("\nQuerying latest image...\n");

    // Get unprocessed images (or all statuses)
    use bingtray::datafusion_bingimage::ImageStatus;

    // Try each status to find any records
    for status in [ImageStatus::Unprocessed, ImageStatus::KeepFavorite, ImageStatus::Blacklisted] {
        let images = db.get_images_by_status(status.clone())?;
        if !images.is_empty() {
            let latest = &images[0]; // Already sorted by fetched_at DESC
            println!("Latest {:?} image:", status);
            println!("  URL: {}", latest.url);
            println!("  Title: {}", latest.title);
            println!("  Copyright: {:?}", latest.copyright);
            println!("  Market Code: {}", latest.market_code);
            println!("  Fetched At: {} (Unix timestamp)", latest.fetched_at);
            println!("  Status: {:?}", latest.status);
            return Ok(());
        }
    }

    println!("No images found in database");
    Ok(())
}

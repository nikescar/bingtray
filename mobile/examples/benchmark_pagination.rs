use bingtray::datafusion_bingimage::{BingImageDb, BingImageRecord, ImageStatus};
use std::time::Instant;
use tempfile::TempDir;

fn main() -> anyhow::Result<()> {
    println!("=== Pagination Performance Benchmark ===\n");

    // Create test database with historical images
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("bench.db");
    let db = BingImageDb::new(db_path)?;

    // Insert 1000 "historical" images
    println!("Setting up test data: 1000 historical images...");
    let setup_start = Instant::now();

    let records: Vec<BingImageRecord> = (0..1000).map(|i| BingImageRecord {
        url: format!("https://example.com/historical_{}.jpg", i),
        title: format!("Historical Image {}", i),
        copyright: Some("Test Copyright".to_string()),
        copyright_link: Some("https://example.com".to_string()),
        market_code: "historical".to_string(),
        fetched_at: 1000000000 + i as i64,
        status: ImageStatus::Unprocessed,
    }).collect();

    db.batch_upsert_images(&records)?;
    db.checkpoint()?; // Flush to Parquet

    println!("Setup complete in {:?}\n", setup_start.elapsed());

    // Benchmark: Count ALL historical images (what the old code did)
    println!("=== OLD METHOD (wasteful) ===");
    println!("Getting all historical images just to count...");
    let start = Instant::now();
    let all_images = db.get_images_by_market_code("historical")?;
    let count = all_images.len();
    let old_time = start.elapsed();
    println!("  Loaded {} records in {:?}", count, old_time);
    println!("  Memory used: ~{} KB\n", count * std::mem::size_of::<BingImageRecord>() / 1024);

    // Benchmark: Efficient count
    println!("=== NEW METHOD (efficient) ===");
    println!("Using COUNT(*) query...");
    let start = Instant::now();
    let count = db.count_by_market_code("historical")?;
    let new_time = start.elapsed();
    println!("  Counted {} records in {:?}", count, new_time);
    println!("  Memory used: ~8 bytes\n");

    // Benchmark: Paginated query (what we actually need)
    println!("=== PAGINATION (what carousel needs) ===");
    let page = 5;
    let limit = 3;
    let offset = page * limit;

    println!("Loading page {} (limit={}, offset={})...", page, limit, offset);
    let start = Instant::now();
    let page_results = db.get_images_by_market_code_paginated("historical", limit, offset)?;
    let page_time = start.elapsed();
    println!("  Loaded {} records in {:?}\n", page_results.len(), page_time);

    // Summary
    println!("=== PERFORMANCE SUMMARY ===");
    println!("Old method (load all):     {:?}", old_time);
    println!("New method (count):        {:?}  ({:.0}x faster)", new_time, old_time.as_secs_f64() / new_time.as_secs_f64());
    println!("Pagination (needed):       {:?}", page_time);
    println!("\nOld carousel next page time: ~{:?} (count + pagination)", old_time + page_time);
    println!("New carousel next page time: ~{:?}", page_time);
    println!("\nSpeedup: {:.1}x faster! 🚀", (old_time + page_time).as_secs_f64() / page_time.as_secs_f64());

    Ok(())
}

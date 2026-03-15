use bingtray::datafusion_bingimage::{BingImageDb, BingImageRecord, ImageStatus};
use std::time::Instant;
use tempfile::TempDir;

fn main() -> anyhow::Result<()> {
    println!("=== DataFusion Upsert Performance Benchmark ===\n");

    // Create test database
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("bench.db");
    let db = BingImageDb::new(db_path)?;

    let (cache_size, threshold) = db.cache_stats();
    println!("Cache threshold: {} records", threshold);
    println!("Initial cache size: {}\n", cache_size);

    // Benchmark: Insert 1000 records
    println!("Benchmarking 1000 sequential upserts...");
    let start = Instant::now();

    for i in 0..1000 {
        let record = BingImageRecord {
            url: format!("https://example.com/image{}.jpg", i),
            title: format!("Test Image {}", i),
            copyright: Some("Test Copyright".to_string()),
            copyright_link: Some("https://example.com".to_string()),
            market_code: "en-US".to_string(),
            fetched_at: 1234567890 + i as i64,
            status: ImageStatus::Unprocessed,
        };

        db.upsert_image(&record)?;

        // Show cache stats every 100 inserts
        if (i + 1) % 100 == 0 {
            let (cache_size, _) = db.cache_stats();
            println!("  {} records inserted, cache size: {}", i + 1, cache_size);
        }
    }

    let elapsed = start.elapsed();
    println!("\n✅ Completed 1000 upserts in {:?}", elapsed);
    println!("   Average: {:.2} µs per upsert", elapsed.as_micros() as f64 / 1000.0);
    println!("   Throughput: {:.0} upserts/sec", 1000.0 / elapsed.as_secs_f64());

    // Final cache stats
    let (cache_size, _) = db.cache_stats();
    println!("\nFinal cache size: {} records", cache_size);

    // Benchmark: Query performance
    println!("\nBenchmarking query (with cache merge)...");
    let start = Instant::now();
    let records = db.get_images_by_status(ImageStatus::Unprocessed)?;
    let query_time = start.elapsed();
    println!("  Found {} records in {:?}", records.len(), query_time);

    // Force flush and measure
    println!("\nForcing cache flush...");
    let start = Instant::now();
    db.checkpoint()?;
    let flush_time = start.elapsed();
    println!("  Flushed {} records in {:?}", cache_size, flush_time);

    // Query again from Parquet
    println!("\nQuerying from Parquet (no cache)...");
    let start = Instant::now();
    let records = db.get_images_by_status(ImageStatus::Unprocessed)?;
    let parquet_query_time = start.elapsed();
    println!("  Found {} records in {:?}", records.len(), parquet_query_time);

    println!("\n=== Performance Summary ===");
    println!("Upsert (cached):  {:.2} µs", elapsed.as_micros() as f64 / 1000.0);
    println!("Query (cached):   {:?}", query_time);
    println!("Flush to disk:    {:?}", flush_time);
    println!("Query (Parquet):  {:?}", parquet_query_time);

    Ok(())
}

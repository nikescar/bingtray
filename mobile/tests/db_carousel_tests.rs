use bingtray::db::{establish_connection, operations, models::NewBingImage, ImageStatus};
use diesel::prelude::*;
use std::path::PathBuf;

#[test]
fn test_crop_coords_json_storage() {
    // Setup: In-memory database
    let db_path = PathBuf::from(":memory:");
    let mut conn = establish_connection(&db_path);

    // Run migrations
    use diesel_migrations::MigrationHarness;
    conn.run_pending_migrations(bingtray::db::MIGRATIONS)
        .expect("migrations");

    // Insert test image with crop coords
    let test_image = NewBingImage {
        url: "https://www.bing.com/test.jpg",
        title: "Test Image",
        copyright: Some("© Test"),
        copyright_link: Some("https://test.com"),
        market_code: "en-US",
        status: "unprocessed",
        fetched_at: 1234567890,
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    operations::upsert_image(&mut conn, &test_image).expect("insert");

    // Update with crop coords
    let crop_json = r#"{"x":0.1,"y":0.2,"width":0.6,"height":0.8}"#;
    operations::update_crop_coords(&mut conn, test_image.url, Some(crop_json))
        .expect("update crop");

    // Retrieve and verify
    let retrieved = operations::get_crop_coords(&mut conn, test_image.url)
        .expect("get crop");

    assert_eq!(retrieved, Some(crop_json.to_string()));
}

#[test]
fn test_crop_coords_null_handling() {
    let db_path = PathBuf::from(":memory:");
    let mut conn = establish_connection(&db_path);

    use diesel_migrations::MigrationHarness;
    conn.run_pending_migrations(bingtray::db::MIGRATIONS).expect("migrations");

    // Insert image without crop_coords
    let test_image = NewBingImage {
        url: "https://www.bing.com/test2.jpg",
        title: "Test Image 2",
        copyright: None,
        copyright_link: None,
        market_code: "en-US",
        status: "unprocessed",
        fetched_at: 1234567890,
        created_at: 1234567890,
        updated_at: 1234567890,
    };

    operations::upsert_image(&mut conn, &test_image).expect("insert");

    // Query crop_coords (should be None)
    let crop = operations::get_crop_coords(&mut conn, test_image.url).expect("get");
    assert_eq!(crop, None);
}

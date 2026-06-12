use bingtray::db::{self, models::*, ImageStatus};
use diesel::prelude::*;
use tempfile::TempDir;

fn setup_test_db() -> (SqliteConnection, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let conn = db::establish_connection(&db_path);
    (conn, temp_dir)
}

fn create_test_image(url: &str, status: ImageStatus) -> NewBingImage {
    NewBingImage {
        url,
        title: "Test Image",
        copyright: Some("Test Copyright"),
        copyright_link: Some("https://example.com"),
        market_code: "en-US",
        fetched_at: 1234567890,
        status: status.as_str(),
        created_at: 1234567890,
        updated_at: 1234567890,
    }
}

#[test]
fn test_upsert_creates_new_image() {
    let (mut conn, _dir) = setup_test_db();

    let new_img = create_test_image("https://example.com/img1.jpg", ImageStatus::Unprocessed);
    let result = db::operations::upsert_image(&mut conn, &new_img).unwrap();

    assert_eq!(result.url, "https://example.com/img1.jpg");
    assert_eq!(result.title, "Test Image");
    assert_eq!(result.status, "unprocessed");
}

#[test]
fn test_upsert_updates_existing_image() {
    let (mut conn, _dir) = setup_test_db();

    let url = "https://example.com/img2.jpg";
    let new_img = create_test_image(url, ImageStatus::Unprocessed);
    db::operations::upsert_image(&mut conn, &new_img).unwrap();

    // Update with different title and status
    let updated_img = NewBingImage {
        title: "Updated Title",
        status: ImageStatus::KeepFavorite.as_str(),
        ..new_img
    };
    db::operations::upsert_image(&mut conn, &updated_img).unwrap();

    let retrieved = db::operations::get_image(&mut conn, url).unwrap().unwrap();
    assert_eq!(retrieved.title, "Updated Title");
    assert_eq!(retrieved.status, "keepfavorite");
}

#[test]
fn test_get_images_by_status() {
    let (mut conn, _dir) = setup_test_db();

    // Insert images with different statuses
    let img1 = create_test_image("https://example.com/u1.jpg", ImageStatus::Unprocessed);
    let img2 = create_test_image("https://example.com/u2.jpg", ImageStatus::Unprocessed);
    let img3 = create_test_image("https://example.com/f1.jpg", ImageStatus::KeepFavorite);
    let img4 = create_test_image("https://example.com/b1.jpg", ImageStatus::Blacklisted);

    db::operations::upsert_image(&mut conn, &img1).unwrap();
    db::operations::upsert_image(&mut conn, &img2).unwrap();
    db::operations::upsert_image(&mut conn, &img3).unwrap();
    db::operations::upsert_image(&mut conn, &img4).unwrap();

    let unprocessed = db::operations::get_images_by_status(&mut conn, ImageStatus::Unprocessed).unwrap();
    assert_eq!(unprocessed.len(), 2);

    let favorites = db::operations::get_images_by_status(&mut conn, ImageStatus::KeepFavorite).unwrap();
    assert_eq!(favorites.len(), 1);

    let blacklisted = db::operations::get_images_by_status(&mut conn, ImageStatus::Blacklisted).unwrap();
    assert_eq!(blacklisted.len(), 1);
}

#[test]
fn test_get_images_by_market_code_pagination() {
    let (mut conn, _dir) = setup_test_db();

    // Insert 5 images with same market code
    for i in 0..5 {
        let url = format!("https://example.com/img{}.jpg", i);
        let img = create_test_image(&url, ImageStatus::Unprocessed);
        db::operations::upsert_image(&mut conn, &img).unwrap();
    }

    let page1 = db::operations::get_images_by_market_code(&mut conn, "en-US", 2, 0).unwrap();
    assert_eq!(page1.len(), 2);

    let page2 = db::operations::get_images_by_market_code(&mut conn, "en-US", 2, 2).unwrap();
    assert_eq!(page2.len(), 2);

    let page3 = db::operations::get_images_by_market_code(&mut conn, "en-US", 2, 4).unwrap();
    assert_eq!(page3.len(), 1);
}

#[test]
fn test_sql_injection_protection() {
    let (mut conn, _dir) = setup_test_db();

    let malicious_url = "'; DROP TABLE bing_images; --";
    let img = create_test_image(malicious_url, ImageStatus::Unprocessed);

    db::operations::upsert_image(&mut conn, &img).unwrap();

    // Should retrieve safely without executing SQL
    let retrieved = db::operations::get_image(&mut conn, malicious_url).unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().url, malicious_url);
}

#[test]
fn test_config_operations() {
    let (mut conn, _dir) = setup_test_db();

    // Set config
    db::operations::set_config(&mut conn, "test_key", "test_value").unwrap();

    // Get config
    let value = db::operations::get_config(&mut conn, "test_key").unwrap();
    assert_eq!(value, Some("test_value".to_string()));

    // Update config
    db::operations::set_config(&mut conn, "test_key", "updated_value").unwrap();
    let value = db::operations::get_config(&mut conn, "test_key").unwrap();
    assert_eq!(value, Some("updated_value".to_string()));

    // Non-existent key
    let value = db::operations::get_config(&mut conn, "non_existent").unwrap();
    assert_eq!(value, None);
}

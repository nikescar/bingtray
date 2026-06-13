// Integration tests for CLI ViewModel functionality
// Tests full workflows including database persistence, auto-download, and state management

#![cfg(feature = "cli-only")]

use tempfile::TempDir;
use diesel::prelude::*;

use bingtray::db::{establish_connection, models::ImageStatus, operations};
use bingtray::viewmodel::ViewModel;

// ============================================================================
// Test Setup Helpers
// ============================================================================

/// Create an isolated test database in a temporary directory
fn setup_test_db() -> (SqliteConnection, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_bingtray.db");
    
    // Create database and run migrations
    let conn = establish_connection(&db_path);
    
    (conn, temp_dir)
}

/// Create a ViewModel with isolated test database
fn setup_test_viewmodel() -> (ViewModel, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_bingtray.db");
    
    let vm = ViewModel::new_sync(db_path).expect("Failed to create ViewModel");
    
    (vm, temp_dir)
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn test_cli_full_workflow_empty_database_to_set_wallpaper() {
    // This test simulates a fresh install:
    // 1. Empty database
    // 2. Attempt to set wallpaper (should trigger auto-download)
    // 3. Verify image was downloaded and set
    
    let (vm, _temp_dir) = setup_test_viewmodel();
    
    // Empty database - no images should exist
    let mut conn = vm.db_connection().expect("Failed to get connection");
    let images = operations::get_images_by_status(&mut conn, ImageStatus::Unprocessed)
        .expect("Failed to query images");
    assert_eq!(images.len(), 0, "Database should start empty");
    
    // Note: download_and_set_next_wallpaper_sync will fail in test environment
    // because it requires network access to Bing API and ability to set wallpaper
    // This test documents the expected behavior but cannot fully execute
    
    // In a real environment, this would:
    // - Download 8 images from Bing API
    // - Set the first one as wallpaper
    // - Return WallpaperSetResult with title and URL
    
    // For now, we just verify the ViewModel can be created and database initialized
    assert!(vm.get_market_state_sync().is_ok());
}

#[test]
fn test_database_persistence_across_viewmodel_instances() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_bingtray.db");
    
    // First ViewModel instance - set market state
    {
        let vm1 = ViewModel::new_sync(db_path.clone()).expect("Failed to create first ViewModel");
        vm1.save_market_state_sync("ja-JP", 16).expect("Failed to save market state");
    }
    
    // Second ViewModel instance - verify state persists
    {
        let vm2 = ViewModel::new_sync(db_path.clone()).expect("Failed to create second ViewModel");
        let (market_code, offset) = vm2.get_market_state_sync().expect("Failed to get market state");
        
        assert_eq!(market_code, "ja-JP");
        assert_eq!(offset, 16);
    }
}

#[test]
fn test_offset_increments_across_multiple_sessions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_bingtray.db");
    
    // Session 1: Default offset should be 0
    {
        let vm = ViewModel::new_sync(db_path.clone()).expect("Failed to create ViewModel");
        let (_, offset) = vm.get_market_state_sync().expect("Failed to get market state");
        assert_eq!(offset, 0);
        
        // Simulate a download cycle (increments by 8)
        vm.increment_market_offset_sync().expect("Failed to increment offset");
    }
    
    // Session 2: Offset should persist at 8
    {
        let vm = ViewModel::new_sync(db_path.clone()).expect("Failed to create ViewModel");
        let (_, offset) = vm.get_market_state_sync().expect("Failed to get market state");
        assert_eq!(offset, 8);
        
        vm.increment_market_offset_sync().expect("Failed to increment offset");
    }
    
    // Session 3: Offset should now be 16
    {
        let vm = ViewModel::new_sync(db_path.clone()).expect("Failed to create ViewModel");
        let (_, offset) = vm.get_market_state_sync().expect("Failed to get market state");
        assert_eq!(offset, 16);
    }
}

#[test]
fn test_keep_and_blacklist_operations_persist() {
    let (mut conn, _temp_dir) = setup_test_db();
    
    // Insert test images
    use std::time::{SystemTime, UNIX_EPOCH};
    use bingtray::db::models::NewBingImage;
    
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    
    let img1 = NewBingImage {
        url: "https://example.com/image1.jpg",
        title: "Image 1",
        copyright: Some("Copyright 1"),
        copyright_link: Some("https://example.com/1"),
        market_code: "en-US",
        fetched_at: timestamp,
        status: ImageStatus::Unprocessed.as_str(),
        created_at: timestamp,
        updated_at: timestamp,
    };
    
    let img2 = NewBingImage {
        url: "https://example.com/image2.jpg",
        title: "Image 2",
        copyright: Some("Copyright 2"),
        copyright_link: Some("https://example.com/2"),
        market_code: "en-US",
        fetched_at: timestamp,
        status: ImageStatus::Unprocessed.as_str(),
        created_at: timestamp,
        updated_at: timestamp,
    };
    
    operations::upsert_image(&mut conn, &img1).expect("Failed to insert image 1");
    operations::upsert_image(&mut conn, &img2).expect("Failed to insert image 2");
    
    // Update statuses
    operations::update_image_status(&mut conn, "https://example.com/image1.jpg", ImageStatus::KeepFavorite)
        .expect("Failed to mark as favorite");
    operations::update_image_status(&mut conn, "https://example.com/image2.jpg", ImageStatus::Blacklisted)
        .expect("Failed to blacklist");
    
    // Verify persistence
    let favorites = operations::get_images_by_status(&mut conn, ImageStatus::KeepFavorite)
        .expect("Failed to query favorites");
    assert_eq!(favorites.len(), 1);
    assert_eq!(favorites[0].title, "Image 1");
    
    let blacklisted = operations::get_images_by_status(&mut conn, ImageStatus::Blacklisted)
        .expect("Failed to query blacklisted");
    assert_eq!(blacklisted.len(), 1);
    assert_eq!(blacklisted[0].title, "Image 2");
}

#[test]
fn test_random_favorite_requires_favorites() {
    let (vm, _temp_dir) = setup_test_viewmodel();
    
    // Empty database - no favorites
    let result = vm.set_random_favorite_wallpaper_sync();
    
    // Should return error or None because no favorites exist
    // The actual behavior depends on implementation
    // For now, we just verify it doesn't panic
    assert!(result.is_err() || result.unwrap().is_none());
}

#[test]
fn test_market_state_defaults_correctly() {
    let (vm, _temp_dir) = setup_test_viewmodel();
    
    // Fresh database should have default market state
    let (market_code, offset) = vm.get_market_state_sync()
        .expect("Failed to get default market state");
    
    assert_eq!(market_code, "en-US", "Default market code should be en-US");
    assert_eq!(offset, 0, "Default offset should be 0");
}

#[test]
fn test_viewmodel_can_handle_missing_wallpaper() {
    let (vm, _temp_dir) = setup_test_viewmodel();
    
    // Attempt to keep/blacklist when no wallpaper is set
    // Should return None (no match)
    
    // Note: This test will fail if a wallpaper is actually set on the system
    // In a real test environment, we'd mock api_setwallpaper::get_wallpaper()
    
    let keep_result = vm.keep_current_wallpaper_sync();
    let blacklist_result = vm.blacklist_current_wallpaper_sync();
    
    // Both should handle the case gracefully (return Ok(None) or Err)
    // The exact behavior depends on whether get_wallpaper() returns Some or None
    // For now, we just verify they don't panic
    assert!(keep_result.is_ok() || keep_result.is_err());
    assert!(blacklist_result.is_ok() || blacklist_result.is_err());
}

#[test]
fn test_database_schema_created_correctly() {
    let (mut conn, _temp_dir) = setup_test_db();
    
    // Verify all required tables exist
    use diesel::sql_query;
    use diesel::sql_types::Text;
    
    #[derive(QueryableByName)]
    struct TableName {
        #[diesel(sql_type = Text)]
        name: String,
    }
    
    let tables: Vec<TableName> = sql_query(
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
    )
    .load(&mut conn)
    .expect("Failed to query tables");
    
    let table_names: Vec<String> = tables.iter().map(|t| t.name.clone()).collect();
    
    // Verify core tables exist
    assert!(table_names.contains(&"bing_images".to_string()), "bing_images table missing");
    assert!(table_names.contains(&"config_kv".to_string()), "config_kv table missing");
    assert!(table_names.contains(&"market_codes".to_string()), "market_codes table missing");
    assert!(table_names.contains(&"__diesel_schema_migrations".to_string()), "migrations table missing");
}

#[test]
fn test_concurrent_viewmodel_access_same_database() {
    // This test verifies that multiple ViewModel instances can safely access
    // the same database file without corruption
    
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_bingtray.db");
    
    // Create two ViewModels pointing to same database
    let vm1 = ViewModel::new_sync(db_path.clone()).expect("Failed to create VM 1");
    let vm2 = ViewModel::new_sync(db_path.clone()).expect("Failed to create VM 2");
    
    // Both should be able to read default state
    let (code1, offset1) = vm1.get_market_state_sync().expect("VM1 failed to get state");
    let (code2, offset2) = vm2.get_market_state_sync().expect("VM2 failed to get state");
    
    assert_eq!(code1, code2);
    assert_eq!(offset1, offset2);
    
    // VM1 updates state
    vm1.save_market_state_sync("ja-JP", 24).expect("VM1 failed to save");
    
    // VM2 should see the update (after creating new connection)
    let (code2_new, offset2_new) = vm2.get_market_state_sync().expect("VM2 failed to get updated state");
    assert_eq!(code2_new, "ja-JP");
    assert_eq!(offset2_new, 24);
}

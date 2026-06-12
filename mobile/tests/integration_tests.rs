use tempfile::TempDir;

#[test]
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32"), not(feature = "cli-only")))]
fn test_desktop_viewmodel_initialization() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Test async ViewModel creation
    let vm = bingtray::viewmodel::ViewModel::new_async(db_path).unwrap();

    // Send test command
    vm.send_command(bingtray::viewmodel::ViewModelCommand::RefreshDatabase).unwrap();

    // Give background thread time to process
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Cleanup
    vm.send_command(bingtray::viewmodel::ViewModelCommand::Shutdown).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
}

#[test]
#[cfg(feature = "cli-only")]
fn test_cli_sync_viewmodel() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let vm = bingtray::viewmodel::ViewModel::new_sync(db_path).unwrap();

    // Test sync operations
    let images = vm.get_images_by_status_sync(bingtray::db::ImageStatus::Unprocessed).unwrap();
    assert!(images.is_empty());

    // Test download stub
    let count = vm.download_images_sync("en-US").unwrap();
    assert_eq!(count, 0);  // Stub returns 0
}

#[test]
fn test_database_persists_across_connections() {
    use bingtray::db::{self, operations, models::*, ImageStatus};

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Insert data
    {
        let mut conn = db::establish_connection(&db_path);
        let new_img = NewBingImage {
            url: "https://example.com/persist.jpg",
            title: "Persist Test",
            copyright: None,
            copyright_link: None,
            market_code: "en-US",
            fetched_at: 1234567890,
            status: ImageStatus::Unprocessed.as_str(),
            created_at: 1234567890,
            updated_at: 1234567890,
        };
        operations::upsert_image(&mut conn, &new_img).unwrap();
    }

    // Verify data persists
    {
        let mut conn = db::establish_connection(&db_path);
        let img = operations::get_image(&mut conn, "https://example.com/persist.jpg").unwrap();
        assert!(img.is_some());
        assert_eq!(img.unwrap().title, "Persist Test");
    }
}

#[cfg(not(feature = "cli-only"))]
use bingtray::viewmodel::{ViewModel, ViewModelCommand, ViewModelEvent};
#[cfg(feature = "cli-only")]
use bingtray::viewmodel::ViewModel;
use bingtray::db::ImageStatus;
use tempfile::TempDir;
use std::thread;
use std::time::Duration;

#[test]
#[cfg(not(feature = "cli-only"))]
fn test_viewmodel_async_creation() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let vm = ViewModel::new_async(db_path).unwrap();

    // Send shutdown command
    vm.send_command(ViewModelCommand::Shutdown).unwrap();

    // Give background thread time to shut down
    thread::sleep(Duration::from_millis(100));
}

#[test]
#[cfg(not(feature = "cli-only"))]
fn test_viewmodel_command_response() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let vm = ViewModel::new_async(db_path).unwrap();

    // Send command to get images
    vm.send_command(ViewModelCommand::GetImagesByStatus {
        status: ImageStatus::Unprocessed,
    }).unwrap();

    // Wait for background thread to process
    thread::sleep(Duration::from_millis(100));

    // Poll for events
    let events = vm.poll_events();
    assert!(events.iter().any(|e| matches!(e, ViewModelEvent::ImagesLoaded { .. })));

    // Cleanup
    vm.send_command(ViewModelCommand::Shutdown).unwrap();
    thread::sleep(Duration::from_millis(100));
}

#[test]
#[cfg(feature = "cli-only")]
fn test_viewmodel_sync_operations() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let vm = ViewModel::new_sync(db_path).unwrap();

    // Test synchronous get
    let images = vm.get_images_by_status_sync(ImageStatus::Unprocessed).unwrap();
    assert!(images.is_empty());  // Empty database
}

#[test]
#[cfg(not(feature = "cli-only"))]
fn test_unmark_image_command() {
    use bingtray::db::{establish_connection, operations, models::NewBingImage};

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Insert a test image directly to database BEFORE creating ViewModel
    {
        let mut conn = establish_connection(&db_path);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let test_image = NewBingImage {
            url: "https://bing.com/test_image.jpg",
            title: "Test Image",
            copyright: Some("Test Copyright"),
            copyright_link: Some("https://bing.com/link"),
            status: "blacklisted",  // Start as blacklisted
            market_code: "en-US",
            fetched_at: now,
            created_at: now,
            updated_at: now,
        };
        operations::upsert_image(&mut conn, &test_image).unwrap();
        // Explicitly drop connection
        drop(conn);
    }

    // Small delay to ensure WAL file is released
    thread::sleep(Duration::from_millis(100));

    // Now create ViewModel
    let vm = ViewModel::new_async(db_path.clone()).unwrap();

    // Send UnmarkImage command to set it back to unprocessed
    vm.send_command(ViewModelCommand::UnmarkImage {
        url: "https://bing.com/test_image.jpg".to_string(),
    }).unwrap();

    // Wait for processing
    thread::sleep(Duration::from_millis(200));

    // Poll for status update event
    let events = vm.poll_events();
    let status_updated = events.iter().any(|e| {
        matches!(e, ViewModelEvent::StatusUpdated { url, status }
            if url == "https://bing.com/test_image.jpg" && *status == ImageStatus::Unprocessed)
    });
    assert!(status_updated, "Expected StatusUpdated event for unmark operation");

    // Verify in database
    {
        let mut conn = establish_connection(&db_path);
        let image = operations::get_image(&mut conn, "https://bing.com/test_image.jpg")
            .unwrap()
            .expect("Image should exist");
        assert_eq!(image.status, "unprocessed", "Image status should be unprocessed after unmark");
    }

    // Cleanup
    vm.send_command(ViewModelCommand::Shutdown).unwrap();
    thread::sleep(Duration::from_millis(100));
}

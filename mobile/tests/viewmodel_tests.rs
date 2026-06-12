#[cfg(not(feature = "cli-only"))]
use bingtray::viewmodel::{ViewModel, ViewModelCommand, ViewModelEvent};
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

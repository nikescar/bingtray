use std::sync::mpsc::{Receiver, Sender};
use std::path::PathBuf;
use super::{ViewModelCommand, ViewModelEvent};

/// Background thread message loop (GUI/Android only)
pub fn run_background_loop(
    db_path: PathBuf,
    cmd_rx: Receiver<ViewModelCommand>,
    evt_tx: Sender<ViewModelEvent>,
) {
    log::info!("ViewModel background thread started");

    // Create Asupersync runtime
    let runtime = match asupersync::runtime::RuntimeBuilder::current_thread().build() {
        Ok(rt) => rt,
        Err(e) => {
            log::error!("Failed to create Asupersync runtime: {}", e);
            evt_tx.send(ViewModelEvent::Error {
                message: format!("Runtime error: {}", e)
            }).ok();
            return;
        }
    };

    let mut conn = crate::db::establish_connection(&db_path);

    // Message loop
    for cmd in cmd_rx {
        handle_command(&runtime, &mut conn, &evt_tx, cmd);
    }

    log::info!("ViewModel background thread stopped");
}

fn handle_command(
    _runtime: &asupersync::runtime::Runtime,
    conn: &mut diesel::SqliteConnection,
    evt_tx: &Sender<ViewModelEvent>,
    cmd: ViewModelCommand,
) {
    use ViewModelCommand::*;
    use crate::db::operations;

    match cmd {
        GetImagesByStatus { status } => {
            match operations::get_images_by_status(conn, status) {
                Ok(images) => {
                    evt_tx.send(ViewModelEvent::ImagesLoaded { images }).ok();
                }
                Err(e) => {
                    evt_tx.send(ViewModelEvent::Error {
                        message: format!("Failed to get images: {}", e)
                    }).ok();
                }
            }
        }

        GetImagesByMarket { market_code, page } => {
            let limit = 20;
            let offset = (page * limit) as i64;
            match operations::get_images_by_market_code(conn, &market_code, limit as i64, offset) {
                Ok(images) => {
                    evt_tx.send(ViewModelEvent::ImagesLoaded { images }).ok();
                }
                Err(e) => {
                    evt_tx.send(ViewModelEvent::Error {
                        message: format!("Failed to get images: {}", e)
                    }).ok();
                }
            }
        }

        ToggleFavorite { url } => {
            match super::commands::toggle_favorite_sync(conn, &url) {
                Ok(_) => {
                    evt_tx.send(ViewModelEvent::StatusUpdated {
                        url,
                        status: crate::db::ImageStatus::KeepFavorite
                    }).ok();
                }
                Err(e) => {
                    evt_tx.send(ViewModelEvent::Error {
                        message: format!("Failed to toggle favorite: {}", e)
                    }).ok();
                }
            }
        }

        BlacklistImage { url } => {
            match super::commands::blacklist_image_sync(conn, &url) {
                Ok(_) => {
                    evt_tx.send(ViewModelEvent::StatusUpdated {
                        url,
                        status: crate::db::ImageStatus::Blacklisted
                    }).ok();
                }
                Err(e) => {
                    evt_tx.send(ViewModelEvent::Error {
                        message: format!("Failed to blacklist: {}", e)
                    }).ok();
                }
            }
        }

        DownloadImages { market_code } => {
            // Placeholder: will implement async download with Asupersync later
            match super::commands::download_images_sync(conn, &market_code) {
                Ok(count) => {
                    evt_tx.send(ViewModelEvent::DownloadComplete { count }).ok();
                }
                Err(e) => {
                    evt_tx.send(ViewModelEvent::Error {
                        message: format!("Download failed: {}", e)
                    }).ok();
                }
            }
        }

        SetWallpaper { url } => {
            match super::commands::set_wallpaper_sync(conn, &url) {
                Ok(success) => {
                    evt_tx.send(ViewModelEvent::WallpaperSet { success }).ok();
                }
                Err(e) => {
                    evt_tx.send(ViewModelEvent::Error {
                        message: format!("Failed to set wallpaper: {}", e)
                    }).ok();
                }
            }
        }

        RefreshDatabase => {
            // No-op for now
            log::info!("RefreshDatabase command received");
        }

        Shutdown => {
            log::info!("Shutdown command received");
            // Break from message loop (handled by cmd_rx iterator ending)
        }
    }
}

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

    // Create executor for async operations (using smol)
    let ex = smol::Executor::new();

    let mut conn = crate::db::establish_connection(&db_path);

    // Message loop
    for cmd in cmd_rx {
        handle_command(&ex, &mut conn, &evt_tx, cmd);
    }

    log::info!("ViewModel background thread stopped");
}

fn handle_command(
    _ex: &smol::Executor,
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

        // NEW: Carousel operations
        ViewModelCommand::LoadCarouselPage { filter, page } => {
            log::info!("Loading carousel page {} with filter {:?}", page, filter);

            use crate::db::operations;
            use crate::schema::bing_images;
            use diesel::prelude::*;

            let offset = (page * 20) as i64;

            // Build query based on filter
            let query = if let Some(status) = filter {
                let status_str = status.as_str();
                bing_images::table
                    .filter(bing_images::status.eq(status_str))
                    .order(bing_images::fetched_at.desc())
                    .limit(20)
                    .offset(offset)
                    .load::<crate::db::BingImage>(conn)
            } else {
                // No filter = All images
                bing_images::table
                    .order(bing_images::fetched_at.desc())
                    .limit(20)
                    .offset(offset)
                    .load::<crate::db::BingImage>(conn)
            };

            match query {
                Ok(images) => {
                    // Count total for this filter
                    let total_count = if let Some(status) = filter {
                        operations::count_by_status(conn, status).unwrap_or(0) as usize
                    } else {
                        bing_images::table.count().get_result::<i64>(conn).unwrap_or(0) as usize
                    };

                    evt_tx.send(ViewModelEvent::CarouselPageLoaded {
                        page,
                        images,
                        total_count,
                    }).ok();
                }
                Err(e) => {
                    evt_tx.send(ViewModelEvent::Error {
                        message: format!("Failed to load carousel page: {}", e),
                    }).ok();
                }
            }
        }

        ViewModelCommand::LoadMainImage { url } => {
            log::info!("Loading main image: {}", url);

            use crate::viewmodel::commands;

            // Try cache first
            match commands::load_cached_image(&url) {
                Ok(Some(bytes)) => {
                    log::info!("Loaded from cache: {} ({} bytes)", url, bytes.len());
                    evt_tx.send(ViewModelEvent::MainImageLoaded {
                        url: url.clone(),
                        image_bytes: bytes.clone(),
                        cached: true,
                    }).ok();

                    // Spawn background refresh (fetch fresh from network)
                    let url_clone = url.clone();
                    let evt_tx_clone = evt_tx.clone();
                    std::thread::spawn(move || {
                        if let Ok(fresh_bytes) = commands::download_image(&url_clone) {
                            // Only emit refresh if bytes differ
                            if fresh_bytes != bytes {
                                log::info!("Image refreshed from network (changed)");
                                evt_tx_clone.send(ViewModelEvent::MainImageRefreshed {
                                    url: url_clone,
                                    image_bytes: fresh_bytes,
                                }).ok();
                            }
                        }
                    });
                }
                Ok(None) => {
                    // Not in cache, download from network
                    log::info!("Cache miss, downloading from network");
                    match commands::download_image(&url) {
                        Ok(bytes) => {
                            // Save to cache
                            commands::save_to_cache(&url, &bytes).ok();

                            evt_tx.send(ViewModelEvent::MainImageLoaded {
                                url,
                                image_bytes: bytes,
                                cached: false,
                            }).ok();
                        }
                        Err(e) => {
                            evt_tx.send(ViewModelEvent::Error {
                                message: format!("Failed to load image: {}", e),
                            }).ok();
                        }
                    }
                }
                Err(e) => {
                    evt_tx.send(ViewModelEvent::Error {
                        message: format!("Cache error: {}", e),
                    }).ok();
                }
            }
        }

        ViewModelCommand::UpdateCropCoords { url, coords } => {
            log::info!("Updating crop coords for: {}", url);

            use crate::db::operations;

            // Clamp coords to valid range
            let clamped = coords.clamp();

            // Serialize to JSON
            match clamped.to_json() {
                Ok(json) => {
                    match operations::update_crop_coords(conn, &url, Some(&json)) {
                        Ok(_) => {
                            log::info!("Crop coords saved for: {}", url);
                            evt_tx.send(ViewModelEvent::CropCoordsSaved {
                                url,
                            }).ok();
                        }
                        Err(e) => {
                            evt_tx.send(ViewModelEvent::Error {
                                message: format!("Failed to save crop coords: {}", e),
                            }).ok();
                        }
                    }
                }
                Err(e) => {
                    evt_tx.send(ViewModelEvent::Error {
                        message: format!("Failed to serialize crop coords: {}", e),
                    }).ok();
                }
            }
        }
    }
}

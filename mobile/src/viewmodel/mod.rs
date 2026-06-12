use crate::db::{BingImage, ImageStatus};
use std::sync::mpsc::{Sender, Receiver};
use std::path::PathBuf;

pub mod background;
pub mod commands;

/// Commands sent from UI to ViewModel background thread
#[derive(Debug, Clone)]
pub enum ViewModelCommand {
    DownloadImages { market_code: String },
    SetWallpaper { url: String },
    ToggleFavorite { url: String },
    BlacklistImage { url: String },
    GetImagesByStatus { status: ImageStatus },
    GetImagesByMarket { market_code: String, page: usize },
    RefreshDatabase,
    Shutdown,
}

/// Events sent from ViewModel background thread to UI
#[derive(Debug, Clone)]
pub enum ViewModelEvent {
    DownloadProgress { current: usize, total: usize },
    DownloadComplete { count: usize },
    ImagesLoaded { images: Vec<BingImage> },
    WallpaperSet { success: bool },
    StatusUpdated { url: String, status: ImageStatus },
    Error { message: String },
}

/// ViewModel struct
pub struct ViewModel {
    db_path: PathBuf,

    #[cfg(not(feature = "cli-only"))]
    command_tx: Option<Sender<ViewModelCommand>>,

    #[cfg(not(feature = "cli-only"))]
    event_rx: Option<Receiver<ViewModelEvent>>,
}

use anyhow::Result;
use std::sync::mpsc::channel;

impl ViewModel {
    /// Create async ViewModel with background thread (GUI/Android)
    #[cfg(not(feature = "cli-only"))]
    pub fn new_async(db_path: PathBuf) -> Result<Self> {
        let (cmd_tx, cmd_rx) = channel();
        let (evt_tx, evt_rx) = channel();

        let db_path_clone = db_path.clone();
        std::thread::spawn(move || {
            background::run_background_loop(db_path_clone, cmd_rx, evt_tx);
        });

        Ok(Self {
            db_path,
            command_tx: Some(cmd_tx),
            event_rx: Some(evt_rx),
        })
    }

    /// Send command to background thread
    #[cfg(not(feature = "cli-only"))]
    pub fn send_command(&self, cmd: ViewModelCommand) -> Result<()> {
        self.command_tx.as_ref()
            .expect("command_tx initialized")
            .send(cmd)?;
        Ok(())
    }

    /// Poll for events from background thread (non-blocking)
    #[cfg(not(feature = "cli-only"))]
    pub fn poll_events(&self) -> Vec<ViewModelEvent> {
        self.event_rx.as_ref()
            .expect("event_rx initialized")
            .try_iter()
            .collect()
    }

    /// Create sync ViewModel (CLI only)
    #[cfg(feature = "cli-only")]
    pub fn new_sync(db_path: PathBuf) -> Result<Self> {
        Ok(Self { db_path })
    }

    /// Download images synchronously (CLI only)
    #[cfg(feature = "cli-only")]
    pub fn download_images_sync(&self, market_code: &str) -> Result<usize> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::download_images_sync(&mut conn, market_code)
    }

    /// Get images by status synchronously (CLI only)
    #[cfg(feature = "cli-only")]
    pub fn get_images_by_status_sync(&self, status: ImageStatus) -> Result<Vec<BingImage>> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        crate::db::operations::get_images_by_status(&mut conn, status)
    }

    /// Set wallpaper synchronously (CLI only)
    #[cfg(feature = "cli-only")]
    pub fn set_wallpaper_sync(&self, url: &str) -> Result<bool> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::set_wallpaper_sync(&mut conn, url)
    }

    /// Toggle favorite synchronously (CLI only)
    #[cfg(feature = "cli-only")]
    pub fn toggle_favorite_sync(&self, url: &str) -> Result<()> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::toggle_favorite_sync(&mut conn, url)
    }

    /// Blacklist image synchronously (CLI only)
    #[cfg(feature = "cli-only")]
    pub fn blacklist_image_sync(&self, url: &str) -> Result<()> {
        let mut conn = crate::db::establish_connection(&self.db_path);
        commands::blacklist_image_sync(&mut conn, url)
    }
}

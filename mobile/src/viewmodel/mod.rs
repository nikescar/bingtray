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

/// ViewModel struct (will implement in next task)
pub struct ViewModel {
    db_path: PathBuf,

    #[cfg(not(feature = "cli-only"))]
    command_tx: Option<Sender<ViewModelCommand>>,

    #[cfg(not(feature = "cli-only"))]
    event_rx: Option<Receiver<ViewModelEvent>>,
}

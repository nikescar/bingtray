use anyhow::{Result, Context};
use bingtray_core::{FileSystemService, WallpaperService, ServiceProvider, ProjectDirectories};
use directories::ProjectDirs;

/// Concrete implementation of FileSystemService using the directories crate
pub struct ConcreteFileSystemService;

impl FileSystemService for ConcreteFileSystemService {
    fn get_project_dirs(&self) -> Result<ProjectDirectories> {
        let proj_dirs = ProjectDirs::from("com", "bingtray", "bingtray")
            .context("Failed to get project directories")?;
        
        Ok(ProjectDirectories::new(proj_dirs.config_dir().to_path_buf()))
    }
}

/// Concrete implementation of WallpaperService using the wallpaper crate
pub struct ConcreteWallpaperService;

impl WallpaperService for ConcreteWallpaperService {
    fn set_wallpaper_from_path(&self, file_path: &str) -> Result<()> {
        wallpaper::set_from_path(file_path)
            .map_err(|e| anyhow::anyhow!("Failed to set wallpaper: {}", e))?;
        Ok(())
    }
}

/// Combined service provider that uses the concrete implementations
pub struct BingtrayServiceProvider {
    filesystem_service: ConcreteFileSystemService,
    wallpaper_service: ConcreteWallpaperService,
}

impl BingtrayServiceProvider {
    pub fn new() -> Self {
        Self {
            filesystem_service: ConcreteFileSystemService,
            wallpaper_service: ConcreteWallpaperService,
        }
    }
}

impl Default for BingtrayServiceProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl FileSystemService for BingtrayServiceProvider {
    fn get_project_dirs(&self) -> Result<ProjectDirectories> {
        self.filesystem_service.get_project_dirs()
    }
}

impl WallpaperService for BingtrayServiceProvider {
    fn set_wallpaper_from_path(&self, file_path: &str) -> Result<()> {
        self.wallpaper_service.set_wallpaper_from_path(file_path)
    }
}

impl ServiceProvider for BingtrayServiceProvider {}
// Service traits and default implementations for dependency injection
use anyhow::Result;
use std::path::PathBuf;

/// Service trait for file system operations
pub trait FileSystemService {
    /// Get project directories for the application
    fn get_project_dirs(&self) -> Result<ProjectDirectories>;
}

/// Service trait for wallpaper operations
pub trait WallpaperService {
    /// Set wallpaper from file path
    fn set_wallpaper_from_path(&self, file_path: &str) -> Result<()>;
}

/// Project directories abstraction
pub struct ProjectDirectories {
    pub config_dir: PathBuf,
}

impl ProjectDirectories {
    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }
    
    pub fn config_dir(&self) -> &PathBuf {
        &self.config_dir
    }
}

/// Combined service provider for dependency injection
pub trait ServiceProvider: FileSystemService + WallpaperService {}

/// Default implementation that uses no-op or fallback behavior
pub struct DefaultServiceProvider;

impl FileSystemService for DefaultServiceProvider {
    fn get_project_dirs(&self) -> Result<ProjectDirectories> {
        // Fallback implementation for cases where directories service is not available
        #[cfg(target_os = "android")]
        {
            Ok(ProjectDirectories::new(PathBuf::from("/data/data/pe.nikescar.bingtray/files")))
        }
        
        #[cfg(target_arch = "wasm32")]
        {
            Ok(ProjectDirectories::new(PathBuf::from("/tmp/bingtray")))
        }
        
        #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
        {
            // This should be overridden by the bingtray crate implementation
            Err(anyhow::anyhow!("Project directories service not implemented"))
        }
    }
}

impl WallpaperService for DefaultServiceProvider {
    fn set_wallpaper_from_path(&self, _file_path: &str) -> Result<()> {
        // Fallback implementation
        #[cfg(any(target_os = "android", target_arch = "wasm32"))]
        {
            Err(anyhow::anyhow!("Wallpaper setting not supported on this platform"))
        }
        
        #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
        {
            // This should be overridden by the bingtray crate implementation
            Err(anyhow::anyhow!("Wallpaper service not implemented"))
        }
    }
}

impl ServiceProvider for DefaultServiceProvider {}
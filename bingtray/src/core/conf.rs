
use std::path::PathBuf;
use std::fs;
use anyhow::{Result, Context};
use directories::ProjectDirs;

pub struct Conf {
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub unprocessed_dir: PathBuf,
    pub keepfavorite_dir: PathBuf,
    pub cached_dir: PathBuf,
    
    pub blacklist_file: PathBuf,
    pub marketcodes_file: PathBuf,
    pub metadata_file: PathBuf,
    pub historical_metadata_file: PathBuf,

    pub sqlite_file: PathBuf,
}

impl Conf {
    pub fn new() -> Result<Self> {
        let (config_dir, cache_dir, unprocessed_dir, keepfavorite_dir, cached_dir, blacklist_file, marketcodes_file, metadata_file, historical_metadata_file, sqlite_file) = {
            #[cfg(target_os = "android")]
            {
                // Android-specific paths
                let config_dir = PathBuf::from("/data/data/pe.nikescar.bingtray/files");
                let cache_dir = PathBuf::from("/data/data/pe.nikescar.bingtray/cache");
                let unprocessed_dir = cache_dir.join("unprocessed");
                let keepfavorite_dir = cache_dir.join("keepfavorite");
                let cached_dir = cache_dir.join("cached");
                let blacklist_file = config_dir.join("blacklist.conf");
                let marketcodes_file = config_dir.join("marketcodes.conf");
                let metadata_file = config_dir.join("metadata.conf");
                let historical_metadata_file = config_dir.join("historical.metadata.conf");
                let sqlite_file = config_dir.join("bingtray.sqlite");
                (config_dir, cache_dir, unprocessed_dir, keepfavorite_dir, cached_dir, blacklist_file, marketcodes_file, metadata_file, historical_metadata_file, sqlite_file)
            }

            #[cfg(target_os = "ios")]
            {
                // ios-specific paths - placeholder
                let config_dir = PathBuf::from("/tmp/bingtray");
                let cache_dir = config_dir.clone();
                let unprocessed_dir = cache_dir.join("unprocessed");
                let keepfavorite_dir = cache_dir.join("keepfavorite");
                let cached_dir = cache_dir.join("cached");
                let blacklist_file = config_dir.join("blacklist.conf");
                let marketcodes_file = config_dir.join("marketcodes.conf");
                let metadata_file = config_dir.join("metadata.conf");
                let historical_metadata_file = config_dir.join("historical.metadata.conf");
                let sqlite_file = config_dir.join("bingtray.sqlite");
                (config_dir, cache_dir, unprocessed_dir, keepfavorite_dir, cached_dir, blacklist_file, marketcodes_file, metadata_file, historical_metadata_file, sqlite_file)
            }

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let proj_dirs = ProjectDirs::from("com", "bingtray", "bingtray")
                    .context("Failed to get project directories")?;
                let config_dir = proj_dirs.config_dir().to_path_buf();
                let cache_dir = config_dir.clone(); // Added cache_dir for consistency
                let unprocessed_dir = config_dir.join("unprocessed");
                let keepfavorite_dir = config_dir.join("keepfavorite");
                let cached_dir = config_dir.join("cached");
                let blacklist_file = config_dir.join("blacklist.conf");
                let marketcodes_file = config_dir.join("marketcodes.conf");
                let metadata_file = config_dir.join("metadata.conf");
                let historical_metadata_file = config_dir.join("historical.metadata.conf");
                let sqlite_file = config_dir.join("bingtray.sqlite");
                (config_dir, cache_dir, unprocessed_dir, keepfavorite_dir, cached_dir, blacklist_file, marketcodes_file, metadata_file, historical_metadata_file, sqlite_file)
            }

            // case of wasm, make files in opfs vfs like in bingtray/tmp/diesel/examples/sqlite/wasm/src/lib.rs
            // #[cfg(target_arch = "wasm32")]
            // {
            // }
        };

        // Create directories if they don't exist
        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&unprocessed_dir)?;
        fs::create_dir_all(&keepfavorite_dir)?;
        fs::create_dir_all(&cached_dir)?;

        // // Create blacklist.conf if it doesn't exist
        // if !blacklist_file.exists() {
        //     fs::write(&blacklist_file, "")?;
        // }

        // // Create metadata.conf if it doesn't exist
        // if !metadata_file.exists() {
        //     fs::write(&metadata_file, "")?;
        // }

        // // Create historical.metadata.conf if it doesn't exist with first line as "0"
        // if !historical_metadata_file.exists() {
        //     fs::write(&historical_metadata_file, "0\n")?;
        // }

        Ok(Conf {
            config_dir,
            cache_dir,
            unprocessed_dir,
            keepfavorite_dir,
            cached_dir,
            blacklist_file,
            marketcodes_file,
            metadata_file,
            historical_metadata_file,
            sqlite_file,
        })
    }


}
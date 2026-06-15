use diesel::prelude::*;
use std::path::Path;

#[cfg(not(target_arch = "wasm32"))]
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

pub mod models;
pub mod operations;

pub use models::{BingImage, ImageStatus, MarketCode, ConfigKv};

#[cfg(not(target_arch = "wasm32"))]
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

/// Get database file path
#[cfg(not(target_arch = "wasm32"))]
pub fn get_database_path() -> anyhow::Result<std::path::PathBuf> {
    use std::path::PathBuf;

    #[cfg(target_os = "android")]
    {
        Ok(PathBuf::from("/data/data/pe.nikescar.bingtray/files/bingtray.db"))
    }

    #[cfg(not(target_os = "android"))]
    {
        let config_dir = directories::ProjectDirs::from("com", "nikescar", "bingtray")
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
            .config_dir()
            .to_path_buf();

        std::fs::create_dir_all(&config_dir)?;
        Ok(config_dir.join("bingtray.db"))
    }
}

/// Establish SQLite database connection
#[cfg(not(target_arch = "wasm32"))]
pub fn establish_connection(db_path: &Path) -> SqliteConnection {
    let url = db_path.to_str().expect("Valid UTF-8 path");
    let mut conn = SqliteConnection::establish(url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", url));

    // Enable WAL mode for better concurrent access
    diesel::sql_query("PRAGMA journal_mode=WAL;")
        .execute(&mut conn)
        .expect("Failed to set WAL mode");

    // Set busy timeout to 30 seconds
    diesel::sql_query("PRAGMA busy_timeout=30000;")
        .execute(&mut conn)
        .expect("Failed to set busy timeout");

    // Run migrations
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run database migrations");

    conn
}

/// WASM stub
#[cfg(target_arch = "wasm32")]
pub fn establish_connection(_db_path: &Path) -> () {
    log::warn!("SQLite not available on WASM");
}

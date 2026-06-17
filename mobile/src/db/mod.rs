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
    #[cfg(target_os = "android")]
    {
        use std::path::PathBuf;
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

    // Set busy timeout FIRST (before any other operations)
    // This allows subsequent operations to wait instead of failing immediately
    diesel::sql_query("PRAGMA busy_timeout=30000;")
        .execute(&mut conn)
        .expect("Failed to set busy timeout");

    // Enable WAL mode for better concurrent access
    // Retry logic handles race conditions during rapid connection creation
    let mut retries = 0;
    loop {
        match diesel::sql_query("PRAGMA journal_mode=WAL;").execute(&mut conn) {
            Ok(_) => break,
            Err(e) => {
                retries += 1;
                if retries >= 5 {
                    panic!("Failed to set WAL mode after {} retries: {}", retries, e);
                }
                log::warn!("WAL mode attempt {} failed ({}), retrying...", retries, e);
                std::thread::sleep(std::time::Duration::from_millis(100 * retries));
            }
        }
    }

    // Run migrations (only the first connection will actually run them)
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run database migrations");

    conn
}

/// WASM stub
#[cfg(target_arch = "wasm32")]
pub fn establish_connection(_db_path: &Path) -> () {
    log::warn!("SQLite not available on WASM");
}

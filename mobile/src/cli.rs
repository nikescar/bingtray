//! CLI interface for Bingtray (Desktop only)
//!
//! Provides a simple menu-driven REPL for managing Bing wallpapers
//!
//! For CLI interface, since there is no ui, set/keep/black operation
//! is based on current wallpaper image on desktop.
//!

use crate::viewmodel::ViewModel;
use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::PathBuf;

/// Run the CLI mode with a REPL loop
pub fn run_cli_mode() -> Result<()> {
    // Get platform-specific config directory
    // Linux: ~/.config/bingtray/bingtray.db
    // Windows: %APPDATA%\bingtray\bingtray.db
    // macOS: ~/Library/Application Support/bingtray/bingtray.db
    let db_path = dirs::config_dir()
        .context("Could not determine config directory")?
        .join("bingtray")
        .join("bingtray.db");

    // Create config directory if it doesn't exist
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let viewmodel = ViewModel::new_sync(db_path)?;

    println!(
        "Bingtray v{} - Bing Wallpaper Manager",
        env!("CARGO_PKG_VERSION")
    );
    println!("═══════════════════════════════════════════════════════════");
    println!();

    loop {
        // Display menu
        print_menu();

        // Read user input
        print!("\nEnter your choice: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let choice = input.trim();

        // Handle choice
        match choice {
            "0" => {
                // Open cache directory
                handle_open_cache_directory()?;
            }
            "1" => {
                // Download & Set Next Wallpaper
                handle_download_and_set_next(&viewmodel)?;
            }
            "2" => {
                // Keep Current Wallpaper
                handle_keep_current_wallpaper(&viewmodel)?;
            }
            "3" => {
                // Blacklist Current Wallpaper
                handle_blacklist_current_wallpaper(&viewmodel)?;
            }
            "4" => {
                // Set Random Favorite
                handle_set_random_favorite(&viewmodel)?;
            }
            "5" | "q" | "quit" | "exit" => {
                println!("\nGoodbye!");
                break;
            }
            "" => {
                // Empty input, just show menu again
                continue;
            }
            _ => {
                println!("⚠ Invalid choice. Please enter 0-5.");
            }
        }

        println!();
    }

    Ok(())
}

/// Print the menu
fn print_menu() {
    println!("═══════════════════════════════════════════════════════════");
    println!("MENU:");
    println!("  0. Open Cache Directory");
    println!("  1. Download & Set Next Wallpaper");
    println!("  2. Keep Current Wallpaper");
    println!("  3. Blacklist Current Wallpaper");
    println!("  4. Set Random Favorite");
    println!("  5. Exit");
    println!("═══════════════════════════════════════════════════════════");
}

/// Handle option 0: Open Cache Directory
fn handle_open_cache_directory() -> Result<()> {
    let cache_dir = dirs::cache_dir()
        .context("Could not determine cache directory")?
        .join("bingtray");

    // Create cache directory if it doesn't exist
    std::fs::create_dir_all(&cache_dir)?;

    opener::open(&cache_dir)?;
    println!("✓ Opened cache directory");
    Ok(())
}

/// Handle option 1: Download & Set Next Wallpaper
fn handle_download_and_set_next(viewmodel: &ViewModel) -> Result<()> {
    println!("⏳ Downloading and setting wallpaper...");
    match viewmodel.download_and_set_next_wallpaper_sync() {
        Ok(result) => {
            println!("✓ Wallpaper set successfully!");
            println!("  Title: {}", result.title);
        }
        Err(e) => println!("✗ Error: {}", e),
    }
    Ok(())
}

/// Handle option 2: Keep Current Wallpaper
fn handle_keep_current_wallpaper(viewmodel: &ViewModel) -> Result<()> {
    println!("⏳ Marking current wallpaper as favorite...");
    match viewmodel.keep_current_wallpaper_sync() {
        Ok(Some(title)) => {
            println!("✓ Kept: \"{}\"", title);
        }
        Ok(None) => {
            println!("⚠ No matching wallpaper found in database");
            println!("  (Current wallpaper may not be from BingTray)");
        }
        Err(e) => println!("✗ Error: {}", e),
    }
    Ok(())
}

/// Handle option 3: Blacklist Current Wallpaper
fn handle_blacklist_current_wallpaper(viewmodel: &ViewModel) -> Result<()> {
    println!("⏳ Blacklisting current wallpaper...");
    match viewmodel.blacklist_current_wallpaper_sync() {
        Ok(Some(title)) => {
            println!("✓ Blacklisted: \"{}\"", title);
        }
        Ok(None) => {
            println!("⚠ No matching wallpaper found in database");
            println!("  (Current wallpaper may not be from BingTray)");
        }
        Err(e) => println!("✗ Error: {}", e),
    }
    Ok(())
}

/// Handle option 4: Set Random Favorite
fn handle_set_random_favorite(viewmodel: &ViewModel) -> Result<()> {
    println!("⏳ Setting random favorite wallpaper...");
    match viewmodel.set_random_favorite_wallpaper_sync() {
        Ok(Some(title)) => {
            println!("✓ Set favorite: \"{}\"", title);
        }
        Ok(None) => {
            println!("⚠ No favorites available");
            println!("  Use option 2 to keep some wallpapers first.");
        }
        Err(e) => println!("✗ Error: {}", e),
    }
    Ok(())
}

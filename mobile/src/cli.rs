//! CLI interface for Bingtray (Desktop only)
//!
//! Provides a simple menu-driven REPL for managing Bing wallpapers

use crate::calc_bingimage::BingTrayLogic;
use anyhow::Result;
use std::io::{self, Write};

/// Run the CLI mode with a REPL loop
pub fn run_cli_mode(logic: &mut BingTrayLogic) -> Result<()> {
    println!("🖼️  Bingtray v{} - Bing Wallpaper Manager", env!("CARGO_PKG_VERSION"));
    println!("═══════════════════════════════════════════════════════════");
    println!();

    loop {
        // Display menu
        print_menu(logic);

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
                match logic.open_cache_directory() {
                    Ok(_) => println!("✓ Opened cache directory"),
                    Err(e) => println!("✗ Error: {}", e),
                }
            }
            "1" => {
                // Next market wallpaper
                println!("⏳ Downloading and setting next wallpaper...");
                match logic.set_next_market_wallpaper() {
                    Ok(true) => {
                        println!("✓ Wallpaper set successfully!");
                        println!("  Current: {}", logic.get_current_image_title());
                    }
                    Ok(false) => {
                        println!("⚠ No wallpapers available. Please download more images.");
                    }
                    Err(e) => println!("✗ Error: {}", e),
                }
            }
            "2" => {
                // Keep current image
                if logic.can_keep() {
                    println!("⏳ Moving to favorites...");
                    match logic.keep_current_image() {
                        Ok(_) => {
                            println!("✓ Image moved to favorites!");
                            if logic.can_keep() {
                                println!("  Current: {}", logic.get_current_image_title());
                            } else {
                                println!("  No more images available");
                            }
                        }
                        Err(e) => println!("✗ Error: {}", e),
                    }
                } else {
                    println!("⚠ No current image to keep");
                }
            }
            "3" => {
                // Blacklist current image
                if logic.can_blacklist() {
                    let title = logic.get_current_image_title();
                    println!("⏳ Blacklisting \"{}\"...", title);
                    match logic.blacklist_current_image() {
                        Ok(_) => {
                            println!("✓ Image blacklisted!");
                            if logic.can_blacklist() {
                                println!("  Current: {}", logic.get_current_image_title());
                            } else {
                                println!("  No more images available");
                            }
                        }
                        Err(e) => println!("✗ Error: {}", e),
                    }
                } else {
                    println!("⚠ No current image to blacklist");
                }
            }
            "4" => {
                // Set random favorite wallpaper
                if logic.has_kept_wallpapers() {
                    println!("⏳ Setting random favorite wallpaper...");
                    match logic.set_kept_wallpaper() {
                        Ok(true) => {
                            println!("✓ Favorite wallpaper set!");
                            println!("  Current: {}", logic.get_current_image_title());
                        }
                        Ok(false) => {
                            println!("⚠ No favorite wallpapers available");
                        }
                        Err(e) => println!("✗ Error: {}", e),
                    }
                } else {
                    println!("⚠ No favorite wallpapers available. Use option 2 to keep some first.");
                }
            }
            "5" | "q" | "quit" | "exit" => {
                println!("\n👋 Goodbye!");
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

/// Print the menu with current state
fn print_menu(logic: &BingTrayLogic) {
    println!("═══════════════════════════════════════════════════════════");
    println!("MENU:");
    println!("  0. 📁 Open Cache Directory");
    println!("  1. 🔄 Download & Set Next Market Wallpaper{}",
        if logic.has_next_available() { "" } else { " (downloading...)" });

    if logic.can_keep() {
        println!("  2. ⭐ Keep \"{}\"", logic.get_current_image_title());
    } else {
        println!("  2. ⭐ Keep Current Image (no current image)");
    }

    if logic.can_blacklist() {
        println!("  3. 🚫 Blacklist \"{}\"", logic.get_current_image_title());
    } else {
        println!("  3. 🚫 Blacklist Current Image (no current image)");
    }

    println!("  4. 🎲 Random Favorite Wallpaper{}",
        if logic.has_kept_wallpapers() { "" } else { " (no favorites yet)" });
    println!("  5. 🚪 Exit");
    println!("═══════════════════════════════════════════════════════════");
}

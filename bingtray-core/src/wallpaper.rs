use anyhow::Result;
use std::path::Path;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use std::process::Command;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use crate::services::{WallpaperService, DefaultServiceProvider};

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
pub fn get_desktop_environment() -> String {
    if let Ok(desktop_session) = std::env::var("DESKTOP_SESSION") {
        let session = desktop_session.to_lowercase();
        if ["gnome", "unity", "cinnamon", "mate", "xfce4", "lxde", "fluxbox", 
            "blackbox", "openbox", "icewm", "jwm", "afterstep", "trinity", "kde"].contains(&session.as_str()) {
            return session;
        }
        
        if session.contains("xfce") || session.starts_with("xubuntu") {
            return "xfce4".to_string();
        } else if session.starts_with("ubuntustudio") {
            return "kde".to_string();
        } else if session.starts_with("ubuntu") {
            return "gnome".to_string();
        } else if session.starts_with("lubuntu") {
            return "lxde".to_string();
        } else if session.starts_with("kubuntu") {
            return "kde".to_string();
        }
    }
    
    if std::env::var("KDE_FULL_SESSION").unwrap_or_default() == "true" {
        return "kde".to_string();
    }
    
    if std::env::var("GNOME_DESKTOP_SESSION_ID").is_ok() {
        return "gnome".to_string();
    }
    
    "unknown".to_string()
}

pub fn set_wallpaper(file_path: &Path) -> Result<bool> {
    set_wallpaper_with_service(file_path, &DefaultServiceProvider)
}

pub fn set_wallpaper_with_service<S: WallpaperService>(file_path: &Path, service: &S) -> Result<bool> {
    let file_loc = file_path.to_string_lossy();
    
    // Android-specific wallpaper setting
    #[cfg(target_os = "android")]
    {
        // Read the image file and use set_wallpaper_from_bytes
        match std::fs::read(file_path) {
            Ok(_image_bytes) => {
                // This function should be provided by the mobile crate
                // For now, we'll return false as it requires mobile integration
                eprintln!("Android wallpaper setting requires mobile crate integration");
                return Ok(false);
            }
            Err(e) => {
                eprintln!("Failed to read image file for Android wallpaper: {}", e);
                return Ok(false);
            }
        }
    }
    
    // Use wallpaper service for cross-platform wallpaper setting (non-Android, non-WASM)
    #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
    {
        match service.set_wallpaper_from_path(&file_loc) {
            Ok(_) => {
                println!("Wallpaper set successfully to: {}", file_loc);
                Ok(true)
            }
            Err(e) => {
                eprintln!("Failed to set wallpaper: {}", e);
                
                // Fallback to platform-specific methods for Linux if wallpaper service fails
                return set_wallpaper_linux_fallback(file_path);
            }
        }
    }
    
    // WASM fallback - wallpaper setting not supported
    #[cfg(target_arch = "wasm32")]
    {
        eprintln!("Wallpaper setting not supported on WASM");
        Ok(false)
    }
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn set_wallpaper_linux_fallback(file_path: &Path) -> Result<bool> {
    let file_loc = file_path.to_string_lossy();
    let desktop_env = get_desktop_environment();
    
    match desktop_env.as_str() {
        "gnome" | "unity" | "cinnamon" => {
            let uri = format!("file://{}", file_loc);
            let output = Command::new("gsettings")
                .args(&["set", "org.gnome.desktop.background", "picture-uri", &uri])
                .output()?;
            Ok(output.status.success())
        }
        "mate" => {
            let output = Command::new("gsettings")
                .args(&["set", "org.mate.background", "picture-filename", &file_loc])
                .output()?;
            Ok(output.status.success())
        }
        "xfce4" => {
            // Get all monitor paths that contain "workspace0/last-image"
            let list_output = Command::new("xfconf-query")
                .args(&["-c", "xfce4-desktop", "-l"])
                .output()?;
            
            if list_output.status.success() {
                let paths = String::from_utf8_lossy(&list_output.stdout);
                let monitor_paths: Vec<&str> = paths
                    .lines()
                    .filter(|line| line.contains("workspace0/last-image"))
                    .collect();
                
                // Set wallpaper for each monitor
                for path in monitor_paths {
                    if !path.trim().is_empty() {
                        Command::new("xfconf-query")
                            .args(&["-c", "xfce4-desktop", "-p", path.trim(), "-s", &file_loc])
                            .output()?;
                    }
                }
            }
            
            // Set default properties for the primary monitor as fallback
            Command::new("xfconf-query")
                .args(&["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/image-path", "-s", &file_loc])
                .output()?;
            Command::new("xfconf-query")
                .args(&["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/image-style", "-s", "3"])
                .output()?;
            Command::new("xfconf-query")
                .args(&["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/image-show", "-s", "true"])
                .output()?;
            
            let output = Command::new("xfdesktop")
                .args(&["--reload"])
                .output()?;
            Ok(output.status.success())
        }
        "lxde" => {
            let cmd = format!("pcmanfm --set-wallpaper {} --wallpaper-mode=scaled", file_loc);
            let output = Command::new("sh")
                .args(&["-c", &cmd])
                .output()?;
            Ok(output.status.success())
        }
        "fluxbox" | "jwm" | "openbox" | "afterstep" => {
            let output = Command::new("fbsetbg")
                .arg(file_loc.as_ref())
                .output()?;
            Ok(output.status.success())
        }
        "icewm" => {
            let output = Command::new("icewmbg")
                .arg(file_loc.as_ref())
                .output()?;
            Ok(output.status.success())
        }
        "blackbox" => {
            let output = Command::new("bsetbg")
                .args(&["-full", &file_loc])
                .output()?;
            Ok(output.status.success())
        }
        _ => {
            eprintln!("Desktop environment '{}' not supported", desktop_env);
            Ok(false)
        }
    }
}

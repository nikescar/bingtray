//! Wallpaper setting abstraction (Desktop only)
//!
//! This module provides a platform-abstracted interface for setting wallpapers.
//! - Desktop: Uses the `wallpaper` crate
//! - Android: Uses injected WallpaperSetter trait (see bingtray.rs)
//! - WASM: No-op (not applicable in browser)

use anyhow::{Context, Result};
use std::path::Path;

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
use std::process::Command;

/// Detect the desktop environment (Linux only)
#[cfg(target_os = "linux")]
fn get_desktop_environment() -> String {
    // Check common environment variables
    if let Ok(de) = std::env::var("XDG_CURRENT_DESKTOP") {
        return de.to_lowercase();
    }
    if let Ok(de) = std::env::var("DESKTOP_SESSION") {
        return de.to_lowercase();
    }

    // Try to detect from running processes
    let processes = ["gnome-shell", "plasma", "xfce4-session", "mate-session", "cinnamon"];
    for process in &processes {
        if let Ok(output) = Command::new("pgrep")
            .arg("-x")
            .arg(process)
            .output()
        {
            if output.status.success() && !output.stdout.is_empty() {
                return process.trim_end_matches("-session").trim_end_matches("-shell").to_string();
            }
        }
    }

    "unknown".to_string()
}

/// Get the user who owns the X session
#[cfg(target_os = "linux")]
fn get_x_session_user() -> Option<String> {
    use std::os::unix::fs::MetadataExt;

    // Get current display from DISPLAY env var (e.g., ":0", ":1")
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
    let display_num = display.trim_start_matches(':').split('.').next().unwrap_or("0");

    // Check X11 socket owner
    let x11_socket = format!("/tmp/.X11-unix/X{}", display_num);
    if let Ok(metadata) = std::fs::metadata(&x11_socket) {
        let uid = metadata.uid();

        // Get username from UID
        if let Ok(output) = Command::new("id")
            .arg("-un")
            .arg(uid.to_string())
            .output()
        {
            if output.status.success() {
                return Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
            }
        }
    }

    // Fallback: check XAUTHORITY file owner
    if let Ok(xauth) = std::env::var("XAUTHORITY") {
        if let Ok(metadata) = std::fs::metadata(&xauth) {
            let uid = metadata.uid();
            if let Ok(output) = Command::new("id")
                .arg("-un")
                .arg(uid.to_string())
                .output()
            {
                if output.status.success() {
                    return Some(String::from_utf8_lossy(&output.stdout).trim().to_string());
                }
            }
        }
    }

    None
}

/// Check if desktop user and runtime user are different (Linux with X11)
/// Returns (current_user, desktop_user, is_different)
#[cfg(target_os = "linux")]
pub fn check_user_mismatch() -> (String, String, bool) {
    let current_user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());

    let desktop_user = get_x_session_user().unwrap_or_else(|| "unknown".to_string());

    let is_different = current_user != desktop_user &&
                       desktop_user != "unknown" &&
                       current_user != "unknown";

    (current_user, desktop_user, is_different)
}

/// Check if desktop user and runtime user are different (Windows/macOS stub)
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn check_user_mismatch() -> (String, String, bool) {
    let current_user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    (current_user.clone(), current_user, false)
}

/// Check if desktop user and runtime user are different (Android/WASM stub)
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
pub fn check_user_mismatch() -> (String, String, bool) {
    ("n/a".to_string(), "n/a".to_string(), false)
}

/// Set wallpaper from a file path (Desktop platforms)
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub fn set_wallpaper<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();

    if !path.exists() {
        anyhow::bail!("Image file does not exist: {:?}", path);
    }

    // On Linux, prioritize DE-specific methods as they're more reliable
    #[cfg(target_os = "linux")]
    {
        log::info!("Linux detected, trying DE-specific methods first");

        // Try Linux desktop environment specific method first
        match set_wallpaper_linux_fallback(path)? {
            true => {
                log::info!("Wallpaper set successfully using Linux DE method: {:?}", path);
                return Ok(());
            }
            false => {
                log::warn!("Linux DE method failed or not supported, trying custom Wallpaper app");
            }
        }

        // Try custom Wallpaper app for puppy linux
        match set_wallpaper_custom_app(path)? {
            true => {
                log::info!("Wallpaper set successfully using custom Wallpaper app: {:?}", path);
                return Ok(());
            }
            false => {
                log::warn!("Custom Wallpaper app failed, trying wallpaper crate as last resort");
            }
        }
    }

    // Try the wallpaper crate (primary method for macOS/Windows, fallback for Linux)
    match wallpaper::set_from_path(path.to_str().context("Invalid path")?) {
        Ok(_) => {
            log::info!("Wallpaper set successfully using wallpaper crate: {:?}", path);
            Ok(())
        }
        Err(e) => {
            log::error!("All wallpaper setting methods failed");
            anyhow::bail!("Failed to set wallpaper: {:?}", e)
        }
    }
}

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
fn set_wallpaper_linux_fallback(file_path: &Path) -> Result<bool> {
    let file_loc = file_path.to_string_lossy();
    let desktop_env = get_desktop_environment();

    // Fall back to DE-specific methods
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

/// Try to set wallpaper using custom Wallpaper app
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
fn set_wallpaper_custom_app(file_path: &Path) -> Result<bool> {
    let wallpaper_app = "/usr/local/apps/Wallpaper/set_bg";

    // Check if the custom wallpaper app exists
    if !std::path::Path::new(wallpaper_app).exists() {
        return Ok(false);
    }

    let file_loc = file_path.to_string_lossy();
    let output = Command::new(wallpaper_app)
        .arg(file_loc.as_ref())
        .output()?;

    Ok(output.status.success())
}

/// Set wallpaper from image bytes (Desktop platforms)
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub fn set_wallpaper_from_bytes(bytes: &[u8]) -> Result<()> {
    // Write bytes to a temporary file
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join("bingtray_wallpaper.jpg");

    std::fs::write(&temp_path, bytes)?;

    // Set wallpaper from temp file
    set_wallpaper(&temp_path)?;

    log::info!("Wallpaper set from bytes ({} bytes)", bytes.len());

    Ok(())
}

/// Set wallpaper from a file path in the cache directory (Desktop platforms)
///
/// This function expects the image to already be saved in the cache directory.
/// It simply sets the wallpaper using the provided path.
///
/// # Arguments
/// * `path` - Path to the image file in the cache directory
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub fn set_wallpaper_from_cache(path: &Path) -> Result<()> {
    log::info!("Setting wallpaper from cache: {:?}", path);

    if !path.exists() {
        anyhow::bail!("Image file does not exist: {:?}", path);
    }

    // Set wallpaper from the cached file
    set_wallpaper(path)?;

    log::info!("Wallpaper set from cache: {:?}", path);

    Ok(())
}

/// Get current wallpaper path (Desktop platforms)
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub fn get_wallpaper() -> Result<String> {
    wallpaper::get()
        .map_err(|e| anyhow::anyhow!("Failed to get wallpaper: {:?}", e))
}

// Android and WASM stubs (not implemented here)
// Android uses the WallpaperSetter trait injected in BingtrayApp
// WASM has no wallpaper setting capability

#[cfg(target_os = "android")]
pub fn set_wallpaper<P: AsRef<Path>>(_path: P) -> Result<()> {
    log::warn!("set_wallpaper called on Android - use WallpaperSetter trait instead");
    Ok(())
}

#[cfg(target_os = "android")]
pub fn set_wallpaper_from_bytes(_bytes: &[u8]) -> Result<()> {
    log::warn!("set_wallpaper_from_bytes called on Android - use WallpaperSetter trait instead");
    Ok(())
}

#[cfg(target_os = "android")]
pub fn get_wallpaper() -> Result<String> {
    Ok(String::new())
}

#[cfg(target_arch = "wasm32")]
pub fn set_wallpaper<P: AsRef<Path>>(_path: P) -> Result<()> {
    log::warn!("Wallpaper setting not available in WASM");
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn set_wallpaper_from_bytes(_bytes: &[u8]) -> Result<()> {
    log::warn!("Wallpaper setting not available in WASM");
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn get_wallpaper() -> Result<String> {
    Ok(String::new())
}

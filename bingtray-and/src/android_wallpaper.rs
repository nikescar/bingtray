use std::path::Path;
use log::info;

#[cfg(target_os = "android")]
use jni::objects::{JObject, JValue};

#[cfg(target_os = "android")]
use ndk_context;

/// Set wallpaper from image bytes using Android WallpaperManager and ByteArrayInputStream
#[cfg(target_os = "android")]
pub fn set_wallpaper_from_bytes(image_bytes: &[u8]) -> std::io::Result<bool> {
    // Create a VM for executing Java calls
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as _) }.map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Expected to find JVM via ndk_context crate",
        )
    })?;

    let activity = unsafe { jni::objects::JObject::from_raw(ctx.context() as _) };
    let mut env = vm
        .attach_current_thread()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Failed to attach current thread"))?;

    // Get WallpaperManager instance with error handling
    let wallpaper_manager_class = env.find_class("android/app/WallpaperManager")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find WallpaperManager class: {}", e)))?;

    let wallpaper_manager = env.call_static_method(
        &wallpaper_manager_class,
        "getInstance",
        "(Landroid/content/Context;)Landroid/app/WallpaperManager;",
        &[JValue::Object(&activity)],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get WallpaperManager instance: {}", e)))?;
    info!("Getting WallpaperManager instance has done.");

    // Create Java byte array from Rust bytes
    let java_byte_array = env.byte_array_from_slice(image_bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create Java byte array: {}", e)))?;
    info!("Create Java byte array from Rust bytes.");

    // Create Bitmap using BitmapFactory.decodeByteArray
    let bitmap_factory_class = env.find_class("android/graphics/BitmapFactory")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find BitmapFactory class: {}", e)))?;
    
    let bitmap = env.call_static_method(
        &bitmap_factory_class,
        "decodeByteArray",
        "([BII)Landroid/graphics/Bitmap;",
        &[
            JValue::Object(&JObject::from(java_byte_array)),
            JValue::Int(0),
            JValue::Int(image_bytes.len() as i32),
        ],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to decode bitmap from byte array: {}", e)))?;

    // Check if bitmap creation was successful
    let bitmap_obj = bitmap.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get bitmap object: {}", e)))?;
    if bitmap_obj.is_null() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Failed to create bitmap from image data"));
    }

    info!("Decoding bitmap from bytearray has done.");
    std::thread::yield_now();

    // Set wallpaper using setBitmap
    let result = env.call_method(
        wallpaper_manager.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get wallpaper manager object: {}", e)))?,
        "setBitmap",
        "(Landroid/graphics/Bitmap;)V",
        &[JValue::Object(&bitmap_obj)],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to set wallpaper bitmap: {}", e)))?;
    info!("Setting wallpaper from bitmap has done.");

    std::thread::yield_now();

    //quit android application
    let quit_result = env.call_method(
        &activity,
        "quit",
        "()V",
        &[],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to quit activity: {}", e)))?;
    info!("Quitting Android application has done.");

    Ok(true)
}

/// Set wallpaper from image file path using Android WallpaperManager
/// This function reads the file and calls set_wallpaper_from_bytes
#[cfg(target_os = "android")]
pub fn set_wallpaper_from_path<P: AsRef<Path>>(image_path: P) -> std::io::Result<bool> {
    // Read the image file into bytes
    let image_bytes = std::fs::read(image_path.as_ref())?;
    
    // Use the new bytes-based function
    set_wallpaper_from_bytes(&image_bytes)
}

#[cfg(not(target_os = "android"))]
pub fn set_wallpaper_from_bytes(_image_bytes: &[u8]) -> std::io::Result<bool> {
    eprintln!("Android wallpaper setting not available on this platform");
    Ok(false)
}

#[cfg(not(target_os = "android"))]
pub fn set_wallpaper_from_path<P: AsRef<Path>>(_image_path: P) -> std::io::Result<bool> {
    eprintln!("Android wallpaper setting not available on this platform");
    Ok(false)
}


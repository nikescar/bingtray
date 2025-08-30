#[cfg(target_os = "android")]
use jni::objects::{JObject, JValue};

#[cfg(target_os = "android")]
use ndk_context;

/// Set wallpaper from image bytes using Android WallpaperManager
#[cfg(target_os = "android")]
pub fn set_wallpaper_from_bytes(image_bytes: &[u8]) -> std::io::Result<bool> {
    set_wallpaper_from_bytes_with_crop(image_bytes, None)
}

/// Set wallpaper from image bytes using AndroidServiceWallpaper Java class
#[cfg(target_os = "android")]
pub fn set_wallpaper_from_bytes_with_crop(
    image_bytes: &[u8],
    _crop_rect: Option<(i32, i32, i32, i32)>, // Currently unused - cropping handled in caller
) -> std::io::Result<bool> {
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

    // Find the AndroidServiceWallpaper class
    let wallpaper_service_class = env.find_class("pe/nikescar/bingtray/AndroidServiceWallpaper")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find AndroidServiceWallpaper class: {}", e)))?;

    // Create Java byte array from Rust bytes
    let java_byte_array = env.byte_array_from_slice(image_bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create Java byte array: {}", e)))?;

    log::info!("Calling AndroidServiceWallpaper.setWallpaperFromBytesJNI with {} bytes", image_bytes.len());

    // Call the static JNI wrapper method
    let result = env.call_static_method(
        &wallpaper_service_class,
        "setWallpaperFromBytesJNI",
        "(Landroid/content/Context;[B)Z",
        &[
            JValue::Object(&activity),
            JValue::Object(&JObject::from(java_byte_array)),
        ],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to call setWallpaperFromBytesJNI: {}", e)))?;

    // Get the boolean result
    let success = result.z().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get boolean result: {}", e)))?;
    
    if success {
        log::info!("AndroidServiceWallpaper completed successfully");
        Ok(true)
    } else {
        log::error!("AndroidServiceWallpaper failed");
        Err(std::io::Error::new(std::io::ErrorKind::Other, "AndroidServiceWallpaper failed to set wallpaper"))
    }
}

#[cfg(not(target_os = "android"))]
pub fn set_wallpaper_from_bytes(_image_bytes: &[u8]) -> std::io::Result<bool> {
    eprintln!("Android wallpaper setting not available on this platform");
    Ok(false)
}

#[cfg(not(target_os = "android"))]
pub fn set_wallpaper_from_bytes_with_crop(
    _image_bytes: &[u8],
    _crop_rect: Option<(i32, i32, i32, i32)>,
) -> std::io::Result<bool> {
    eprintln!("Android wallpaper setting not available on this platform");
    Ok(false)
}
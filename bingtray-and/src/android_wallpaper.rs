use std::path::Path;
use log::info;

#[cfg(target_os = "android")]
use jni::objects::{JObject, JValue};

#[cfg(target_os = "android")]
use ndk_context;

/// Set wallpaper from image file path using Android WallpaperManager
#[cfg(target_os = "android")]
pub fn set_wallpaper_from_path<P: AsRef<Path>>(image_path: P) -> std::io::Result<bool> {
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
        .attach_current_thread_as_daemon()
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

    // Extract the JObject from JValueGen to avoid move issues
    let wallpaper_manager_obj = wallpaper_manager.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get wallpaper manager object: {}", e)))?;

    // Convert path to Java string
    let image_path_str = image_path.as_ref().to_str()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid UTF-8 path"))?;
    let java_path = env.new_string(image_path_str)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create Java string: {}", e)))?;

    // Create File object
    let file_class = env.find_class("java/io/File")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find File class: {}", e)))?;
    let file = env.new_object(
        &file_class,
        "(Ljava/lang/String;)V",
        &[JValue::Object(&JObject::from(java_path))],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create File object: {}", e)))?;

    // Create FileInputStream
    let file_input_stream_class = env.find_class("java/io/FileInputStream")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find FileInputStream class: {}", e)))?;
    let file_input_stream = env.new_object(
        &file_input_stream_class,
        "(Ljava/io/File;)V",
        &[JValue::Object(&file)],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create FileInputStream: {}", e)))?;

    // Set wallpaper using InputStream - this is the potentially blocking operation
    let result = env.call_method(
        &wallpaper_manager_obj,
        "setStream",
        "(Ljava/io/InputStream;)V",
        &[JValue::Object(&file_input_stream)],
    );
    info!("Setting wallpaper has done.");

    // Always close the stream, regardless of wallpaper setting result
    let close_result = env.call_method(
        &file_input_stream,
        "close",
        "()V",
        &[],
    );

    // Check results
    result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to set wallpaper: {}", e)))?;
    close_result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to close stream: {}", e)))?;

    //clear of wallpapermanager
    // let clear_result = env.call_method(
    //     &wallpaper_manager_obj,
    //     "clear",
    //     "()V",
    //     &[],
    // ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to clear WallpaperManager: {}", e)))?;

    // close, quit exit the rust app
    

    Ok(true)
}

#[cfg(not(target_os = "android"))]
pub fn set_wallpaper_from_path<P: AsRef<Path>>(_image_path: P) -> std::io::Result<bool> {
    eprintln!("Android wallpaper setting not available on this platform");
    Ok(false)
}


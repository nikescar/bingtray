#[cfg(target_os = "android")]
use jni::objects::{JObject, JValue};

#[cfg(target_os = "android")]
use ndk_context;

/// Set wallpaper from image bytes using Android WallpaperManager
#[cfg(target_os = "android")]
pub fn set_wallpaper_from_bytes(image_bytes: &[u8]) -> std::io::Result<bool> {
    set_wallpaper_from_bytes_with_crop(image_bytes, None)
}

/// Set wallpaper from image bytes using Android foreground service
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

    log::info!("Setting wallpaper using foreground service with {} bytes", image_bytes.len());

    // Find the WallpaperService class using the application's ClassLoader (works from native threads)
    let wallpaper_service_class = (|| -> Result<jni::objects::JClass, String> {
        // Get the app's ClassLoader from the Activity/Context
        let loader_val = env
            .call_method(&activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
            .map_err(|e| format!("Failed to get class loader: {}", e))?;
        let loader = loader_val.l().map_err(|e| format!("Failed to convert loader to object: {}", e))?;

        // Prepare the class name string
        let cls_name = env
            .new_string("pe.nikescar.bingtray.BingtrayWallpaperService")
            .map_err(|e| format!("Failed to create class name string: {}", e))?;

        // Convert JString to JObject and keep it in a local variable so we can pass a reference
        let cls_name_obj = JObject::from(cls_name);

        // Call ClassLoader.loadClass(String)
        let cls_obj = env
            .call_method(loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&cls_name_obj)])
            .map_err(|e| format!("Failed to call loadClass: {}", e))?;
        let cls = cls_obj.l().map_err(|e| format!("Failed to convert loaded class to object: {}", e))?;

        Ok(jni::objects::JClass::from(cls))
    })().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find BingtrayWallpaperService class: {}", e)))?;

    // Create Java byte array from Rust bytes
    let java_byte_array = env.byte_array_from_slice(image_bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create Java byte array: {}", e)))?;

    // Use application context instead of activity to start the service
    let app_ctx_obj = env
        .call_method(&activity, "getApplicationContext", "()Landroid/content/Context;", &[])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get application context: {}", e)))?
        .l()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to convert application context to object: {}", e)))?;

    // Call the static method to start the wallpaper service
    let result = env.call_static_method(
        &wallpaper_service_class,
        "startWallpaperService",
        "(Landroid/content/Context;[B)Z",
        &[
            JValue::Object(&app_ctx_obj),
            JValue::Object(&JObject::from(java_byte_array)),
        ],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to call startWallpaperService: {}", e)))?;

    // Get the boolean result
    let success = result.z().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get boolean result: {}", e)))?;
    
    if success {
        log::info!("WallpaperService started successfully");
        Ok(true)
    } else {
        log::error!("Failed to start WallpaperService");
        Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to start WallpaperService"))
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
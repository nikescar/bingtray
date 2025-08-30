#[cfg(target_os = "android")]
use jni::objects::{JObject, JValue};

#[cfg(target_os = "android")]
use ndk_context;

/// Set wallpaper from image bytes using Android WallpaperManager
#[cfg(target_os = "android")]
pub fn set_wallpaper_from_bytes(image_bytes: &[u8]) -> std::io::Result<bool> {
    set_wallpaper_from_bytes_with_crop(image_bytes, None)
}

/// Set wallpaper from image bytes using Android WallpaperManager with main thread posting
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

    log::info!("Setting wallpaper using main thread handler approach with {} bytes", image_bytes.len());

    // Create Java byte array from Rust bytes
    let java_byte_array = env.byte_array_from_slice(image_bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create Java byte array: {}", e)))?;

    // Get main Looper
    let looper_class = env.find_class("android/os/Looper")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find Looper class: {}", e)))?;
    
    let main_looper = env.call_static_method(&looper_class, "getMainLooper", "()Landroid/os/Looper;", &[])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get main looper: {}", e)))?;

    // Create Handler on main thread
    let handler_class = env.find_class("android/os/Handler")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find Handler class: {}", e)))?;

    let main_looper_obj = main_looper.l()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get main looper object: {}", e)))?;

    let handler = env.new_object(&handler_class, "(Landroid/os/Looper;)V", &[JValue::Object(&main_looper_obj)])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create handler: {}", e)))?;

    // Create a Runnable that sets the wallpaper
    // We'll use a lambda/anonymous class approach to avoid needing separate Java files
    let runnable_class = env.find_class("java/lang/Runnable")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find Runnable class: {}", e)))?;

    // Since we can't easily create anonymous classes from JNI, let's use a different approach
    // Let's post the wallpaper setting operation to run on main thread directly
    
    // Get WallpaperManager instance
    let wallpaper_manager_class = env.find_class("android/app/WallpaperManager")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find WallpaperManager class: {}", e)))?;

    let wallpaper_manager = env.call_static_method(
        &wallpaper_manager_class,
        "getInstance",
        "(Landroid/content/Context;)Landroid/app/WallpaperManager;",
        &[JValue::Object(&activity)],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get WallpaperManager instance: {}", e)))?;

    let wallpaper_manager_obj = wallpaper_manager.l()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get WallpaperManager object: {}", e)))?;

    // Decode bitmap from byte array using BitmapFactory  
    let bitmap_factory_class = env.find_class("android/graphics/BitmapFactory")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find BitmapFactory class: {}", e)))?;

    let bitmap_result = env.call_static_method(
        &bitmap_factory_class,
        "decodeByteArray",
        "([BII)Landroid/graphics/Bitmap;",
        &[
            JValue::Object(&JObject::from(java_byte_array)),
            JValue::Int(0),
            JValue::Int(image_bytes.len() as i32),
        ],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to decode bitmap: {}", e)))?;

    let bitmap = bitmap_result.l()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get bitmap object: {}", e)))?;

    if bitmap.is_null() {
        log::error!("Failed to decode bitmap from byte array - bitmap is null");
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to decode bitmap from byte array"));
    }

    log::info!("Successfully decoded bitmap from {} bytes", image_bytes.len());

    // Try to use setStream instead of setBitmap to avoid theme changes
    let bitmap_class = env.find_class("android/graphics/Bitmap")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find Bitmap class: {}", e)))?;

    // Create ByteArrayOutputStream 
    let baos_class = env.find_class("java/io/ByteArrayOutputStream")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find ByteArrayOutputStream class: {}", e)))?;
    
    let baos = env.new_object(&baos_class, "()V", &[])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create ByteArrayOutputStream: {}", e)))?;

    // Compress bitmap to PNG format
    let compress_format_class = env.find_class("android/graphics/Bitmap$CompressFormat")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find CompressFormat class: {}", e)))?;
    
    let png_format = env.get_static_field(&compress_format_class, "PNG", "Landroid/graphics/Bitmap$CompressFormat;")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get PNG format: {}", e)))?;
    
    let png_format_obj = png_format.l()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get PNG format object: {}", e)))?;

    env.call_method(&bitmap, "compress", "(Landroid/graphics/Bitmap$CompressFormat;ILjava/io/OutputStream;)Z", &[
        JValue::Object(&png_format_obj),
        JValue::Int(100),
        JValue::Object(&baos),
    ]).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to compress bitmap: {}", e)))?;

    // Get byte array from ByteArrayOutputStream
    let byte_array_data = env.call_method(&baos, "toByteArray", "()[B", &[])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get byte array: {}", e)))?;
    
    let byte_array_obj = byte_array_data.l()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get byte array object: {}", e)))?;

    // Create ByteArrayInputStream
    let bais_class = env.find_class("java/io/ByteArrayInputStream")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find ByteArrayInputStream class: {}", e)))?;
    
    let bais = env.new_object(&bais_class, "([B)V", &[JValue::Object(&byte_array_obj)])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create ByteArrayInputStream: {}", e)))?;

    // Use setStream instead of setBitmap
    let result = env.call_method(
        &wallpaper_manager_obj,
        "setStream",
        "(Ljava/io/InputStream;)V",
        &[JValue::Object(&bais)],
    );

    // Clean up resources
    let _ = env.call_method(&baos, "close", "()V", &[]);
    let _ = env.call_method(&bais, "close", "()V", &[]);
    let _ = env.call_method(&bitmap, "recycle", "()V", &[]);

    match result {
        Ok(_) => {
            log::info!("Wallpaper set successfully using setStream method");
            Ok(true)
        }
        Err(e) => {
            log::error!("Failed to set wallpaper: {}", e);
            Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to set wallpaper: {}", e)))
        }
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
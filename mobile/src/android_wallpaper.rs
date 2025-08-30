
#[cfg(target_os = "android")]
use jni::objects::{JObject, JValue};

#[cfg(target_os = "android")]
use ndk_context;

/// Set wallpaper from image bytes using Android WallpaperManager
#[cfg(target_os = "android")]
pub fn set_wallpaper_from_bytes(image_bytes: &[u8]) -> std::io::Result<bool> {
    set_wallpaper_from_bytes_with_crop(image_bytes, None)
}

/// Set wallpaper from image bytes using Android WallpaperManager with optional crop hint
#[cfg(target_os = "android")]
pub fn set_wallpaper_from_bytes_with_crop(
    image_bytes: &[u8],
    crop_rect: Option<(i32, i32, i32, i32)>, // (left, top, right, bottom)
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

    // Get WallpaperManager instance
    let wallpaper_manager_class = env.find_class("android/app/WallpaperManager")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find WallpaperManager class: {}", e)))?;

    let wallpaper_manager = env.call_static_method(
        &wallpaper_manager_class,
        "getInstance",
        "(Landroid/content/Context;)Landroid/app/WallpaperManager;",
        &[JValue::Object(&activity)],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get WallpaperManager instance: {}", e)))?;

    // Create Java byte array from Rust bytes
    let java_byte_array = env.byte_array_from_slice(image_bytes)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create Java byte array: {}", e)))?;

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

    log::info!("Successfully created bitmap from image bytes");

    // Set wallpaper using setBitmap with optional crop hint
    if let Some((left, top, right, bottom)) = crop_rect {
        // Create Rect object for crop hint
        let rect_class = env.find_class("android/graphics/Rect")
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find Rect class: {}", e)))?;
        
        let rect_obj = env.new_object(
            &rect_class,
            "(IIII)V",
            &[
                JValue::Int(left),
                JValue::Int(top),
                JValue::Int(right),
                JValue::Int(bottom),
            ],
        ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create Rect object: {}", e)))?;

        // Try the newer API first (API 24+), fall back to older API if it fails
        let wallpaper_manager_obj = wallpaper_manager.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get wallpaper manager object: {}", e)))?;
        let result = env.call_method(
            &wallpaper_manager_obj,
            "setBitmap",
            "(Landroid/graphics/Bitmap;Landroid/graphics/Rect;ZI)I",
            &[
                JValue::Object(&bitmap_obj),
                JValue::Object(&rect_obj),
                JValue::Bool(1u8), // allowBackup = true
                JValue::Int(1), // WallpaperManager.FLAG_SYSTEM = 1
            ],
        );
        
        match result {
            Ok(_) => {
                log::info!("Wallpaper set successfully with crop hint using new API: ({}, {}, {}, {})", left, top, right, bottom);
            }
            Err(e) => {
                log::warn!("New API failed: {}, trying older API", e);
                // Fallback to older API without the 'which' parameter
                env.call_method(
                    &wallpaper_manager_obj,
                    "setBitmap",
                    "(Landroid/graphics/Bitmap;Landroid/graphics/Rect;Z)V",
                    &[
                        JValue::Object(&bitmap_obj),
                        JValue::Object(&rect_obj),
                        JValue::Bool(1u8), // allowBackup = true
                    ],
                ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to set wallpaper bitmap with crop using fallback API: {}", e)))?;
                
                log::info!("Wallpaper set successfully with crop hint using fallback API: ({}, {}, {}, {})", left, top, right, bottom);
            }
        }
    } else {
        // Use original setBitmap method without crop hint
        let wallpaper_manager_obj = wallpaper_manager.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get wallpaper manager object: {}", e)))?;
        env.call_method(
            &wallpaper_manager_obj,
            "setBitmap",
            "(Landroid/graphics/Bitmap;)V",
            &[JValue::Object(&bitmap_obj)],
        ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to set wallpaper bitmap: {}", e)))?;
        
        log::info!("Wallpaper set successfully without crop hint");
    }

    // Gracefully finish the Android application
    // let native_activity = ctx.context() as *mut ndk_sys::ANativeActivity;
    // if !native_activity.is_null() {
    //     log::info!("Finishing native activity");
    //     unsafe {
    //         ndk_sys::ANativeActivity_finish(native_activity);
    //     }
    // } else {
    //     log::warn!("Native activity pointer is null");
    // }

    Ok(true)
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

// Rust JNI bridge to WallpaperBridge.java for live wallpaper service
// On Android, calls static methods on pe.nikescar.bingtray.WallpaperBridge
// On other platforms, provides stub implementations that return errors.

#[cfg(target_os = "android")]
use jni::objects::{GlobalRef, JObject, JValue};

#[cfg(target_os = "android")]
use ndk_context;

#[cfg(target_os = "android")]
use std::sync::OnceLock;

#[cfg(target_os = "android")]
static WALLPAPER_BRIDGE_CLASS: OnceLock<GlobalRef> = OnceLock::new();

/// Initialize the WallpaperBridge class reference
/// This should be called early during app startup when JNI env has correct classloader
#[cfg(target_os = "android")]
pub fn init_wallpaper_bridge() {
    if WALLPAPER_BRIDGE_CLASS.get().is_some() {
        return; // Already initialized
    }

    let Ok((_vm, mut env)) = get_jni_env() else {
        log::error!("Failed to get JNI env for WallpaperBridge initialization");
        return;
    };

    // Get the NativeActivity object
    let ctx = ndk_context::android_context();
    let activity = unsafe { jni::objects::JObject::from_raw(ctx.context() as _) };

    // Get the activity's class
    let Ok(_activity_class) = env.get_object_class(&activity) else {
        log::error!("Failed to get activity class");
        return;
    };

    // Get the class loader from the activity
    let Ok(class_loader) = env
        .call_method(&activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
        .and_then(|v| v.l())
    else {
        log::error!("Failed to get class loader");
        return;
    };

    // Get the class name as a Java string
    let Ok(class_name) = env.new_string("pe.nikescar.bingtray.WallpaperBridge") else {
        log::error!("Failed to create class name string");
        return;
    };

    // Load the WallpaperBridge class using the app's class loader
    let bridge_class = match env.call_method(
        &class_loader,
        "loadClass",
        "(Ljava/lang/String;)Ljava/lang/Class;",
        &[JValue::Object(&class_name)],
    ) {
        Ok(class) => match class.l() {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to extract class object: {}", e);
                return;
            }
        },
        Err(e) => {
            log::error!("Failed to load WallpaperBridge class: {}", e);
            return;
        }
    };

    // Create a global reference to keep the class alive
    match env.new_global_ref(bridge_class) {
        Ok(global_ref) => {
            let _ = WALLPAPER_BRIDGE_CLASS.set(global_ref.clone());
            log::info!("WallpaperBridge class initialized successfully");

            // Initialize the bridge with context
            let jclass: &jni::objects::JClass = global_ref.as_obj().into();
            let _ = env.call_static_method(
                jclass,
                "init",
                "(Landroid/content/Context;)V",
                &[JValue::Object(&activity)],
            );
            log::info!("WallpaperBridge.init() called");
        }
        Err(e) => {
            log::error!("Failed to create global ref: {}", e);
        }
    }
}

#[cfg(target_os = "android")]
fn get_jni_env() -> Result<(jni::JavaVM, jni::AttachGuard<'static>), std::io::Error> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as _) }.map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Failed to get JVM: {}", e),
        )
    })?;
    // SAFETY: The JavaVM outlives the AttachGuard because ndk_context holds a
    // reference for the lifetime of the process. We transmute the lifetime to
    // 'static so we can return both the VM and guard together. The guard must
    // not outlive the calling scope in practice.
    let env: jni::AttachGuard<'static> =
        unsafe { std::mem::transmute(vm.attach_current_thread().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to attach thread: {}", e),
            )
        })?) };
    Ok((vm, env))
}

/// Get the cached WallpaperBridge class reference
/// Initializes on first call if not already initialized
#[cfg(target_os = "android")]
fn get_bridge_class() -> Result<&'static GlobalRef, std::io::Error> {
    // Try to get existing class
    if let Some(class_ref) = WALLPAPER_BRIDGE_CLASS.get() {
        return Ok(class_ref);
    }

    // Initialize if not done yet
    init_wallpaper_bridge();

    // Return the now-initialized class
    WALLPAPER_BRIDGE_CLASS.get().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "WallpaperBridge class initialization failed",
        )
    })
}

/// Set static wallpaper using WallpaperManager (not live wallpaper)
#[cfg(target_os = "android")]
pub fn set_static_wallpaper_from_bytes(image_bytes: &[u8]) -> std::io::Result<bool> {
    let (_vm, mut env) = get_jni_env()?;
    let class = get_bridge_class()?;
    let jclass: &jni::objects::JClass = class.as_obj().into();

    // Create Java byte array from Rust bytes
    let java_byte_array = env.byte_array_from_slice(image_bytes).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to create Java byte array: {}", e),
        )
    })?;

    // Call setStaticWallpaperFromBytes
    let result = env
        .call_static_method(
            jclass,
            "setStaticWallpaperFromBytes",
            "([B)Z",
            &[JValue::Object(&java_byte_array.into())],
        )
        .and_then(|v| v.z())
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("setStaticWallpaperFromBytes call failed: {}", e),
            )
        })?;

    if result {
        log::info!("Static wallpaper set successfully via WallpaperManager");
        Ok(true)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to set static wallpaper",
        ))
    }
}

/// Save wallpaper image for later use (e.g., when opening the picker)
#[cfg(target_os = "android")]
pub fn save_wallpaper_image(image_bytes: &[u8]) -> std::io::Result<bool> {
    let (_vm, mut env) = get_jni_env()?;
    let class = get_bridge_class()?;
    let jclass: &jni::objects::JClass = class.as_obj().into();

    // Create Java byte array from Rust bytes
    let java_byte_array = env.byte_array_from_slice(image_bytes).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to create Java byte array: {}", e),
        )
    })?;

    // Call saveWallpaperImage
    let result = env
        .call_static_method(
            jclass,
            "saveWallpaperImage",
            "([B)Z",
            &[JValue::Object(&java_byte_array.into())],
        )
        .and_then(|v| v.z())
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("saveWallpaperImage call failed: {}", e),
            )
        })?;

    if result {
        log::info!("Wallpaper image saved successfully");
        Ok(true)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to save wallpaper image",
        ))
    }
}

/// Copy wallpaper image to public Pictures directory so media selector can see it
#[cfg(target_os = "android")]
pub fn copy_wallpaper_to_public_pictures() -> bool {
    let Ok((_vm, mut env)) = get_jni_env() else {
        log::error!("copy_wallpaper_to_public_pictures: failed to get JNI env");
        return false;
    };
    let Ok(class) = get_bridge_class() else {
        log::error!("copy_wallpaper_to_public_pictures: WallpaperBridge class not initialized");
        return false;
    };
    let jclass: &jni::objects::JClass = class.as_obj().into();
    match env
        .call_static_method(jclass, "copyWallpaperToPublicPictures", "()Z", &[])
        .and_then(|v| v.z())
    {
        Ok(result) => {
            if result {
                log::info!("Wallpaper copied to public Pictures successfully");
            } else {
                log::warn!("Failed to copy wallpaper to public Pictures");
            }
            result
        }
        Err(e) => {
            log::error!("copy_wallpaper_to_public_pictures JNI call failed: {}", e);
            false
        }
    }
}

/// Set wallpaper from image bytes using the live wallpaper service
#[cfg(target_os = "android")]
pub fn set_wallpaper_from_bytes(image_bytes: &[u8]) -> std::io::Result<bool> {
    set_wallpaper_from_bytes_with_crop(image_bytes, None)
}

/// Set wallpaper from image bytes (crop_rect parameter is kept for API compatibility but ignored)
// #[cfg(target_os = "android")]
// pub fn set_wallpaper_from_bytes_with_crop(
//     image_bytes: &[u8],
//     _crop_rect: Option<(i32, i32, i32, i32)>, // Ignored for live wallpaper
// ) -> std::io::Result<bool> {
//     let (_vm, mut env) = get_jni_env()?;
//     let class = get_bridge_class()?;
//     let jclass: &jni::objects::JClass = class.as_obj().into();

//     // Create Java byte array from Rust bytes
//     let java_byte_array = env.byte_array_from_slice(image_bytes).map_err(|e| {
//         std::io::Error::new(
//             std::io::ErrorKind::Other,
//             format!("Failed to create Java byte array: {}", e),
//         )
//     })?;

//     // Call setWallpaperFromBytes
//     let result = env
//         .call_static_method(
//             jclass,
//             "setWallpaperFromBytes",
//             "([B)Z",
//             &[JValue::Object(&java_byte_array.into())],
//         )
//         .and_then(|v| v.z())
//         .map_err(|e| {
//             std::io::Error::new(
//                 std::io::ErrorKind::Other,
//                 format!("setWallpaperFromBytes call failed: {}", e),
//             )
//         })?;

//     if result {
//         log::info!("Wallpaper updated successfully via live wallpaper service");
//         Ok(true)
//     } else {
//         Err(std::io::Error::new(
//             std::io::ErrorKind::Other,
//             "Failed to update wallpaper",
//         ))
//     }
// }
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

    Ok(true)
}

/// Check if Bingtray wallpaper is currently active
#[cfg(target_os = "android")]
pub fn is_wallpaper_active() -> bool {
    let Ok((_vm, mut env)) = get_jni_env() else {
        log::error!("is_wallpaper_active: failed to get JNI env");
        return false;
    };
    let Ok(class) = get_bridge_class() else {
        log::error!("is_wallpaper_active: WallpaperBridge class not initialized");
        return false;
    };
    let jclass: &jni::objects::JClass = class.as_obj().into();
    match env
        .call_static_method(jclass, "isWallpaperActive", "()Z", &[])
        .and_then(|v| v.z())
    {
        Ok(result) => result,
        Err(e) => {
            log::error!("is_wallpaper_active JNI call failed: {}", e);
            false
        }
    }
}

/// Open the system wallpaper picker to set Bingtray as live wallpaper
#[cfg(target_os = "android")]
pub fn open_wallpaper_picker() -> bool {
    let Ok((_vm, mut env)) = get_jni_env() else {
        log::error!("open_wallpaper_picker: failed to get JNI env");
        return false;
    };
    let Ok(class) = get_bridge_class() else {
        log::error!("open_wallpaper_picker: WallpaperBridge class not initialized");
        return false;
    };
    let jclass: &jni::objects::JClass = class.as_obj().into();
    match env
        .call_static_method(jclass, "openWallpaperPicker", "()Z", &[])
        .and_then(|v| v.z())
    {
        Ok(result) => result,
        Err(e) => {
            log::error!("open_wallpaper_picker JNI call failed: {}", e);
            false
        }
    }
}

/// Test method 1: Try to open live wallpaper picker directly
#[cfg(target_os = "android")]
pub fn test_live_wallpaper_picker() -> bool {
    let Ok((_vm, mut env)) = get_jni_env() else {
        log::error!("test_live_wallpaper_picker: failed to get JNI env");
        return false;
    };
    let Ok(class) = get_bridge_class() else {
        log::error!("test_live_wallpaper_picker: WallpaperBridge class not initialized");
        return false;
    };
    let jclass: &jni::objects::JClass = class.as_obj().into();
    match env
        .call_static_method(jclass, "testLiveWallpaperPicker", "()Z", &[])
        .and_then(|v| v.z())
    {
        Ok(result) => result,
        Err(e) => {
            log::error!("test_live_wallpaper_picker JNI call failed: {}", e);
            false
        }
    }
}

/// Test method 2: Try CROP_AND_SET_WALLPAPER action
#[cfg(target_os = "android")]
pub fn test_crop_and_set_wallpaper() -> bool {
    let Ok((_vm, mut env)) = get_jni_env() else {
        log::error!("test_crop_and_set_wallpaper: failed to get JNI env");
        return false;
    };
    let Ok(class) = get_bridge_class() else {
        log::error!("test_crop_and_set_wallpaper: WallpaperBridge class not initialized");
        return false;
    };
    let jclass: &jni::objects::JClass = class.as_obj().into();
    match env
        .call_static_method(jclass, "testCropAndSetWallpaper", "()Z", &[])
        .and_then(|v| v.z())
    {
        Ok(result) => result,
        Err(e) => {
            log::error!("test_crop_and_set_wallpaper JNI call failed: {}", e);
            false
        }
    }
}

/// Test method 3: Try to open wallpaper media selector (no image provided)
#[cfg(target_os = "android")]
pub fn test_wallpaper_media_selector() -> bool {
    let Ok((_vm, mut env)) = get_jni_env() else {
        log::error!("test_wallpaper_media_selector: failed to get JNI env");
        return false;
    };
    let Ok(class) = get_bridge_class() else {
        log::error!("test_wallpaper_media_selector: WallpaperBridge class not initialized");
        return false;
    };
    let jclass: &jni::objects::JClass = class.as_obj().into();
    match env
        .call_static_method(jclass, "testWallpaperMediaSelector", "()Z", &[])
        .and_then(|v| v.z())
    {
        Ok(result) => result,
        Err(e) => {
            log::error!("test_wallpaper_media_selector JNI call failed: {}", e);
            false
        }
    }
}

// --- Non-Android stubs ---

#[cfg(not(target_os = "android"))]
pub fn init_wallpaper_bridge() {
    // No-op on non-Android platforms
}

#[cfg(not(target_os = "android"))]
pub fn set_static_wallpaper_from_bytes(_image_bytes: &[u8]) -> std::io::Result<bool> {
    eprintln!("Android static wallpaper setting not available on this platform");
    Ok(false)
}

#[cfg(not(target_os = "android"))]
pub fn save_wallpaper_image(_image_bytes: &[u8]) -> std::io::Result<bool> {
    eprintln!("Android wallpaper image saving not available on this platform");
    Ok(false)
}

#[cfg(not(target_os = "android"))]
pub fn copy_wallpaper_to_public_pictures() -> bool {
    false
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

#[cfg(not(target_os = "android"))]
pub fn is_wallpaper_active() -> bool {
    false
}

#[cfg(not(target_os = "android"))]
pub fn open_wallpaper_picker() -> bool {
    false
}

#[cfg(not(target_os = "android"))]
pub fn test_live_wallpaper_picker() -> bool {
    false
}

#[cfg(not(target_os = "android"))]
pub fn test_crop_and_set_wallpaper() -> bool {
    false
}

#[cfg(not(target_os = "android"))]
pub fn test_wallpaper_media_selector() -> bool {
    false
}

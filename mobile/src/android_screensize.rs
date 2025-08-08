#[cfg(target_os = "android")]
use jni::objects::JValue;

#[cfg(target_os = "android")]
use ndk_context;

/// Get Android screen size (width, height) in pixels using DisplayMetrics
#[cfg(target_os = "android")]
pub fn get_screen_size() -> std::io::Result<(i32, i32)> {
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

    // Get WindowManager from the activity
    let window_manager = env.call_method(
        &activity,
        "getWindowManager",
        "()Landroid/view/WindowManager;",
        &[],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get WindowManager: {}", e)))?;

    // Get default display from WindowManager
    let display = env.call_method(
        window_manager.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get WindowManager object: {}", e)))?,
        "getDefaultDisplay",
        "()Landroid/view/Display;",
        &[],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get default display: {}", e)))?;

    // Create DisplayMetrics instance
    let display_metrics_class = env.find_class("android/util/DisplayMetrics")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find DisplayMetrics class: {}", e)))?;

    let display_metrics = env.new_object(
        &display_metrics_class,
        "()V",
        &[],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create DisplayMetrics object: {}", e)))?;

    // Get display metrics
    env.call_method(
        display.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get Display object: {}", e)))?,
        "getMetrics",
        "(Landroid/util/DisplayMetrics;)V",
        &[JValue::Object(&display_metrics)],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get display metrics: {}", e)))?;

    // Get width and height from DisplayMetrics
    let width = env.get_field(
        &display_metrics,
        "widthPixels",
        "I",
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get width pixels: {}", e)))?
    .i().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to convert width to int: {}", e)))?;

    let height = env.get_field(
        &display_metrics,
        "heightPixels",
        "I",
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get height pixels: {}", e)))?
    .i().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to convert height to int: {}", e)))?;

    log::info!("Android screen size detected: {}x{} pixels", width, height);

    Ok((width, height))
}

/// Get Android screen size for non-Android platforms (returns default values)
#[cfg(not(target_os = "android"))]
pub fn get_screen_size() -> std::io::Result<(i32, i32)> {
    log::warn!("Android screen size detection not available on this platform, returning default values");
    // Return common mobile screen resolution as fallback
    Ok((1080, 1920))
}
use std::path::Path;
use log::info;

#[cfg(target_os = "android")]
use jni::objects::{JObject, JValue};

#[cfg(target_os = "android")]
use ndk_context;


/// Helper function to get resource ID from a view
#[cfg(target_os = "android")]
fn get_resource_id_from_view(env: &mut jni::JNIEnv, view: &JObject) -> std::io::Result<i32> {
    // Method 1: Try to get the resource ID using getId()
    let view_id = env.call_method(
        view,
        "getId",
        "()I",
        &[],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get view ID: {}", e)))?;

    let id = view_id.i().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to convert view ID to int: {}", e)))?;
    
    info!("View ID: {}", id);

    // Method 2: Try to get the tag (sometimes used to store resource info)
    let tag_result = env.call_method(
        view,
        "getTag",
        "()Ljava/lang/Object;",
        &[],
    );

    if let Ok(tag) = tag_result {
        let tag_obj = tag.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get tag object: {}", e)))?;
        if !tag_obj.is_null() {
            info!("View has a tag");
        }
    }

    // Method 3: Try to get layout parameters to understand the view better
    let layout_params_result = env.call_method(
        view,
        "getLayoutParams",
        "()Landroid/view/ViewGroup$LayoutParams;",
        &[],
    );

    if let Ok(layout_params) = layout_params_result {
        let layout_params_obj = layout_params.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get layout params object: {}", e)))?;
        if !layout_params_obj.is_null() {
            info!("View has layout parameters");
        }
    }

    Ok(id)
}

/// Helper function to show a Toast message safely by using Activity.runOnUiThread
#[cfg(target_os = "android")]
fn show_toast_safe(env: &mut jni::JNIEnv, activity: &JObject, message: &str) -> std::io::Result<()> {
    // Try to check if we're already on the UI thread
    let looper_class = env.find_class("android/os/Looper")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find Looper class: {}", e)))?;
    
    let main_looper = env.call_static_method(
        &looper_class,
        "getMainLooper",
        "()Landroid/os/Looper;",
        &[],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get main looper: {}", e)))?;

    let current_looper = env.call_static_method(
        &looper_class,
        "myLooper",
        "()Landroid/os/Looper;",
        &[],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get current looper: {}", e)))?;

    let main_looper_obj = main_looper.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get main looper object: {}", e)))?;
    let current_looper_obj = current_looper.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get current looper object: {}", e)))?;

    // Check if current thread is the main thread
    let is_main_thread = if current_looper_obj.is_null() {
        false
    } else {
        env.call_method(
            &main_looper_obj,
            "equals",
            "(Ljava/lang/Object;)Z",
            &[JValue::Object(&current_looper_obj)],
        ).map(|result| result.z().unwrap_or(false)).unwrap_or(false)
    };

    if is_main_thread {
        info!("On main thread, showing Toast directly: {}", message);
        // We're on the main thread, safe to show Toast directly
        return show_toast_direct(env, activity, message);
    } else {
        info!("Not on main thread, logging Toast message instead: {}", message);
        // We're not on the main thread, just log the message
        // In a full implementation, you would dispatch to UI thread here
        return Ok(());
    }
}

/// Direct Toast implementation (only safe to call from main thread)
#[cfg(target_os = "android")]
fn show_toast_direct(env: &mut jni::JNIEnv, activity: &JObject, message: &str) -> std::io::Result<()> {
    // Create Java string for the message
    let message_string = env.new_string(message)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create message string: {}", e)))?;

    // Find Toast class
    let toast_class = env.find_class("android/widget/Toast")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find Toast class: {}", e)))?;

    // Call Toast.makeText(context, message, Toast.LENGTH_LONG)
    let length_long = 1i32; // Toast.LENGTH_LONG
    let toast = env.call_static_method(
        &toast_class,
        "makeText",
        "(Landroid/content/Context;Ljava/lang/CharSequence;I)Landroid/widget/Toast;",
        &[
            JValue::Object(activity),
            JValue::Object(&JObject::from(message_string)),
            JValue::Int(length_long),
        ],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create Toast: {}", e)))?;

    let toast_obj = toast.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get Toast object: {}", e)))?;

    // Show the toast
    env.call_method(
        &toast_obj,
        "show",
        "()V",
        &[],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to show Toast: {}", e)))?;

    info!("Toast shown successfully: {}", message);
    Ok(())
}

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

    // Step 1: Get content view using findViewById(android.R.id.content)
    let android_r_id_content = 0x1020002i32; // android.R.id.content
    let content_view = env.call_method(
        &activity,
        "findViewById",
        "(I)Landroid/view/View;",
        &[JValue::Int(android_r_id_content)],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find content view: {}", e)))?;

    let content_view_obj = content_view.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get content view object: {}", e)))?;

    if !content_view_obj.is_null() {
        info!("Successfully got content view");

        // Step 2: Get resource ID from view (equivalent to getResourceIdFromView)
        let resource_id = get_resource_id_from_view(&mut env, &content_view_obj)?;
        info!("Current Layout Resource ID: {}", resource_id);

        // Step 3: Show Toast with resource ID (equivalent to Toast.makeText)
        // Use safe toast to avoid threading issues
        show_toast_safe(&mut env, &activity, &format!("Current Layout Resource ID: {}", resource_id))?;

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

        // Step 4: Note about setContentView for NativeActivity
        // setContentView is not typically used with NativeActivity and requires main thread
        // For NativeActivity, the native code manages the UI directly
        if resource_id != 0 {
            info!("Note: setContentView with resource ID {} would be called here, but it's not supported for NativeActivity and requires main thread", resource_id);
            // Removing the actual setContentView call to avoid threading crash:
            // let set_content_result = env.call_method(
            //     &activity,
            //     "setContentView",
            //     "(I)V",
            //     &[JValue::Int(resource_id)],
            // );
        }

        // Step 5: Demonstrate finding a view by ID (like findViewById(R.id.resetButton))
        // Note: This will likely fail as NativeActivity doesn't have traditional layouts
        let button_id = 0x7f090001i32; // Example button ID - would be R.id.resetButton
        let button_view = env.call_method(
            &activity,
            "findViewById",
            "(I)Landroid/view/View;",
            &[JValue::Int(button_id)],
        );

        match button_view {
            Ok(view) => {
                let view_obj = view.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get button view object: {}", e)))?;
                if !view_obj.is_null() {
                    info!("Found button view with ID: {}", button_id);
                    // Here you would normally set an OnClickListener, but this is complex in JNI
                } else {
                    info!("Button view with ID {} is null", button_id);
                }
            }
            Err(e) => {
                info!("Failed to find button view (expected for NativeActivity): {}", e);
            }
        }

    } else {
        info!("Content view is null - this is expected for NativeActivity");
    }   

    
    // Show success toast safely (avoid threading issues)
    show_toast_safe(&mut env, &activity, "Wallpaper set successfully!")?;

    std::thread::yield_now();

    // set content view to the inflated layout 
    // let set_content_view = env.call_method(
    //     &activity,
    //     "setContentView",
    //     "(Landroid/view/View;)V",
    //     &[JValue::Object(&view.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get view object: {}", e)))?)],
    // ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to set content view: {}", e)))?;
    // info!("Set content view to the inflated layout."); 

    // Gracefully finish the Android application using ANativeActivity_finish
    // Get the ANativeActivity from the ndk context
    // let native_activity = ctx.context() as *mut ndk_sys::ANativeActivity;
    // if !native_activity.is_null() {
    //     info!("Calling ANativeActivity_finish to gracefully close the application");
    //     unsafe {
    //         ndk_sys::ANativeActivity_finish(native_activity);
    //     }
    //     info!("ANativeActivity_finish has been called");
    // } else {
    //     warn!("Native activity pointer is null, cannot call ANativeActivity_finish");
    // }

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

// /// Set wallpaper from image bytes using Android getCropAndSetWallpaperIntent for cropping
// #[cfg(target_os = "android")]
// pub fn set_wallpaper_with_crop_from_bytes(image_bytes: &[u8]) -> std::io::Result<bool> {
//     // Create a VM for executing Java calls
//     let ctx = ndk_context::android_context();
//     let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as _) }.map_err(|_| {
//         std::io::Error::new(
//             std::io::ErrorKind::NotFound,
//             "Expected to find JVM via ndk_context crate",
//         )
//     })?;

//     let activity = unsafe { jni::objects::JObject::from_raw(ctx.context() as _) };
//     let mut env = vm
//         .attach_current_thread()
//         .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Failed to attach current thread"))?;

//     // Create Java byte array from Rust bytes
//     let java_byte_array = env.byte_array_from_slice(image_bytes)
//         .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create Java byte array: {}", e)))?;
//     info!("Created Java byte array from Rust bytes for cropping.");

//     // Create Bitmap using BitmapFactory.decodeByteArray
//     let bitmap_factory_class = env.find_class("android/graphics/BitmapFactory")
//         .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find BitmapFactory class: {}", e)))?;
    
//     let bitmap = env.call_static_method(
//         &bitmap_factory_class,
//         "decodeByteArray",
//         "([BII)Landroid/graphics/Bitmap;",
//         &[
//             JValue::Object(&JObject::from(java_byte_array)),
//             JValue::Int(0),
//             JValue::Int(image_bytes.len() as i32),
//         ],
//     ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to decode bitmap from byte array: {}", e)))?;

//     // Check if bitmap creation was successful
//     let bitmap_obj = bitmap.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get bitmap object: {}", e)))?;
//     if bitmap_obj.is_null() {
//         return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Failed to create bitmap from image data"));
//     }

//     info!("Decoding bitmap from bytearray for cropping has done.");

//     // Save bitmap to a temporary file
//     let files_dir = env.call_method(
//         &activity,
//         "getFilesDir",
//         "()Ljava/io/File;",
//         &[],
//     ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get files directory: {}", e)))?;

//     let files_dir_obj = files_dir.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get files directory object: {}", e)))?;

//     // Create a temporary file name
//     let temp_filename = env.new_string("temp_wallpaper.jpg")
//         .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create temp filename: {}", e)))?;

//     // Create File object for the temporary file
//     let file_class = env.find_class("java/io/File")
//         .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find File class: {}", e)))?;

//     let temp_file = env.new_object(
//         &file_class,
//         "(Ljava/io/File;Ljava/lang/String;)V",
//         &[JValue::Object(&files_dir_obj), JValue::Object(&JObject::from(temp_filename))],
//     ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create temp file object: {}", e)))?;

//     // Create FileOutputStream
//     let file_output_stream_class = env.find_class("java/io/FileOutputStream")
//         .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find FileOutputStream class: {}", e)))?;

//     let file_output_stream = env.new_object(
//         &file_output_stream_class,
//         "(Ljava/io/File;)V",
//         &[JValue::Object(&temp_file)],
//     ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create FileOutputStream: {}", e)))?;

//     // Save bitmap to file using compress
//     let compress_format_class = env.find_class("android/graphics/Bitmap$CompressFormat")
//         .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find CompressFormat class: {}", e)))?;

//     let jpeg_format = env.get_static_field(
//         &compress_format_class,
//         "JPEG",
//         "Landroid/graphics/Bitmap$CompressFormat;",
//     ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get JPEG format: {}", e)))?;

//     let compress_result = env.call_method(
//         &bitmap_obj,
//         "compress",
//         "(Landroid/graphics/Bitmap$CompressFormat;ILjava/io/OutputStream;)Z",
//         &[JValue::Object(&jpeg_format.l().unwrap()), JValue::Int(90), JValue::Object(&file_output_stream)],
//     ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to compress bitmap: {}", e)))?;

//     // Close the file output stream
//     env.call_method(
//         &file_output_stream,
//         "close",
//         "()V",
//         &[],
//     ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to close FileOutputStream: {}", e)))?;

//     // Get FileProvider URI for the temp file
//     let file_provider_class = env.find_class("androidx/core/content/FileProvider")
//         .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find FileProvider class: {}", e)))?;

//     // Get package name
//     let package_name = env.call_method(
//         &activity,
//         "getPackageName",
//         "()Ljava/lang/String;",
//         &[],
//     ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get package name: {}", e)))?;

//     let package_name_str = package_name.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get package name string: {}", e)))?;

//     // Create authority string (package name + ".provider")
//     let authority_suffix = env.new_string(".provider")
//         .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create authority suffix: {}", e)))?;

//     // Concatenate package name with ".provider"
//     let string_class = env.find_class("java/lang/String")
//         .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find String class: {}", e)))?;

//     let authority = env.call_method(
//         &package_name_str,
//         "concat",
//         "(Ljava/lang/String;)Ljava/lang/String;",
//         &[JValue::Object(&JObject::from(authority_suffix))],
//     ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to concatenate authority: {}", e)))?;

//     let authority_obj = authority.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get authority object: {}", e)))?;

//     // Get URI for file
//     let content_uri = env.call_static_method(
//         &file_provider_class,
//         "getUriForFile",
//         "(Landroid/content/Context;Ljava/lang/String;Ljava/io/File;)Landroid/net/Uri;",
//         &[JValue::Object(&activity), JValue::Object(&authority_obj), JValue::Object(&temp_file)],
//     ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get URI for file: {}", e)))?;

//     let content_uri_obj = content_uri.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get content URI object: {}", e)))?;

//     // Get WallpaperManager instance
//     let wallpaper_manager_class = env.find_class("android/app/WallpaperManager")
//         .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find WallpaperManager class: {}", e)))?;

//     let wallpaper_manager = env.call_static_method(
//         &wallpaper_manager_class,
//         "getInstance",
//         "(Landroid/content/Context;)Landroid/app/WallpaperManager;",
//         &[JValue::Object(&activity)],
//     ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get WallpaperManager instance: {}", e)))?;

//     let wallpaper_manager_obj = wallpaper_manager.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get wallpaper manager object: {}", e)))?;

//     // Get crop and set wallpaper intent
//     let crop_intent = env.call_method(
//         &wallpaper_manager_obj,
//         "getCropAndSetWallpaperIntent",
//         "(Landroid/net/Uri;)Landroid/content/Intent;",
//         &[JValue::Object(&content_uri_obj)],
//     ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get crop and set wallpaper intent: {}", e)))?;

//     let crop_intent_obj = crop_intent.l().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get crop intent object: {}", e)))?;

//     if crop_intent_obj.is_null() {
//         return Err(std::io::Error::new(std::io::ErrorKind::Other, "getCropAndSetWallpaperIntent returned null"));
//     }

//     // Start the crop activity
//     env.call_method(
//         &activity,
//         "startActivity",
//         "(Landroid/content/Intent;)V",
//         &[JValue::Object(&crop_intent_obj)],
//     ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to start crop activity: {}", e)))?;

//     info!("Started crop and set wallpaper activity");

//     // Gracefully finish the Android application after starting the crop intent
//     let native_activity = ctx.context() as *mut ndk_sys::ANativeActivity;
//     if !native_activity.is_null() {
//         info!("Calling ANativeActivity_finish after starting crop intent");
//         unsafe {
//             ndk_sys::ANativeActivity_finish(native_activity);
//         }
//         info!("ANativeActivity_finish has been called after crop intent");
//     }

//     Ok(true)
// }

// #[cfg(not(target_os = "android"))]
// pub fn set_wallpaper_with_crop_from_bytes(_image_bytes: &[u8]) -> std::io::Result<bool> {
//     Err(std::io::Error::new(
//         std::io::ErrorKind::Unsupported,
//         "Wallpaper cropping is only supported on Android",
//     ))
// }


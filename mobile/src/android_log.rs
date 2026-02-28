// usage
// send_debug_msg(&self, string tag, string msg)
// send_error_msg(&self, string tag, string msg)
// send_info_msg(&self, string tag, string msg)
// send_warn_msg(&self, string tag, string msg)
// send_verbose_msg(&self, string tag, string msg)
// send_wtf_msg(&self, string tag, string msg)

// reference
// https://developer.android.com/reference/android/util/Log
// static int 	d(String tag, String msg)
// static int 	e(String tag, String msg)
// static int 	i(String tag, String msg)
// static int 	w(String tag, String msg)
// static int 	v(String tag, String msg)
// static int 	wtf(String tag, String msg)

#[cfg(target_os = "android")]
use jni::objects::JValue;

#[cfg(target_os = "android")]
use ndk_context;

/// Internal helper to call Android Log methods
#[cfg(target_os = "android")]
fn call_log_method(method_name: &str, tag: &str, msg: &str) -> std::io::Result<i32> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as _) }.map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Expected to find JVM via ndk_context crate",
        )
    })?;

    let mut env = vm
        .attach_current_thread()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Failed to attach current thread"))?;

    // Find android.util.Log class
    let log_class = env.find_class("android/util/Log")
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to find Log class: {}", e)))?;

    // Create Java strings for tag and msg
    let j_tag = env.new_string(tag)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create tag string: {}", e)))?;
    let j_msg = env.new_string(msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create msg string: {}", e)))?;

    // Call the static log method
    let result = env.call_static_method(
        &log_class,
        method_name,
        "(Ljava/lang/String;Ljava/lang/String;)I",
        &[JValue::Object(&j_tag), JValue::Object(&j_msg)],
    ).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to call Log.{}: {}", method_name, e)))?;

    result.i().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get int result: {}", e)))
}

/// Send debug log message using Android Log.d()
#[cfg(target_os = "android")]
pub fn send_debug_msg(tag: &str, msg: &str) -> std::io::Result<i32> {
    call_log_method("d", tag, msg)
}

/// Send error log message using Android Log.e()
#[cfg(target_os = "android")]
pub fn send_error_msg(tag: &str, msg: &str) -> std::io::Result<i32> {
    call_log_method("e", tag, msg)
}

/// Send info log message using Android Log.i()
#[cfg(target_os = "android")]
pub fn send_info_msg(tag: &str, msg: &str) -> std::io::Result<i32> {
    call_log_method("i", tag, msg)
}

/// Send warning log message using Android Log.w()
#[cfg(target_os = "android")]
pub fn send_warn_msg(tag: &str, msg: &str) -> std::io::Result<i32> {
    call_log_method("w", tag, msg)
}

/// Send verbose log message using Android Log.v()
#[cfg(target_os = "android")]
pub fn send_verbose_msg(tag: &str, msg: &str) -> std::io::Result<i32> {
    call_log_method("v", tag, msg)
}

/// Send wtf (What a Terrible Failure) log message using Android Log.wtf()
#[cfg(target_os = "android")]
pub fn send_wtf_msg(tag: &str, msg: &str) -> std::io::Result<i32> {
    call_log_method("wtf", tag, msg)
}

// Non-Android fallback implementations

#[cfg(not(target_os = "android"))]
pub fn send_debug_msg(tag: &str, msg: &str) -> std::io::Result<i32> {
    eprintln!("[DEBUG] {}: {}", tag, msg);
    Ok(0)
}

#[cfg(not(target_os = "android"))]
pub fn send_error_msg(tag: &str, msg: &str) -> std::io::Result<i32> {
    eprintln!("[ERROR] {}: {}", tag, msg);
    Ok(0)
}

#[cfg(not(target_os = "android"))]
pub fn send_info_msg(tag: &str, msg: &str) -> std::io::Result<i32> {
    eprintln!("[INFO] {}: {}", tag, msg);
    Ok(0)
}

#[cfg(not(target_os = "android"))]
pub fn send_warn_msg(tag: &str, msg: &str) -> std::io::Result<i32> {
    eprintln!("[WARN] {}: {}", tag, msg);
    Ok(0)
}

#[cfg(not(target_os = "android"))]
pub fn send_verbose_msg(tag: &str, msg: &str) -> std::io::Result<i32> {
    eprintln!("[VERBOSE] {}: {}", tag, msg);
    Ok(0)
}

#[cfg(not(target_os = "android"))]
pub fn send_wtf_msg(tag: &str, msg: &str) -> std::io::Result<i32> {
    eprintln!("[WTF] {}: {}", tag, msg);
    Ok(0)
}

# Bug Fix - Part 4: Add Timeout to Wallpaper Tracking
**Date**: 2026-06-15

## Issue
Current wallpaper tracking was taking too long, causing delays in CLI operations (keep/blacklist).

## Root Cause Analysis

The `get_current_desktop_wallpaper_url_sync()` function has two paths:

1. **Fast path** (works 99% of the time):
   - Read `current_wallpaper_url` from config database
   - Verify URL exists in database
   - **Very fast** - just database queries

2. **Slow fallback path** (rarely used):
   - Calls `wallpaper::get()` to query desktop environment
   - Parses filename and searches database
   - **Problem**: No timeout, can hang indefinitely on some systems
   - This path only triggers when:
     - Config tracking is missing (shouldn't happen)
     - Database was cleared
     - First run before any wallpaper set

## Why It's Slow

The `wallpaper` crate's `get()` function queries the desktop environment:
- **Linux**: Tries multiple DE-specific methods (GNOME, KDE, etc.)
- **Windows**: Queries registry
- **macOS**: Queries system preferences

On some systems (especially "unknown" desktop environments), this can:
- Try multiple methods sequentially
- Timeout on D-Bus calls
- Hang waiting for system responses

## Solution

Added **2-second timeout** using background thread:

```rust
// Spawn detection in background thread
std::thread::spawn(move || {
    let result = crate::api_setwallpaper::get_wallpaper();
    let _ = tx.send(result);
});

// Wait with timeout
match rx.recv_timeout(std::time::Duration::from_secs(2)) {
    Ok(Ok(wallpaper_path_str)) => { /* process result */ }
    Ok(Err(e)) => { log::warn!("Detection failed"); Ok(None) }
    Err(_) => { log::warn!("Detection timed out"); Ok(None) }
}
```

### Benefits
1. **Non-blocking**: Main thread continues after 2 seconds max
2. **Graceful degradation**: If detection fails/times out, just returns None
3. **No impact on normal operation**: Fast path (config tracking) is unchanged
4. **Better UX**: CLI doesn't hang on keep/blacklist operations

### Timeout Choice
- **2 seconds** is reasonable because:
  - Fast systems will respond in < 100ms
  - Slow systems get 2s to try all methods
  - Longer than 2s usually means it's stuck

## Files Modified
- `mobile/src/viewmodel/commands.rs`: Added timeout to `get_current_desktop_wallpaper_url_sync()`

## Testing

Run CLI and observe timing:
```bash
cargo run --manifest-path mobile/Cargo.toml
# Choose option 2 (Keep) or 3 (Blacklist)
# Should be instant, not hanging
```

Watch logs for timeout messages:
```
[DEBUG] Found tracked wallpaper URL: ... (fast path - instant)
[DEBUG] Attempting desktop environment wallpaper detection with 2s timeout
[WARN]  Desktop environment detection timed out after 2s - skipping wallpaper tracking
```

## Status
✅ Added 2-second timeout to wallpaper detection
✅ Fallback path now non-blocking
✅ Fast path (config tracking) unchanged
✅ Better error messages for debugging

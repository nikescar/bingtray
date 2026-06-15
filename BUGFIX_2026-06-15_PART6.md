# Bug Fix - Part 6: Menu State Consistency Across All Modes
**Date**: 2026-06-15

## Issues Fixed

### 1. Missing Status Count in CLI and GUI Menus
**Problem**: Tray menu showed "(14 available)" after "Download & Set Next Wallpaper", but CLI and GUI menus didn't show this count.

**Solution**:
- **CLI**: Added `get_unprocessed_count()` helper to query database
- **GUI**: Added wallpaper_status to `get_menu_state()` helper
- Both now show the same status as tray menu

### 2. Disabled Keep/Blacklist/Random Favorite in GUI Menu
**Problem**: GUI menu had hardcoded `false` values for menu state:
- Keep Current: Always disabled (should check if current wallpaper is already favorite)
- Blacklist Current: Always disabled (should check if there's a current wallpaper)
- Random Favorite: Always disabled (should check if there are favorites)

**Root Cause**: GUI menu used hardcoded state instead of querying database like tray menu does.

**Solution**: Implemented `get_menu_state()` helper that queries actual database state.

## Implementation Details

### CLI Menu (mobile/src/cli.rs)

Added helper function:
```rust
fn get_unprocessed_count() -> Result<i64> {
    let db_path = dirs::config_dir()?.join("bingtray").join("bingtray.db");
    let mut conn = diesel::SqliteConnection::establish(&db_path.to_string_lossy())?;
    crate::db::operations::count_by_status(&mut conn, crate::db::ImageStatus::Unprocessed)
}
```

Updated menu display:
```rust
let unprocessed_count = get_unprocessed_count().unwrap_or(0);
let status = if unprocessed_count > 0 {
    format!(" ({} available)", unprocessed_count)
} else {
    String::new()
};
println!("  1. Download & Set Next Wallpaper{}", status);
```

### GUI Menu (mobile/src/bingtray.rs)

Added comprehensive state query:
```rust
fn get_menu_state(&self) -> anyhow::Result<(bool, bool, bool, bool, String, String)> {
    // Returns: (has_next, can_keep, can_blacklist, has_kept, current_title, status)
    
    let mut conn = self.get_db_connection()?;
    
    // 1. Get unprocessed count for status display
    let unprocessed_count = operations::count_by_status(&mut conn, ImageStatus::Unprocessed)?;
    let wallpaper_status = format!("({} available)", unprocessed_count);
    
    // 2. Get current wallpaper info
    let (can_keep, can_blacklist, current_title) = 
        if let Ok(Some(url)) = get_current_desktop_wallpaper_url_sync(&mut conn) {
            if let Ok(Some(image)) = operations::get_image(&mut conn, &url) {
                let can_keep = image.status != ImageStatus::KeepFavorite.as_str();
                let can_blacklist = true;
                (can_keep, can_blacklist, image.title.clone())
            } else {
                (false, false, String::new())
            }
        } else {
            (false, false, String::new())
        };
    
    // 3. Check if there are kept wallpapers
    let has_kept_wallpapers = operations::count_by_status(&mut conn, ImageStatus::KeepFavorite)? > 0;
    
    Ok((true, can_keep, can_blacklist, has_kept_wallpapers, current_title, wallpaper_status))
}
```

Updated menu to use actual state:
```rust
let (has_next_available, can_keep, can_blacklist, has_kept_wallpapers, current_title, wallpaper_status) = {
    match self.get_menu_state() {
        Ok(state) => state,
        Err(e) => {
            error!("Failed to get menu state: {}", e);
            (true, false, false, false, String::new(), String::new())
        }
    }
};
```

## Menu State Logic

### Next Wallpaper
- **Always enabled** - auto-downloads if needed
- **Shows count**: "(14 available)"

### Keep Current
- **Enabled if**: Current wallpaper exists AND not already a favorite
- **Disabled if**: No current wallpaper OR already favorite

### Blacklist Current  
- **Enabled if**: Current wallpaper exists
- **Disabled if**: No current wallpaper

### Random Favorite
- **Enabled if**: At least one favorite exists
- **Disabled if**: No favorites

## Consistency Across Modes

All three modes (CLI, GUI, Tray) now have identical behavior:

| Feature | CLI | GUI | Tray |
|---------|-----|-----|------|
| Show unprocessed count | ✅ | ✅ | ✅ |
| Enable Keep (if applicable) | ✅ | ✅ | ✅ |
| Enable Blacklist (if current) | ✅ | ✅ | ✅ |
| Enable Random Favorite (if exists) | ✅ | ✅ | ✅ |
| Query real-time state | ✅ | ✅ | ✅ |

## Testing

### CLI Mode
```bash
cargo run --manifest-path mobile/Cargo.toml
```
Expected:
```
MENU:
  1. Download & Set Next Wallpaper (14 available)
```

### GUI Mode
```bash
cargo run --manifest-path mobile/Cargo.toml -- --gui
```
- Click hamburger menu
- Verify "Download & Set Next Wallpaper" shows "(14 available)"
- Verify Keep/Blacklist/Random Favorite enabled/disabled appropriately

### Tray Mode
```bash
cargo run --manifest-path mobile/Cargo.toml -- --tray
```
- Right-click tray icon
- Should match GUI menu state exactly

## Files Modified
- `mobile/src/cli.rs` - Added count display
- `mobile/src/bingtray.rs` - Added state query helper

## Status
✅ All menus show unprocessed count
✅ All menus enable/disable items based on actual state
✅ Consistent behavior across CLI, GUI, tray

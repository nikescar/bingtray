# CLI ViewModel Integration Design

**Date:** 2026-06-13  
**Status:** Approved  
**Author:** Claude (with user approval)

## Overview

Migrate CLI from file-based `CalcBingimage` to database-backed `ViewModel`, matching the architecture used by GUI/tray. This removes the file-based workarounds (`blacklist.conf`, `market_state.conf`) and provides a unified data layer across all entry points.

## Goals

1. **Unified architecture:** CLI, GUI, and tray all use ViewModel + database
2. **Remove file-based persistence:** All data stored in SQLite database
3. **On-demand image downloads:** Images downloaded only when needed, not cached to disk (except favorites)
4. **Desktop wallpaper matching:** CLI operations (keep/blacklist) work on current desktop wallpaper
5. **Auto-download:** When no unprocessed images exist, automatically download from Bing API

## Non-Goals

- Multi-market rotation (remains single active market)
- Historical images feature (out of scope)
- GUI changes (GUI already uses ViewModel)
- Disk caching for all images (only favorites cached)

## Architecture

### Component Structure

```
cli.rs (UI Layer)
   ↓ calls sync methods
ViewModel (Business Logic Layer)
   ↓ uses
┌──────────────┬─────────────────┬──────────────────┐
│ db/          │ api_bingimage   │ api_setwallpaper │
│ operations   │ (Bing API)      │ (Desktop API)    │
└──────────────┴─────────────────┴──────────────────┘
```

### ViewModel Responsibilities

- **Download workflow:** Check database → Download from API if needed → Store metadata → Return images
- **Wallpaper operations:** Get desktop wallpaper → Match to database → Update status → Set new wallpaper
- **Market state management:** Load/save market code + offset in database config table
- **Image status transitions:** Unprocessed → KeepFavorite or Blacklisted

### CLI Responsibilities

- REPL menu display
- User input handling
- Call ViewModel sync methods
- Display results/errors

### Database Schema

Uses existing schema (no changes needed):

**`bing_images` table:**
- `id`: Primary key
- `url`: Full Bing image URL (unique)
- `title`: Image title
- `copyright`: Copyright text
- `copyright_link`: Link to copyright info
- `market_code`: Market code (e.g., "en-US")
- `status`: "unprocessed", "keepfavorite", or "blacklisted"
- `fetched_at`: Timestamp when downloaded from API
- `updated_at`: Timestamp of last status change

**`config_kv` table:**
- `key`: Configuration key
- `value`: Configuration value

**Market state stored in config_kv:**
- Key: `"market_code"`, Value: `"en-US"` (or other market)
- Key: `"offset"`, Value: `"0"`, `"8"`, `"16"`, etc.

## New ViewModel Sync Methods

All methods below are available when CLI is compiled. They are NOT feature-gated (no `#[cfg(feature = "cli-only")]`) because they may be useful for other entry points in the future. However, they will primarily be used by the CLI initially.

### 1. `get_current_desktop_wallpaper_url_sync() -> Result<Option<String>>`

**Purpose:** Match the current desktop wallpaper to a database URL.

**Algorithm:**
1. Call `api_setwallpaper::get_current_wallpaper()` to get desktop wallpaper path
2. Extract filename from path (e.g., `/path/to/OHR_CherryBlossom_EN-US1234567890.jpg`)
3. Extract core identifier by removing prefixes/suffixes (e.g., `OHR_`, `_UHD`, `.jpg`)
4. Query database: `SELECT * FROM bing_images WHERE url LIKE '%<identifier>%'`
5. Return first match URL, or `None` if no match

**Returns:**
- `Ok(Some(url))`: Matched desktop wallpaper to database URL
- `Ok(None)`: No match found (wallpaper not from BingTray, or database cleared)
- `Err(e)`: Database or filesystem error

### 2. `download_and_set_next_wallpaper_sync() -> Result<WallpaperSetResult>`

**Purpose:** Download next wallpaper if needed, then set it as desktop wallpaper.

**Algorithm:**
1. Query database for unprocessed images: `SELECT * FROM bing_images WHERE status = 'unprocessed' ORDER BY fetched_at DESC LIMIT 1`
2. **If no unprocessed images:**
   - Load market state from config_kv: `market_code`, `offset` (default: "en-US", 0)
   - Call `api_bingimage::get_bing_images_manifest(market_code, 8, offset)`
   - Insert images into database with status='unprocessed' (upsert by URL)
   - Increment offset by 8 and save to config_kv
   - Pick first newly inserted image
3. **If unprocessed image exists:** Use that image
4. Download image bytes on-demand via `ehttp::fetch(url)` (30s timeout)
5. Set wallpaper via `api_setwallpaper::set_wallpaper_from_bytes(&bytes)`
6. Return `WallpaperSetResult { title, url }`

**Returns:**
- `Ok(result)`: Wallpaper successfully set
- `Err(e)`: Network error, API error, wallpaper setting error

**Supporting type:**
```rust
pub struct WallpaperSetResult {
    pub title: String,
    pub url: String,
}
```

### 3. `keep_current_wallpaper_sync() -> Result<Option<String>>`

**Purpose:** Mark current desktop wallpaper as favorite.

**Algorithm:**
1. Call `get_current_desktop_wallpaper_url_sync()` to get URL
2. If `None`, return `Ok(None)` (no match)
3. Update database: `UPDATE bing_images SET status = 'keepfavorite' WHERE url = ?`
4. Return image title

**Note:** Disk caching of favorites is deferred to future enhancement. For v1, favorites are re-downloaded on-demand.

**Returns:**
- `Ok(Some(title))`: Wallpaper marked as favorite
- `Ok(None)`: No matching wallpaper found
- `Err(e)`: Database error

### 4. `blacklist_current_wallpaper_sync() -> Result<Option<String>>`

**Purpose:** Mark current desktop wallpaper as blacklisted.

**Algorithm:**
1. Call `get_current_desktop_wallpaper_url_sync()` to get URL
2. If `None`, return `Ok(None)` (no match)
3. Update database: `UPDATE bing_images SET status = 'blacklisted' WHERE url = ?`
4. Do NOT cache image to disk (on-demand approach)
5. Return image title

**Returns:**
- `Ok(Some(title))`: Wallpaper marked as blacklisted
- `Ok(None)`: No matching wallpaper found
- `Err(e)`: Database error

### 5. `set_random_favorite_wallpaper_sync() -> Result<Option<String>>`

**Purpose:** Set a random favorite as desktop wallpaper.

**Algorithm:**
1. Query database: `SELECT * FROM bing_images WHERE status = 'keepfavorite'`
2. If empty, return `Ok(None)`
3. Pick one randomly (use `rand::thread_rng()`)
4. Download image bytes via `ehttp::fetch(url)`
5. Set wallpaper via `api_setwallpaper::set_wallpaper_from_bytes(&bytes)`
6. Return image title

**Returns:**
- `Ok(Some(title))`: Random favorite set as wallpaper
- `Ok(None)`: No favorites available
- `Err(e)`: Network error or wallpaper setting error

### Supporting Helper Methods

**`get_market_state_sync() -> Result<(String, u32)>`**
- Reads `market_code` and `offset` from config_kv table
- Returns `("en-US", 0)` as default if not found

**`save_market_state_sync(market_code: &str, offset: u32) -> Result<()>`**
- Saves `market_code` and `offset` to config_kv table
- Upserts keys (insert or update)

**`increment_market_offset_sync() -> Result<()>`**
- Loads current offset
- Increments by 8
- Saves back to config_kv

## CLI REPL Redesign

### Initialization

```rust
pub fn run_cli_mode() -> Result<()> {
    // Get platform-specific config directory
    // Linux: ~/.config/bingtray/bingtray.db
    // Windows: %APPDATA%\bingtray\bingtray.db
    // macOS: ~/Library/Application Support/bingtray/bingtray.db
    let db_path = dirs::config_dir()
        .context("Could not determine config directory")?
        .join("bingtray")
        .join("bingtray.db");
    
    let viewmodel = ViewModel::new_sync(db_path)?;
    
    println!("Bingtray v{} - Bing Wallpaper Manager", env!("CARGO_PKG_VERSION"));
    
    loop {
        print_menu(&viewmodel);
        let choice = read_user_input()?;
        handle_choice(&viewmodel, choice)?;
    }
}
```

### Menu Structure

```
═══════════════════════════════════════════════════════════
MENU:
  0. Open Cache Directory
  1. Download & Set Next Wallpaper
  2. Keep Current Wallpaper
  3. Blacklist Current Wallpaper
  4. Set Random Favorite
  5. Exit
═══════════════════════════════════════════════════════════
```

### Menu Option Implementations

**Option 0: Open Cache Directory**
```rust
// Get cache directory path (platform-specific)
let cache_dir = dirs::cache_dir()
    .context("Could not determine cache directory")?
    .join("bingtray");

open::that(&cache_dir)?;
println!("✓ Opened cache directory");
```

**Option 1: Download & Set Next Wallpaper**
```rust
println!("⏳ Downloading and setting wallpaper...");
match viewmodel.download_and_set_next_wallpaper_sync() {
    Ok(result) => {
        println!("✓ Wallpaper set successfully!");
        println!("  Title: {}", result.title);
    }
    Err(e) => println!("✗ Error: {}", e),
}
```

**Option 2: Keep Current Wallpaper**
```rust
println!("⏳ Marking current wallpaper as favorite...");
match viewmodel.keep_current_wallpaper_sync() {
    Ok(Some(title)) => {
        println!("✓ Kept: \"{}\"", title);
    }
    Ok(None) => {
        println!("⚠ No matching wallpaper found in database");
        println!("  (Current wallpaper may not be from BingTray)");
    }
    Err(e) => println!("✗ Error: {}", e),
}
```

**Option 3: Blacklist Current Wallpaper**
```rust
println!("⏳ Blacklisting current wallpaper...");
match viewmodel.blacklist_current_wallpaper_sync() {
    Ok(Some(title)) => {
        println!("✓ Blacklisted: \"{}\"", title);
    }
    Ok(None) => {
        println!("⚠ No matching wallpaper found in database");
        println!("  (Current wallpaper may not be from BingTray)");
    }
    Err(e) => println!("✗ Error: {}", e),
}
```

**Option 4: Set Random Favorite**
```rust
println!("⏳ Setting random favorite wallpaper...");
match viewmodel.set_random_favorite_wallpaper_sync() {
    Ok(Some(title)) => {
        println!("✓ Set favorite: \"{}\"", title);
    }
    Ok(None) => {
        println!("⚠ No favorites available");
        println!("  Use option 2 to keep some wallpapers first.");
    }
    Err(e) => println!("✗ Error: {}", e),
}
```

**Option 5: Exit**
```rust
println!("\nGoodbye!");
break;
```

### Removed Complexity

Compared to the old file-based CLI:

- ✅ No `CalcBingimage` struct with complex state
- ✅ No file-based `blacklist.conf` reading/writing
- ✅ No file-based `market_state.conf` reading/writing
- ✅ No directory scanning for cached images
- ✅ No manual offset tracking in CLI code
- ✅ No `can_keep()` / `can_blacklist()` / `has_next_available()` state checks

## Desktop Wallpaper Matching Logic

### Challenge

Map the current desktop wallpaper file path to a database URL.

**Example scenario:**
- Desktop wallpaper: `/home/user/.cache/bingtray/OHR_CherryBlossom_EN-US1234567890.jpg`
- Database URL: `https://www.bing.com/th?id=OHR.CherryBlossom_EN-US1234567890_UHD.jpg&rf=...&pid=...`

### Matching Algorithm

```rust
fn match_wallpaper_to_database(
    conn: &mut SqliteConnection,
    wallpaper_path: &Path
) -> Result<Option<String>> {
    // 1. Extract filename stem (without extension)
    let filename = wallpaper_path
        .file_stem()
        .and_then(|s| s.to_str())
        .context("Invalid wallpaper path")?;
    // Example: "OHR_CherryBlossom_EN-US1234567890"
    
    // 2. Extract core identifier (remove OHR_ prefix)
    let core_id = filename
        .strip_prefix("OHR_")
        .unwrap_or(filename);
    // Example: "CherryBlossom_EN-US1234567890"
    
    // 3. Query database for URLs containing this identifier
    use crate::schema::bing_images;
    let pattern = format!("%{}%", core_id);
    let images: Vec<BingImage> = bing_images::table
        .filter(bing_images::url.like(pattern))
        .order(bing_images::fetched_at.desc())
        .load(conn)?;
    
    // 4. Return first match (most recent if multiple)
    Ok(images.first().map(|img| img.url.clone()))
}
```

### Edge Cases

**Multiple matches:**
- Pick the first one (most recent by `fetched_at`)
- Unlikely in practice (same image from different markets)

**No OHR prefix in filename:**
- Use full filename for matching
- Covers edge cases where filename format differs

**No match found:**
- Return `None`
- CLI shows: "⚠ No matching wallpaper found in database"
- Can happen if:
  - User manually set a wallpaper outside BingTray
  - Database was cleared
  - Image was downloaded before database migration

**Special characters in path:**
- SQL LIKE handles most characters
- Escape `%` and `_` if they appear in filenames (rare)

## Download Workflow and On-Demand Image Handling

### Download & Set Next Wallpaper Workflow

**Step 1: Check for unprocessed images**

```sql
SELECT * FROM bing_images 
WHERE status = 'unprocessed' 
ORDER BY fetched_at DESC 
LIMIT 1
```

**Step 2a: If unprocessed image exists**
- Use that image URL
- Skip to Step 3

**Step 2b: If no unprocessed images**
1. Load market state from database:
   ```sql
   SELECT value FROM config_kv WHERE key = 'market_code';
   SELECT value FROM config_kv WHERE key = 'offset';
   ```
   - Default to ("en-US", 0) if not found
2. Call `api_bingimage::get_bing_images_manifest(market_code, 8, offset)`
3. Insert into database with status='unprocessed':
   ```rust
   for image in images {
       db::operations::upsert_image(conn, &NewBingImage {
           url: image.url,
           title: image.title,
           copyright: image.copyright,
           copyright_link: image.copyright_link,
           market_code: market_code.to_string(),
           status: "unprocessed",
           fetched_at: current_timestamp(),
       })?;
   }
   ```
4. Increment offset by 8 and save to config_kv:
   ```sql
   INSERT OR REPLACE INTO config_kv (key, value) VALUES ('offset', '8');
   ```
5. Pick first newly inserted image

**Step 3: Download image bytes on-demand**
```rust
let (tx, rx) = std::sync::mpsc::channel();
ehttp::fetch(ehttp::Request::get(&url), move |response| {
    let _ = tx.send(response);
});
let response = rx.recv_timeout(Duration::from_secs(30))?;
let bytes = response?.bytes;
```

**Step 4: Set wallpaper**
```rust
api_setwallpaper::set_wallpaper_from_bytes(&bytes)?;
```

**Step 5: Return result**
```rust
Ok(WallpaperSetResult {
    title: image.title.clone(),
    url: image.url.clone(),
})
```

### Caching Strategy

**On-demand downloads (no disk caching):**
- Unprocessed images: Downloaded each time wallpaper is set
- Blacklisted images: Never cached
- Database stores only metadata (URLs, titles, status)

**Optional caching for favorites (deferred to future enhancement):**
When `keep_current_wallpaper_sync()` is called:
1. Match desktop wallpaper to database URL
2. Update status to 'keepfavorite'
3. ~~Download and save to `~/.cache/bingtray/keepfavorite/<sanitized_title>.jpg`~~ (not implemented in v1)
   - Future enhancement: Allows viewing favorites offline
   - For v1: Favorites are re-downloaded on-demand like other images

**Cache directory structure (if favorite caching enabled):**
```
~/.cache/bingtray/
  └── keepfavorite/
      ├── CherryBlossom_EN-US.jpg
      ├── MountainLake_DE-DE.jpg
      └── ...
```

**No cache directories for:**
- `unprocessed/` (removed)
- `blacklisted/` (removed)

## Error Handling and Edge Cases

### Network Failures

**Auto-download fails during "Download & Set Next":**
```rust
Err(e) => {
    println!("✗ Failed to download images from Bing API: {}", e);
    // Offset NOT incremented (will retry same batch)
}
```
- User can retry by selecting option 1 again
- Database remains unchanged

### Database Errors

**Connection failure:**
```rust
let viewmodel = ViewModel::new_sync(db_path)?;
// If this fails, CLI exits with error message
```

**Query failures:**
```rust
Err(e) => {
    println!("✗ Database error: {}", e);
    // Return to menu, user can retry
}
```

### Wallpaper Setting Failures

**`api_setwallpaper::set_wallpaper_from_bytes()` fails:**
```rust
Err(e) => {
    println!("✗ Failed to set wallpaper: {}", e);
    // Image is still in database as unprocessed
    // User can try again or check system permissions
}
```

### Edge Cases

**1. Empty database on first run**
- Option 1 triggers auto-download of first 8 images
- Sets first one as wallpaper
- Offset becomes 8
- User sees: "✓ Wallpaper set successfully!"

**2. User blacklists/keeps all unprocessed images**
- Next "Download & Set Next" auto-downloads new batch
- Seamless continuation
- No manual intervention needed

**3. API exhaustion (offset too high, Bing returns duplicates)**
```rust
// During upsert, detect duplicates by URL
let existing = db::operations::get_image(conn, &image.url)?;
if existing.is_some() {
    log::warn!("Duplicate image detected: {}", image.url);
    duplicate_count += 1;
}

if duplicate_count == 8 {
    // All 8 images were duplicates
    println!("⚠ Bing API returned duplicate images.");
    println!("  You may have reached the historical limit for this market.");
    println!("  Try changing markets (feature not yet implemented).");
}
```
- Offset is NOT incremented (no new images added)
- User can exit and wait for new images, or manually change market in database

**4. Desktop wallpaper doesn't match database**
- `get_current_desktop_wallpaper_url_sync()` returns `None`
- Keep/Blacklist operations return `Ok(None)`
- User sees: "⚠ No matching wallpaper found in database"
- User can still use option 1 to set a BingTray wallpaper

**5. No favorites exist**
- `set_random_favorite_wallpaper_sync()` returns `Ok(None)`
- User sees: "⚠ No favorites available. Use option 2 to keep some first."

**6. Database migration from old file-based CLI**
- Old `blacklist.conf` and `market_state.conf` are ignored
- Database starts fresh
- User can manually blacklist images again if needed
- Market offset resets to 0 (will re-download some images)

## Testing Strategy

### Unit Tests for ViewModel Methods

**File:** `mobile/tests/viewmodel_cli_tests.rs`

**Test: `test_download_and_set_next_wallpaper_empty_db`**
- Setup: Empty database
- Action: Call `download_and_set_next_wallpaper_sync()`
- Mock: `api_bingimage::get_bing_images_manifest()` returns 8 images
- Mock: `api_setwallpaper::set_wallpaper_from_bytes()` returns Ok
- Assert: Database has 8 unprocessed images
- Assert: Market offset is 8
- Assert: Returns `WallpaperSetResult` with correct title

**Test: `test_download_and_set_next_wallpaper_existing_unprocessed`**
- Setup: Database with 3 unprocessed images
- Action: Call `download_and_set_next_wallpaper_sync()`
- Assert: Uses existing image (no API call)
- Assert: Returns correct title
- Assert: Market offset unchanged

**Test: `test_download_and_set_next_wallpaper_api_failure`**
- Setup: Empty database
- Mock: `api_bingimage::get_bing_images_manifest()` returns error
- Action: Call `download_and_set_next_wallpaper_sync()`
- Assert: Returns error
- Assert: Database unchanged
- Assert: Market offset unchanged (can retry)

**Test: `test_get_current_desktop_wallpaper_url_match`**
- Setup: Database with image URL containing "CherryBlossom_EN-US1234567890"
- Mock: `api_setwallpaper::get_current_wallpaper()` returns "/path/OHR_CherryBlossom_EN-US1234567890.jpg"
- Action: Call `get_current_desktop_wallpaper_url_sync()`
- Assert: Returns matched URL

**Test: `test_get_current_desktop_wallpaper_url_no_match`**
- Setup: Database with unrelated images
- Mock: `api_setwallpaper::get_current_wallpaper()` returns "/path/unknown_wallpaper.jpg"
- Action: Call `get_current_desktop_wallpaper_url_sync()`
- Assert: Returns `None`

**Test: `test_keep_current_wallpaper_success`**
- Setup: Database with image, desktop wallpaper matches
- Mock: Desktop wallpaper match succeeds
- Action: Call `keep_current_wallpaper_sync()`
- Assert: Image status updated to 'keepfavorite'
- Assert: Returns image title

**Test: `test_keep_current_wallpaper_no_match`**
- Setup: Desktop wallpaper doesn't match database
- Action: Call `keep_current_wallpaper_sync()`
- Assert: Returns `None`
- Assert: Database unchanged

**Test: `test_blacklist_current_wallpaper_success`**
- Setup: Database with image, desktop wallpaper matches
- Action: Call `blacklist_current_wallpaper_sync()`
- Assert: Image status updated to 'blacklisted'
- Assert: Returns image title

**Test: `test_set_random_favorite_wallpaper_multiple_favorites`**
- Setup: Database with 5 favorite images
- Action: Call `set_random_favorite_wallpaper_sync()` 10 times
- Assert: Each call returns a valid favorite title
- Assert: At least 2 different titles returned (randomness check)

**Test: `test_set_random_favorite_wallpaper_no_favorites`**
- Setup: Database with only unprocessed images
- Action: Call `set_random_favorite_wallpaper_sync()`
- Assert: Returns `None`

**Test: `test_market_offset_increment`**
- Setup: Database with market_code="en-US", offset=0
- Action: Call `increment_market_offset_sync()`
- Assert: offset becomes 8
- Action: Call again
- Assert: offset becomes 16

### Integration Tests

**File:** `mobile/tests/cli_integration_tests.rs`

**Test: `test_full_cli_workflow`**
1. Initialize fresh database
2. Download & set next wallpaper (should download 8, set 1)
3. Keep current wallpaper (should mark as favorite)
4. Download & set next wallpaper (should use existing unprocessed)
5. Blacklist current wallpaper (should mark as blacklisted)
6. Set random favorite (should set the kept one)
7. Assert final database state: 1 favorite, 1 blacklisted, 6 unprocessed

**Test: `test_database_persistence`**
1. Create ViewModel, download images, close
2. Create new ViewModel with same db_path
3. Assert: Images still exist
4. Assert: Market offset persisted

**Test: `test_offset_persistence_across_sessions`**
1. Create ViewModel, download images (offset -> 8)
2. Drop ViewModel
3. Create new ViewModel
4. Assert: offset is still 8
5. Download more images
6. Assert: offset becomes 16

### Mock Strategy

**Mock `api_setwallpaper::get_current_wallpaper()`:**
```rust
#[cfg(test)]
pub fn get_current_wallpaper() -> Result<PathBuf> {
    Ok(PathBuf::from("/test/path/OHR_TestImage_EN-US.jpg"))
}
```

**Mock `api_setwallpaper::set_wallpaper_from_bytes()`:**
```rust
#[cfg(test)]
pub fn set_wallpaper_from_bytes(_bytes: &[u8]) -> Result<bool> {
    Ok(true)
}
```

**Use in-memory database for tests:**
```rust
let db_path = PathBuf::from(":memory:");
let viewmodel = ViewModel::new_sync(db_path)?;
```

Or use temporary file:
```rust
let temp_dir = tempfile::tempdir()?;
let db_path = temp_dir.path().join("test.db");
```

## Migration Plan

### Deprecation of File-Based Persistence

**Files to remove:**
- `~/.config/bingtray/blacklist.conf` (CLI-specific)
- `~/.config/bingtray/market_state.conf` (CLI-specific)

**Migration strategy:**
- No automatic migration (files ignored)
- Database starts fresh on first run
- User will need to re-blacklist images if desired
- Market offset resets to 0 (may re-download some images)

**Rationale:** File-based persistence was a temporary workaround. Clean slate is acceptable for CLI users.

### Code Removal

**Remove from `calc_bingimage.rs`:**
- `read_blacklist()` function
- `write_blacklist()` function
- `load_market_state()` function
- `save_market_state()` function
- All file I/O related to blacklist.conf and market_state.conf

**Deprecate (but keep for GUI compatibility):**
- `CalcBingimage` struct (still used by GUI, but not CLI)
- Other `CalcBingimage` methods (GUI may still use them)

**Note:** Full removal of `CalcBingimage` is out of scope. This design focuses only on CLI migration.

## Implementation Summary

### Files to Modify

**1. `mobile/src/viewmodel/mod.rs`**
- Add new sync methods (feature-gated with `cli-only`)
- Add `WallpaperSetResult` struct

**2. `mobile/src/viewmodel/commands.rs`**
- Implement new sync method bodies
- Add desktop wallpaper matching logic
- Add auto-download workflow

**3. `mobile/src/cli.rs`**
- Rewrite to use ViewModel instead of CalcBingimage
- Simplify menu handling
- Remove file-based state management

**4. `mobile/src/calc_bingimage.rs`**
- Remove file-based persistence functions (read_blacklist, write_blacklist, load_market_state, save_market_state)
- Keep wallpaper utility functions if needed

**5. `mobile/src/db/operations.rs`**
- No changes needed (existing operations sufficient)
- Maybe add helper for config_kv operations if not exists

### Files to Create

**1. `mobile/tests/viewmodel_cli_tests.rs`**
- Unit tests for new ViewModel sync methods

**2. `mobile/tests/cli_integration_tests.rs`**
- Integration tests for full CLI workflows

### Dependencies

**Existing dependencies (no changes):**
- `diesel` - Database operations
- `ehttp` - HTTP fetching
- `anyhow` - Error handling
- Platform-specific wallpaper crates (via `api_setwallpaper`)

**Possible new dependency:**
- `rand` - For random favorite selection (may already be in workspace)

## Success Criteria

1. ✅ CLI compiles and runs without `CalcBingimage`
2. ✅ CLI uses database for all persistence (no `.conf` files)
3. ✅ "Download & Set Next Wallpaper" auto-downloads when database is empty
4. ✅ "Keep Current Wallpaper" correctly identifies and marks desktop wallpaper
5. ✅ "Blacklist Current Wallpaper" correctly identifies and marks desktop wallpaper
6. ✅ "Set Random Favorite" works when favorites exist
7. ✅ Market offset increments and persists across CLI sessions
8. ✅ All unit tests pass
9. ✅ All integration tests pass
10. ✅ Manual testing confirms all menu options work as expected

## Future Enhancements (Out of Scope)

- Multi-market rotation (automatic cycling through markets)
- Historical images feature (requires complex download workflow)
- CLI command-line arguments (non-interactive mode)
- Favorite caching to disk (currently optional)
- GUI migration to ViewModel (already done)
- Complete removal of `CalcBingimage` (requires GUI refactor)

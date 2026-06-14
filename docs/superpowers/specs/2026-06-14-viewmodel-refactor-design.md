# ViewModel Refactor Design: Smart Cache & Dual-Source Image Fetching

**Date:** 2026-06-14  
**Status:** Approved  
**Author:** Claude + User

## Summary

Refactor BingTray to use ViewModel-centric architecture with smart image caching and dual-source fetching (Bing API + GitHub archive). Remove deprecated `calc_bingimage.rs` (2856 lines) and consolidate all business logic in ViewModel layer. Add instant keep/blacklist response using pre-downloaded cache. Fix broken GUI filter control.

## Goals

1. **Delete deprecated code**: Remove `calc_bingimage.rs` entirely
2. **Smart caching**: Pre-download 3 images on startup, 5 more when idle
3. **Dual-source fetching**: Fetch from Bing API (en-US) + GitHub archive, deduplicate
4. **Instant keep/blacklist**: Use cached images for < 100ms wallpaper switching
5. **Fix GUI filter**: Replace broken material3 `select()` with `egui::ComboBox`
6. **7-day HTTP caching**: Use `ehttp_cache` for all network requests

## Architecture

### Module Structure

```
mobile/src/
├── viewmodel/
│   ├── mod.rs              # ViewModel struct, Command/Event enums (existing)
│   ├── commands.rs         # Command handlers (expand for cache-aware operations)
│   ├── background.rs       # Background thread (existing, no changes)
│   ├── cache_manager.rs    # NEW: Smart pre-download cache
│   └── sources.rs          # NEW: Dual-source fetching + deduplication
├── db/                      # Database layer (existing, no changes)
│   ├── mod.rs
│   ├── models.rs
│   └── operations.rs
├── api_bingimage.rs         # Keep for backward compatibility (minimal wrapper)
├── api_setwallpaper.rs      # Platform wallpaper setting (no changes)
├── bingtray.rs              # GUI (filter fix only)
├── cli.rs                   # CLI interface (no changes to interface)
├── tray.rs                  # Tray interface (no changes to interface)
└── calc_bingimage.rs        # DELETE THIS FILE (2856 lines removed)
```

### Data Flow

```
App Startup:
1. Initialize ViewModel
2. Cache Manager: Download 3 images immediately (blocking, ~2-5 seconds)
   - Check database for unprocessed images with cached_at IS NOT NULL
   - If < 3 cached: fetch from sources (Bing API + GitHub)
   - Download image bytes to ~/.cache/bingtray/images/
   - Update database: set cached_at timestamp
3. App becomes interactive

User Idle Detection (5 seconds no interaction):
4. Cache Manager: Download 5 more images in background
   - Fetch next batch from sources
   - Pre-download to cache directory
   - Update database

Keep/Blacklist Action:
5. User clicks "Keep Favorite" or "Blacklist" (CLI/Tray/GUI)
6. Update database status (instant, < 10ms)
7. Load next image from local cache (instant, no network wait)
8. Set wallpaper from cached bytes (< 100ms total)
9. Trigger cache refill if cached count < 3
```

### Cache Directory Structure

```
~/.cache/bingtray/
├── images/                         # Pre-downloaded full-resolution images
│   ├── OHR_Hnausapollur.jpg       
│   ├── OHR_LavenderSunset.jpg
│   └── OHR_MountainPeak.jpg
└── ehttp/                          # HTTP response cache (7 days TTL)
    └── [URL hashes]
```

### Database Schema Changes

Add column to `bing_images` table:

```sql
ALTER TABLE bing_images ADD COLUMN cached_at INTEGER;
```

**Purpose:** Track when image bytes were downloaded to local cache. NULL = not cached yet.

**Query for instant wallpaper switching:**
```sql
SELECT * FROM bing_images 
WHERE status = 'unprocessed' 
AND cached_at IS NOT NULL 
ORDER BY fetched_at DESC 
LIMIT 1;
```

## Component Design

### 1. Cache Manager (`viewmodel/cache_manager.rs`)

**Responsibilities:**
- Download and manage pre-cached images
- Monitor cache count, trigger refills
- Handle disk space checks
- Provide instant access to cached images

**Public API:**

```rust
pub struct CacheManager {
    cache_dir: PathBuf,
    db_path: PathBuf,
    sources: Arc<ImageSource>,
}

impl CacheManager {
    /// Initialize cache on app startup (blocking, download 3 images)
    pub fn initialize(&self) -> Result<usize>;
    
    /// Download 5 more images in background (non-blocking)
    pub fn refill_background(&self) -> Result<()>;
    
    /// Get next cached image for instant wallpaper setting
    pub fn get_next_cached_image(&self) -> Result<Option<CachedImage>>;
    
    /// Load image bytes from cache (instant, no network)
    pub fn load_cached_bytes(&self, url: &str) -> Result<Vec<u8>>;
    
    /// Check if refill needed (count < 3)
    pub fn needs_refill(&self) -> Result<bool>;
    
    /// Clean old cached files (eviction policy: keep last 50)
    pub fn evict_old_cache(&self) -> Result<usize>;
}

pub struct CachedImage {
    pub url: String,
    pub title: String,
    pub cached_path: PathBuf,
}
```

**Cache Refill Strategy:**

1. **Initial download (startup):**
   - Download 3 images synchronously
   - Retry each image up to 3 times on failure
   - Continue with partial success (1-2 images OK)
   
2. **Background refill (idle):**
   - Detect idle: no user interaction for 5 seconds
   - Download 5 images asynchronously
   - Don't block UI
   
3. **On-demand refill (after keep/blacklist):**
   - Check cached count after each wallpaper change
   - If < 3, trigger background download of 5 more

**Disk Space Management:**
- Check free space before downloading: require >= 100MB
- If low space: skip cache downloads, use on-demand fetching
- Evict old cached files: keep max 50 images (remove oldest `cached_at`)

### 2. Image Sources (`viewmodel/sources.rs`)

**Responsibilities:**
- Fetch images from Bing API (en-US only)
- Fetch images from GitHub archive
- Merge and deduplicate results
- Use 7-day HTTP caching for all requests

**Public API:**

```rust
pub struct ImageSource {
    bing_api: BingApiSource,
    github_archive: GitHubArchiveSource,
    ehttp_cache: Arc<EhttpCache>,
}

impl ImageSource {
    /// Fetch images from both sources, merge and deduplicate
    pub fn fetch_images(&self, count: usize) -> Result<Vec<BingImage>>;
    
    /// Fetch from Bing API only (en-US, offset=0, n=8)
    fn fetch_from_bing_api(&self, count: usize) -> Result<Vec<BingImage>>;
    
    /// Fetch from GitHub archive (cached 7 days)
    fn fetch_from_github_archive(&self) -> Result<Vec<BingImage>>;
    
    /// Deduplicate: identifier match OR title match
    fn deduplicate(&self, bing: Vec<BingImage>, github: Vec<BingImage>) -> Vec<BingImage>;
}
```

**Bing API Source:**

- **URL:** `https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=8&mkt=en-US`
- **Market:** en-US only (no multiple markets)
- **Offset:** Always 0 (latest 8 images)
- **Caching:** 7 days via `ehttp_cache`
- **Response format:** JSON with `images` array

**GitHub Archive Source:**

- **URL:** `https://github.com/v5tech/bing-wallpaper/blob/main/bing-wallpaper.md?plain=1`
- **Format:** Markdown table with columns: Date | Title | URL | Copyright
- **Parsing:** Extract rows, parse URL from markdown links
- **Caching:** 7 days via `ehttp_cache`
- **Example row:**
  ```
  | 2024-01-15 | Hnausapollur Crater | [Download](https://example.com/image.jpg) | © Photographer |
  ```

**Deduplication Strategy:**

Use **Option D**: Identifier match OR title match

```rust
fn is_duplicate(img1: &BingImage, img2: &BingImage) -> bool {
    // Extract identifier from URL (e.g., "OHR.Hnausapollur")
    // URL format: https://www.bing.com/th?id=OHR.Hnausapollur_EN-US2080493040_1920x1080.jpg
    let id1 = extract_identifier(&img1.url); // -> "OHR.Hnausapollur"
    let id2 = extract_identifier(&img2.url);
    
    // Match on identifier OR title (case-insensitive, trimmed)
    id1 == id2 || 
    img1.title.to_lowercase().trim() == img2.title.to_lowercase().trim()
}

fn extract_identifier(url: &str) -> Option<String> {
    // Extract from "th?id=OHR.Hnausapollur_EN-US..." -> "OHR.Hnausapollur"
    url.split("th?id=")
       .nth(1)?
       .split('_')
       .next()
       .map(|s| s.to_string())
}
```

**Merge Priority:**
- Bing API results take precedence over GitHub archive (newer data)
- If duplicate detected, keep Bing version, discard GitHub version
- Return merged list sorted by date (newest first)

**Edge Cases:**

1. **Same image, different resolutions:**
   - Keep highest resolution: prefer `_UHD.jpg` over `_1920x1080.jpg`
   
2. **URL identifier extraction fails:**
   - Fall back to title-only matching
   - If title also empty, treat as unique
   
3. **GitHub parse errors:**
   - Log error, continue with Bing API results only
   - Cache error state for 1 hour (don't retry immediately)

### 3. ViewModel Commands (`viewmodel/commands.rs`)

**Expand with cache-aware operations:**

```rust
/// Keep current wallpaper as favorite, set next wallpaper instantly
pub fn keep_current_wallpaper_instant_sync(
    conn: &mut SqliteConnection,
    cache_mgr: &CacheManager,
) -> Result<String> {
    // 1. Get current wallpaper URL
    let url = get_current_desktop_wallpaper_url_sync(conn)?
        .ok_or_else(|| anyhow::anyhow!("No current wallpaper"))?;
    
    // 2. Mark as favorite (instant database update)
    operations::update_image_status(conn, &url, ImageStatus::KeepFavorite)?;
    
    // 3. Get next cached image (pre-downloaded)
    let next_image = cache_mgr.get_next_cached_image()?
        .ok_or_else(|| anyhow::anyhow!("No cached images available"))?;
    
    // 4. Load from local cache (instant, no network)
    let bytes = cache_mgr.load_cached_bytes(&next_image.url)?;
    
    // 5. Set wallpaper
    crate::api_setwallpaper::set_wallpaper_from_bytes(&bytes)?;
    
    // 6. Update current wallpaper tracking
    operations::set_config(conn, "current_wallpaper_url", &next_image.url)?;
    
    // 7. Trigger background cache refill if count < 3
    if cache_mgr.needs_refill()? {
        std::thread::spawn(move || {
            let _ = cache_mgr.refill_background();
        });
    }
    
    Ok(next_image.title)
}

/// Blacklist current wallpaper, set next wallpaper instantly
pub fn blacklist_current_wallpaper_instant_sync(
    conn: &mut SqliteConnection,
    cache_mgr: &CacheManager,
) -> Result<String> {
    // Same as keep, but with ImageStatus::Blacklisted
    // ... (implementation identical except status update)
}
```

**Why:** These functions provide < 100ms wallpaper switching by using pre-cached images.

### 4. GUI Filter Fix (`bingtray.rs`)

**Problem:** material3 `select()` widget not rendering properly

**Solution:** Replace with standard `egui::ComboBox`

**Before (broken):**

```rust
let filter_select = select(carousel_filter)
    .variant(SelectVariant::Filled)
    .label(tr!("status-filter"))
    .option(0, tr!("status-all"))
    .option(1, tr!("status-keep-favorite"))
    .option(2, tr!("status-blacklisted"))
    .option(3, tr!("status-unprocessed"))
    .width(150.0);
ui.add(filter_select);
```

**After (working):**

```rust
ui.label(tr!("filter"));
egui::ComboBox::from_label(tr!("status-filter"))
    .selected_text(match carousel_filter {
        Some(0) => tr!("status-all"),
        Some(1) => tr!("status-keep-favorite"),
        Some(2) => tr!("status-blacklisted"),
        Some(3) => tr!("status-unprocessed"),
        _ => tr!("status-all"),
    })
    .show_ui(ui, |ui| {
        ui.selectable_value(carousel_filter, Some(0), tr!("status-all"));
        ui.selectable_value(carousel_filter, Some(1), tr!("status-keep-favorite"));
        ui.selectable_value(carousel_filter, Some(2), tr!("status-blacklisted"));
        ui.selectable_value(carousel_filter, Some(3), tr!("status-unprocessed"));
    });
```

**Filter logic (already correct in v0.0.14):**

```rust
let filtered_images: Vec<_> = carousel_images
    .iter()
    .enumerate()
    .filter(|(_, img)| {
        match carousel_filter {
            Some(0) => true, // All
            Some(1) => img.status.as_ref().map(|s| s == "keepfavorite").unwrap_or(false),
            Some(2) => img.status.as_ref().map(|s| s == "blacklisted").unwrap_or(false),
            Some(3) => img.status.as_ref().map(|s| s == "unprocessed").unwrap_or(false),
            _ => true,
        }
    })
    .collect();
```

## Error Handling

### Network Failures

**Initial download (3 images on startup):**
1. Retry each image up to 3 times with exponential backoff (1s, 2s, 4s)
2. If still failing, continue with partial success (1-2 images)
3. Log warning: "Downloaded X/3 images. Check network connection."
4. Schedule retry in background after 30 seconds

**Background download (5 images when idle):**
1. Retry failed images on next idle period
2. Don't block UI or show errors (silent retry)
3. Log warnings for monitoring

**No images available:**
- Gracefully degrade: allow app to run in "view-only" mode
- Show message: "Unable to download wallpapers. Check network connection."
- Keep retry button visible in GUI

### Disk Space Issues

**Before downloading:**
1. Check available space: require >= 100MB free
2. If < 100MB: skip cache downloads, use on-demand fetching
3. Show warning: "Low disk space. Image caching disabled."

**Cache eviction:**
- Keep max 50 cached images (configurable)
- Remove oldest by `cached_at` timestamp
- Clean on startup and after each download

### Source-Specific Errors

**Bing API failure:**
- Fall back to GitHub archive only
- Cache error state for 5 minutes (don't retry immediately)
- Log error for monitoring

**GitHub archive failure:**
- Continue with Bing API results only
- Cache error state for 1 hour
- Log error for monitoring

**Both sources fail:**
- Use existing database images (if any)
- Show error to user: "Unable to fetch new wallpapers"
- Retry on next user action

### Deduplication Edge Cases

**URL identifier extraction fails:**
- Fall back to title-only matching
- If title also unavailable, treat as unique image
- Log warning

**Database already contains image:**
- Skip insert (upsert with ON CONFLICT DO NOTHING on url column)
- Don't re-download bytes if `cached_at` IS NOT NULL
- Update metadata if changed (title, copyright)

## Testing Strategy

### Unit Tests

**`sources.rs` - Deduplication:**
- ✓ Extract identifier from standard Bing URL
- ✓ Extract identifier from GitHub URL
- ✓ Extract identifier from malformed URL (fallback to title)
- ✓ Title matching (case-insensitive, whitespace trimmed)
- ✓ Combined identifier + title matching
- ✓ Bing API priority over GitHub (keep Bing version)
- ✓ Resolution preference (keep UHD over 1920x1080)

**`cache_manager.rs` - Caching:**
- ✓ Initial download (3 images)
- ✓ Idle download (5 images)
- ✓ Refill trigger when count < 3
- ✓ Cache eviction (remove oldest, keep 50)
- ✓ Disk space check (skip if < 100MB)
- ✓ Load cached bytes (file not found error)

**`commands.rs` - Keep/Blacklist:**
- ✓ Instant status update in database
- ✓ Load next image from cache
- ✓ Trigger cache refill
- ✓ Error handling: no cached images available
- ✓ Error handling: wallpaper setting fails

### Integration Tests

**End-to-end CLI workflow:**
```rust
#[test]
fn test_cli_keep_workflow() {
    // 1. Initialize app (download 3 images)
    // 2. Verify cache directory contains 3 files
    // 3. Call keep_current_wallpaper_instant_sync()
    // 4. Verify next wallpaper set (< 100ms)
    // 5. Verify cache refill triggered
}
```

**Dual-source fetching:**
```rust
#[test]
fn test_dual_source_deduplication() {
    // Mock Bing API response (3 images)
    // Mock GitHub response (5 images, 2 duplicates)
    // Fetch from sources
    // Verify 6 unique images returned
    // Verify Bing versions kept for duplicates
}
```

**Filter control:**
```rust
#[test]
fn test_gui_filter() {
    // Load test images: 2 favorite, 3 blacklisted, 5 unprocessed
    // Apply filter = Some(1) (favorite)
    // Verify carousel shows 2 images
    // Apply filter = Some(0) (all)
    // Verify carousel shows 10 images
}
```

### Manual Testing Checklist

**GUI:**
- [ ] Filter dropdown renders correctly (ComboBox visible)
- [ ] Filter shows all 4 options (All/Favorite/Blacklisted/Unprocessed)
- [ ] Selecting filter updates carousel immediately
- [ ] Keep button sets next wallpaper instantly (< 100ms perceived)
- [ ] Blacklist button sets next wallpaper instantly
- [ ] Background download doesn't freeze UI

**CLI:**
- [ ] App starts in < 5 seconds (initial 3 image download)
- [ ] Keep/blacklist responds instantly
- [ ] Background refill visible in logs (5 images downloaded)

**Cross-platform:**
- [ ] Works offline after initial cache
- [ ] Cache persists across app restarts
- [ ] Low disk space handled gracefully

## Migration Plan

### Phase 1: Add New Modules (no breaking changes)

1. Create `viewmodel/sources.rs` with dual-source fetching
2. Create `viewmodel/cache_manager.rs` with smart caching
3. Add `cached_at` column to database (migration)
4. Add unit tests for new modules

### Phase 2: Update ViewModel Commands

1. Expand `viewmodel/commands.rs` with cache-aware functions
2. Keep old functions for backward compatibility
3. Add integration tests

### Phase 3: Update UI Layers

1. Fix GUI filter in `bingtray.rs` (ComboBox)
2. Update CLI to use new cache-aware commands
3. Update Tray to use new cache-aware commands

### Phase 4: Delete Deprecated Code

1. Remove `calc_bingimage.rs` (2856 lines)
2. Remove old command implementations
3. Update imports across codebase
4. Run full test suite

### Phase 5: Documentation & Release

1. Update CLAUDE.md with new architecture
2. Update README with new features
3. Create changelog entry
4. Tag release

## Success Metrics

1. **Code reduction:** Remove 2856 lines (calc_bingimage.rs)
2. **Performance:** Keep/blacklist response time < 100ms
3. **User experience:** No visible delay when switching wallpapers
4. **Network efficiency:** 7-day caching reduces API calls by ~80%
5. **Reliability:** App works offline with cached images
6. **Maintainability:** Single source of truth (ViewModel)

## Future Enhancements

**Not in this design, but noted for future:**

1. **Configurable cache size:** User setting for cache count (3-10 images)
2. **Multiple markets:** Re-enable if user requests (en-US, ja-JP, etc.)
3. **Smart prefetch:** ML-based prediction of user preferences
4. **Background sync:** Periodic refresh of GitHub archive
5. **Image compression:** Store smaller cached files for mobile

## Appendix: File Sizes

**Before:**
- `calc_bingimage.rs`: 2856 lines

**After:**
- `viewmodel/sources.rs`: ~400 lines (new)
- `viewmodel/cache_manager.rs`: ~300 lines (new)
- `viewmodel/commands.rs`: +200 lines (expanded)

**Net change:** -1956 lines (68% reduction in wallpaper logic)

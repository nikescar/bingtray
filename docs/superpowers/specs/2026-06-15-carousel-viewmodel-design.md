# Image Carousel with ViewModel Integration Design

**Date**: 2026-06-15  
**Author**: Claude (with user requirements)  
**Status**: Approved

## Overview

Implement a full-featured image carousel and main panel in the egui GUI using the new ViewModel architecture. The carousel will support filtering (All/Favorite/Blacklisted/Unprocessed), lazy pagination, and smart scroll position memory. The main panel will display selected images with crop functionality and wallpaper setting controls.

## Requirements Summary

From user request:
- Image carousel below top app bar
- Filter bar (All/Favorite/Blacklisted/Unprocessed)
- Carousel navigation (page indicators, prev/next)
- Lazy loading (20 images per page)
- Main image selector with:
  - Title and copyright info display
  - Favorite and blacklist toggles
  - Set wallpaper button
  - Set cropped wallpaper button
  - More info button
  - Interactive crop selector overlay

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Loading Strategy | Lazy loading (20 per page) | Scales to 1000+ images, fast startup |
| Filter Behavior | Remember scroll per filter | Best UX - each filter remembers position |
| Main Panel Loading | Cache-first with background refresh | Fast display + fresh images |
| Crop Persistence | Store in database as JSON | Reusable crops, polished UX |
| Page Size | 20 images | Aligns with existing fetch batch size |

---

## Architecture

### High-Level Structure

```
BingtrayApp (UI Layer)
├── TopAppBar (existing - menu, title, actions)
├── CarouselSection (NEW)
│   ├── FilterBar (All/Favorite/Blacklisted/Unprocessed)
│   ├── NavigationControls (page indicator, prev/next)
│   └── Carousel (egui-material3 carousel widget)
│       └── CarouselItems (20 images per page, lazy-loaded)
└── MainPanelSection (NEW)
    ├── ImageMetadata (title, copyright, link)
    ├── ActionBar (favorite toggle, blacklist toggle, buttons)
    ├── CropSelector (interactive rectangle overlay)
    └── MainImage (full-size display)

ViewModel (Data + Logic Layer)
├── Commands (UI → ViewModel)
│   ├── LoadCarouselPage { filter, page }
│   ├── LoadMainImage { url }
│   ├── ToggleFavorite { url }
│   ├── ToggleBlacklist { url }
│   ├── UpdateCropCoords { url, coords }
│   ├── SetWallpaper { url, crop_coords }
│   └── SetCroppedWallpaper { url, crop_coords }
└── Events (ViewModel → UI)
    ├── CarouselPageLoaded { images, total_count, page }
    ├── MainImageLoaded { image_data, cached }
    ├── MainImageRefreshed { image_data }
    ├── StatusToggled { url, new_status }
    ├── CropSaved { url }
    ├── WallpaperSet { success }
    └── Error { message }
```

### Event-Driven Architecture

**Pattern**: Async command/event model
- UI sends commands to ViewModel via `send_command()`
- ViewModel processes in background thread
- ViewModel emits events back to UI
- UI polls events on each frame via `poll_events()`
- UI updates state reactively based on events

**Benefits**:
- Non-blocking: database/network ops don't freeze UI
- Scalable: handles long operations (download, crop, wallpaper)
- Testable: ViewModel logic independent of UI
- Matches existing pattern in codebase

---

## Components & State Management

### UI State (BingtrayApp struct)

```rust
// Carousel state
carousel_filter: CarouselFilter,  // All/Favorite/Blacklisted/Unprocessed
carousel_scroll_positions: HashMap<CarouselFilter, f32>,  // Remember scroll per filter
carousel_pages: HashMap<(CarouselFilter, usize), Vec<CarouselImage>>,  // Cache loaded pages
carousel_current_page: usize,
carousel_total_count: Option<usize>,  // Total images for current filter
carousel_loading: bool,

// Main panel state
selected_image: Option<CarouselImage>,
main_image_loading: bool,
crop_coords: Option<CropCoords>,  // Current crop rectangle
show_crop_selector: bool,

// ViewModel (existing)
viewmodel: Option<ViewModel>,
```

### New Data Types

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum CarouselFilter {
    All,
    Favorite,
    Blacklisted,
    Unprocessed,
}

impl CarouselFilter {
    fn to_image_status(&self) -> Option<ImageStatus> {
        match self {
            All => None,
            Favorite => Some(ImageStatus::KeepFavorite),
            Blacklisted => Some(ImageStatus::Blacklisted),
            Unprocessed => Some(ImageStatus::Unprocessed),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct CropCoords {
    x: f32,      // 0.0-1.0, normalized to image width
    y: f32,      // 0.0-1.0, normalized to image height
    width: f32,  // 0.0-1.0, normalized
    height: f32, // 0.0-1.0, normalized
}
```

**Why normalized coords**: Works across different screen sizes and image resolutions.

### ViewModel Commands (extend existing enum)

```rust
pub enum ViewModelCommand {
    // Existing commands...
    DownloadImages { market_code: String },
    SetWallpaper { url: String },
    ToggleFavorite { url: String },
    BlacklistImage { url: String },
    GetImagesByStatus { status: ImageStatus },
    GetImagesByMarket { market_code: String, page: usize },
    RefreshDatabase,
    Shutdown,
    
    // NEW: Carousel operations
    LoadCarouselPage { 
        filter: Option<ImageStatus>,  // None = All
        page: usize,  // 0-indexed, 20 items per page
    },
    
    // NEW: Main panel operations
    LoadMainImage { url: String },
    UpdateCropCoords { url: String, coords: CropCoords },
}
```

### ViewModel Events (extend existing enum)

```rust
pub enum ViewModelEvent {
    // Existing events...
    DownloadProgress { current: usize, total: usize },
    DownloadComplete { count: usize },
    ImagesLoaded { images: Vec<BingImage> },
    WallpaperSet { success: bool },
    StatusUpdated { url: String, status: ImageStatus },
    Error { message: String },
    
    // NEW: Carousel responses
    CarouselPageLoaded {
        page: usize,
        images: Vec<BingImage>,
        total_count: usize,
    },
    
    // NEW: Main panel responses
    MainImageLoaded {
        url: String,
        image_bytes: Vec<u8>,
        cached: bool,
    },
    MainImageRefreshed {
        url: String,
        image_bytes: Vec<u8>,
    },
    CropCoordsSaved {
        url: String,
    },
}
```

### Database Schema Extension

```sql
-- Add crop_coords column to bing_images table
ALTER TABLE bing_images ADD COLUMN crop_coords TEXT;
```

**Storage format**: JSON string
```json
{"x": 0.1, "y": 0.2, "width": 0.6, "height": 0.8}
```

**Rationale**: Flexible, human-readable, easy to query/update, compatible with serde.

---

## Data Flow & Interactions

### 1. Carousel Loading Flow

#### Initial Load

```
1. UI renders → poll_events() finds no carousel data
2. UI sends: LoadCarouselPage { filter: None (All), page: 0 }
3. ViewModel background thread:
   - Queries DB: 
     SELECT * FROM bing_images 
     ORDER BY fetched_at DESC 
     LIMIT 20 OFFSET 0
   - Counts total: 
     SELECT COUNT(*) FROM bing_images
   - Emits: CarouselPageLoaded { 
       page: 0, 
       images: [...], 
       total_count: 1898 
     }
4. UI receives event:
   - Stores in carousel_pages[(All, 0)] = images
   - Sets carousel_total_count = 1898
   - Renders carousel with 20 items
```

#### Pagination (User Scrolls)

```
1. UI detects scroll_offset approaching end of current items
2. If page N+1 not in carousel_pages:
   - Send: LoadCarouselPage { filter: current, page: N+1 }
   - Set carousel_loading = true
3. ViewModel queries next 20:
   SELECT * FROM bing_images 
   ORDER BY fetched_at DESC 
   LIMIT 20 OFFSET (page * 20)
4. UI receives event:
   - Stores in carousel_pages[(filter, N+1)]
   - Sets carousel_loading = false
   - Carousel automatically shows new items
```

**Prefetch strategy**: When user scrolls past 75% of current page, load next page.

#### Filter Change

```
1. User clicks "Favorite" filter button
2. UI:
   - Saves current scroll position:
     carousel_scroll_positions[All] = current_offset
   - Changes carousel_filter = Favorite
   - Restores saved position:
     scroll_offset = carousel_scroll_positions[Favorite] ?? 0.0
   - If carousel_pages[(Favorite, 0)] exists:
     → Render immediately from cache
   - Else:
     → Send LoadCarouselPage { 
         filter: Some(KeepFavorite), 
         page: 0 
       }
3. ViewModel queries filtered:
   SELECT * FROM bing_images 
   WHERE status = 'keepfavorite'
   ORDER BY fetched_at DESC 
   LIMIT 20
4. UI renders filtered carousel at saved scroll position
```

**Benefit**: Switching between filters feels instant if previously loaded.

### 2. Main Panel Interaction Flow

#### Image Selection

```
1. User clicks carousel item (e.g., index 5)
2. UI:
   - Sets selected_image = carousel_images[5]
   - Sends: LoadMainImage { url: clicked_image.url }
   - Sets main_image_loading = true
3. ViewModel:
   - Checks cache: load_cached_bytes(url)?
   - If cached:
     a. Emits: MainImageLoaded { 
          url, 
          image_bytes, 
          cached: true 
        }
     b. Spawns background task to fetch fresh from network
     c. If fresh differs from cache:
        → Emits: MainImageRefreshed { url, image_bytes }
   - If not cached:
     a. Downloads from network (with progress tracking)
     b. Saves to cache
     c. Emits: MainImageLoaded { 
          url, 
          image_bytes, 
          cached: false 
        }
4. UI:
   - Renders image in main panel
   - Queries DB for existing crop_coords
   - Shows crop selector overlay (if crop exists, restore it)
   - Sets main_image_loading = false
```

**Performance**: Cached images show instantly (<16ms), network fetch happens in background.

#### Toggle Favorite/Blacklist

```
1. User clicks favorite button (star icon)
2. UI:
   - Optimistically updates button state (instant feedback)
   - Sends: ToggleFavorite { url }
3. ViewModel background thread:
   - Reads current status from DB
   - Toggles: unprocessed ↔ keepfavorite
   - Updates DB:
     UPDATE bing_images 
     SET status = 'keepfavorite', updated_at = <timestamp>
     WHERE url = ?
   - Emits: StatusUpdated { url, new_status: KeepFavorite }
4. UI receives event:
   - Confirms optimistic update (or reverts if failed)
   - Updates carousel item icon (star appears)
   - If current filter = Blacklisted:
     → Item disappears from carousel (no longer matches filter)
```

**Optimistic UI**: User sees instant feedback, confirmed when event arrives.

#### Crop & Set Wallpaper

```
1. User drags crop rectangle corners in UI
2. UI stores crop_coords locally (real-time preview feedback)
3. User clicks "Set Cropped Wallpaper" button
4. UI sends: SetWallpaper { 
     url, 
     crop_coords: Some(coords) 
   }
5. ViewModel background thread:
   a. Saves crop to DB:
      UPDATE bing_images 
      SET crop_coords = '{"x":0.1,"y":0.2,"width":0.6,"height":0.8}'
      WHERE url = ?
   b. Loads image bytes from cache
   c. Crops image:
      - Denormalize coords: actual_x = x * image_width
      - Use image::imageops::crop() to extract rectangle
      - Resize to screen resolution if needed
   d. Calls: api_setwallpaper::set_wallpaper_from_bytes(cropped_bytes)
   e. Emits: WallpaperSet { success: true }
6. UI receives event:
   - Shows success notification
   - Optionally: Marks image with "last set" indicator
```

**Crop persistence**: Next time user selects same image, crop is restored.

---

## Error Handling & Edge Cases

### Error Scenarios & Recovery

#### 1. Database Query Failures

**Scenario**: SQLite connection lost or DB corrupted

**Recovery**:
- ViewModel catches `diesel::Error`
- Emits: `Error { message: "Database error: <details>" }`
- UI shows error banner at top of window
- User can retry via refresh button
- Falls back to empty carousel with retry option

**Code location**: `mobile/src/viewmodel/background.rs` - wrap all DB queries in error handling

#### 2. Network Download Failures

**Scenario**: Image URL returns 404 or network timeout

**Recovery**:
- ViewModel retries 3x with exponential backoff (1s, 2s, 4s)
- If all retries fail:
  - Emits: `Error { message: "Failed to load image: <url>" }`
  - If cached version exists: keep showing cached (stale is better than broken)
  - If no cache: show placeholder image with retry button
- User can manually skip to next image or retry

**Code location**: `mobile/src/viewmodel/commands.rs` - in `LoadMainImage` handler

#### 3. Out of Memory (Large Image)

**Scenario**: Trying to load 50MB+ image into memory

**Recovery**:
- Check file size before download (via HEAD request)
- If > 20MB:
  - Downsample during decode (use `image::io::Reader::with_limits`)
  - Target max resolution: 4K (3840x2160)
- If OOM still occurs:
  - Emits: `Error { message: "Image too large to display" }`
  - UI suggests "Set Wallpaper" button without preview
  - Wallpaper setting uses streaming decode (no full image in memory)

**Code location**: `mobile/src/viewmodel/commands.rs` - add size check in image loading

#### 4. Pagination Edge Cases

**Scenario A**: User at last page, scrolls right
- UI checks: `current_page * 20 >= total_count`
- Don't send `LoadCarouselPage` command
- Show "End of list" indicator in carousel

**Scenario B**: Filter changes while page loading
- UI tracks `last_requested_filter`
- When `CarouselPageLoaded` event arrives:
  - If event filter ≠ current filter: **ignore event**
  - Prevents late-arriving events from wrong filter polluting carousel

**Scenario C**: Empty filter result (e.g., no favorites yet)
- ViewModel emits: `CarouselPageLoaded { images: [], total_count: 0 }`
- UI shows centered message: "No favorite images yet"
- Button to clear filter back to "All"

**Code location**: `mobile/src/bingtray.rs` - carousel rendering logic

#### 5. Crop Coordinate Edge Cases

**Scenario A**: Crop extends beyond image bounds
- UI clamps `crop_coords` to [0.0, 1.0] range before sending
- ViewModel validates and clamps again (defense in depth)
- Invalid coords logged as warning

**Scenario B**: Crop too small (< 100px after denormalization)
- UI shows warning badge: "⚠ Crop may be pixelated"
- User can proceed anyway (their choice)
- Warning clears if crop enlarged

**Scenario C**: Invalid crop_coords JSON in DB
- ViewModel parses with `serde_json::from_str()`
- On parse error:
  - Falls back to `crop_coords = None` (full image)
  - Logs warning with URL for debugging
  - Continues normally

**Code location**: `mobile/src/viewmodel/commands.rs` - crop validation functions

#### 6. Concurrent Operations

**Scenario A**: User rapidly clicks carousel items
- UI debounces `LoadMainImage` commands (300ms delay)
- Only sends command for final selection
- Cancels in-flight downloads for old selections (if possible)

**Scenario B**: User sets wallpaper while crop loading
- ViewModel queues commands in order (FIFO)
- `SetWallpaper` always uses latest `crop_coords` from DB
- No race conditions (single background thread)

**Code location**: `mobile/src/bingtray.rs` - add debounce timer for image selection

### Validation Rules

| Field | Rule | Error Handling |
|-------|------|----------------|
| Page number | `page >= 0` | Clamp to 0 if negative |
| Crop x/y | `0.0 <= value <= 1.0` | Clamp to range |
| Crop width/height | `0.01 <= value <= 1.0` | Clamp to range, warn if < 0.05 |
| Image URLs | Must start with `https://www.bing.com/` or `https://cn.bing.com/` | Reject with error event |
| Filter values | Must map to valid `ImageStatus` or `None` | Default to `None` (All) |

---

## Testing Strategy

### Unit Tests

**File**: `mobile/tests/viewmodel_carousel_tests.rs` (new file)

```rust
#[test]
fn test_load_carousel_page_all_filter() {
    // Given: DB with 50 images
    // When: LoadCarouselPage { filter: None, page: 0 }
    // Then: Returns first 20 images ordered by fetched_at DESC
    // And: total_count = 50
}

#[test]
fn test_load_carousel_page_pagination() {
    // Given: DB with 50 images
    // When: Load page 0, then page 1, then page 2
    // Then: Page 0 = images 0-19, page 1 = 20-39, page 2 = 40-49
    // And: No overlaps, correct OFFSET applied
}

#[test]
fn test_load_carousel_page_filtered() {
    // Given: 10 favorites, 15 blacklisted, 25 unprocessed (50 total)
    // When: LoadCarouselPage { filter: Some(KeepFavorite), page: 0 }
    // Then: Returns 10 favorites
    // And: total_count = 10
}

#[test]
fn test_load_main_image_cached() {
    // Given: Image exists in cache (pre-populate)
    // When: LoadMainImage { url }
    // Then: Emits MainImageLoaded { cached: true } within 100ms
    // And: Eventually emits MainImageRefreshed if network version differs
}

#[test]
fn test_load_main_image_network() {
    // Given: Image NOT in cache, network available
    // When: LoadMainImage { url }
    // Then: Downloads from network
    // And: Emits MainImageLoaded { cached: false }
    // And: Saves to cache for next time
}

#[test]
fn test_toggle_favorite_updates_status() {
    // Given: Image with status "unprocessed"
    // When: ToggleFavorite { url }
    // Then: DB updated to "keepfavorite"
    // And: Emits StatusUpdated { url, new_status: KeepFavorite }
}

#[test]
fn test_toggle_favorite_reverses() {
    // Given: Image with status "keepfavorite"
    // When: ToggleFavorite { url }
    // Then: DB updated to "unprocessed"
    // And: Emits StatusUpdated { url, new_status: Unprocessed }
}

#[test]
fn test_crop_coords_persistence() {
    // Given: Image URL
    // When: UpdateCropCoords { url, coords }
    // Then: DB updated with JSON crop_coords
    // When: Query DB for same URL
    // Then: Crop coords match (within floating-point epsilon)
}

#[test]
fn test_crop_coords_clamping() {
    // Given: Invalid coords { x: 1.5, y: -0.2, width: 1.1, height: 0.5 }
    // When: UpdateCropCoords
    // Then: Clamped to { x: 1.0, y: 0.0, width: 1.0, height: 0.5 }
}

#[test]
fn test_set_wallpaper_with_crop() {
    // Given: 1000x600 image with crop { x: 0.2, y: 0.1, width: 0.6, height: 0.8 }
    // When: SetWallpaper { url, crop_coords }
    // Then: Cropped region = (200, 60) to (800, 540) pixels
    // And: Wallpaper set via api_setwallpaper::set_wallpaper_from_bytes
    // And: Emits WallpaperSet { success: true }
}

#[test]
fn test_set_wallpaper_without_crop() {
    // Given: Image with no crop_coords in DB
    // When: SetWallpaper { url, crop_coords: None }
    // Then: Full image used (no cropping)
    // And: Wallpaper set successfully
}
```

**File**: `mobile/tests/db_carousel_tests.rs` (new file)

```rust
#[test]
fn test_get_images_paginated() {
    // Insert 100 test images
    // Query page 0 (LIMIT 20 OFFSET 0)
    // Query page 1 (LIMIT 20 OFFSET 20)
    // Verify: No duplicates, correct order
}

#[test]
fn test_crop_coords_json_storage() {
    // Create BingImage with crop_coords JSON
    // Save to DB
    // Reload from DB
    // Verify: Deserialization matches original
}

#[test]
fn test_filter_performance_with_1000_images() {
    // Insert 1000 test images (mix of statuses)
    // Query filtered: WHERE status = 'keepfavorite' LIMIT 20
    // Measure time
    // Assert: Query completes in < 100ms
}

#[test]
fn test_crop_coords_null_handling() {
    // Insert image without crop_coords (NULL)
    // Query image
    // Verify: crop_coords = None (not error)
}
```

### Integration Tests

**File**: `mobile/tests/carousel_integration_tests.rs` (new file)

```rust
#[test]
fn test_full_carousel_load_flow() {
    // 1. Initialize ViewModel (async mode)
    // 2. Send LoadCarouselPage { filter: None, page: 0 }
    // 3. Poll events until CarouselPageLoaded (timeout 5s)
    // 4. Verify: images.len() == min(20, total_count)
    // 5. Send LoadCarouselPage { filter: None, page: 1 }
    // 6. Poll until next CarouselPageLoaded
    // 7. Verify: New images don't duplicate first page
}

#[test]
fn test_filter_switching() {
    // 1. Load All filter (page 0)
    // 2. Wait for CarouselPageLoaded
    // 3. Switch to Favorite filter (send new LoadCarouselPage)
    // 4. Wait for CarouselPageLoaded
    // 5. Verify: Images match filter (all have status = "keepfavorite")
    // 6. Switch back to All
    // 7. Verify: All images returned (no filter)
}

#[test]
fn test_image_selection_and_crop() {
    // 1. Load carousel page
    // 2. Send LoadMainImage for first image
    // 3. Wait for MainImageLoaded event
    // 4. Verify: image_bytes.len() > 0
    // 5. Send UpdateCropCoords with test coords
    // 6. Wait for CropCoordsSaved event
    // 7. Query DB directly to verify crop persisted
}

#[test]
fn test_wallpaper_setting_flow() {
    // 1. Load main image
    // 2. Set crop coords
    // 3. Send SetWallpaper command
    // 4. Wait for WallpaperSet { success: true } event
    // 5. Verify: No errors emitted
}

#[test]
fn test_rapid_filter_switching() {
    // 1. Send LoadCarouselPage { filter: All, page: 0 }
    // 2. Immediately send LoadCarouselPage { filter: Favorite, page: 0 }
    // 3. Immediately send LoadCarouselPage { filter: Blacklisted, page: 0 }
    // 4. Poll all events
    // 5. Verify: Last event matches last request (Blacklisted)
    // 6. Verify: No crashes or panics
}
```

### Manual Testing Checklist

**Carousel:**
- [ ] Load carousel with 1000+ images - scroll smoothly without jank
- [ ] Scroll to page boundary - next page loads automatically
- [ ] Scroll rapidly - pages load in order, no visual glitches
- [ ] Filter switch (All → Favorite) - scroll position remembered
- [ ] Filter switch back (Favorite → All) - returns to previous position
- [ ] Empty filter (e.g., no blacklisted) - shows "no images" message
- [ ] Network offline - cached carousel images still visible
- [ ] Carousel item click - main panel updates

**Main Panel:**
- [ ] Click carousel item - image loads instantly if cached
- [ ] Click uncached item - shows loading indicator, then image
- [ ] Background refresh - subtle update if image changed (rare case)
- [ ] Crop selector - drag corners smoothly, rectangle constrained to image
- [ ] Crop selector - corner handles visible and clickable
- [ ] Very small crop - warning appears
- [ ] Favorite toggle (unprocessed → favorite) - icon updates, carousel item updates
- [ ] Blacklist toggle - icon updates, item disappears if on filtered view
- [ ] Set wallpaper - applies correctly, shows success notification
- [ ] Set cropped wallpaper - crop applied, wallpaper looks correct
- [ ] More info button - opens copyright link in browser

**Edge Cases:**
- [ ] Rapid filter switching - no crashes or wrong images
- [ ] Rapid image clicking - debounces correctly, shows last clicked
- [ ] Very long image titles - wrap correctly in carousel items
- [ ] Scroll to end of last page - shows "End of list" indicator
- [ ] Database empty - shows empty state with helpful message
- [ ] Network timeout during image load - retries, shows error if fails
- [ ] Invalid crop coords in DB - falls back to full image gracefully
- [ ] Image > 20MB - downsamples, doesn't crash

**Performance:**
- [ ] Carousel renders at 60 FPS during smooth scroll
- [ ] Filter switch feels instant (<100ms to first render)
- [ ] Main image load from cache < 50ms
- [ ] Page load (20 images) completes in < 500ms
- [ ] No memory leaks after 100+ image selections

---

## Implementation Notes

### File Structure

```
mobile/src/
├── bingtray.rs              # MODIFY: Add carousel + main panel UI
├── viewmodel/
│   ├── mod.rs              # MODIFY: Add new command/event types
│   ├── background.rs       # MODIFY: Handle new commands
│   └── commands.rs         # MODIFY: Add carousel/crop sync functions
├── db/
│   ├── operations.rs       # MODIFY: Add crop_coords queries
│   └── mod.rs              # MODIFY: Run migration for crop_coords column
└── migrations/
    └── YYYY-MM-DD-add-crop-coords/
        ├── up.sql          # NEW: ALTER TABLE ADD COLUMN
        └── down.sql        # NEW: ALTER TABLE DROP COLUMN

mobile/tests/
├── viewmodel_carousel_tests.rs      # NEW: Unit tests for ViewModel
├── db_carousel_tests.rs             # NEW: Database tests
└── carousel_integration_tests.rs    # NEW: End-to-end tests
```

### Dependencies

**Already available**:
- `egui-material3` - carousel widget
- `serde_json` - JSON serialization for crop_coords
- `image` - image decoding and cropping
- `diesel` - database operations

**No new dependencies needed**

### Migration Plan

**Phase 1: Database & ViewModel** (backend work)
1. Create migration for crop_coords column
2. Extend ViewModelCommand/Event enums
3. Implement background handlers for new commands
4. Add sync helper functions in commands.rs
5. Write unit tests

**Phase 2: UI Components** (frontend work)
6. Add carousel state fields to BingtrayApp
7. Implement carousel rendering with filter bar
8. Implement main panel layout
9. Wire up command sending on user interactions
10. Wire up event polling and state updates

**Phase 3: Testing & Polish**
11. Write integration tests
12. Manual testing against checklist
13. Performance optimization if needed
14. Documentation

### Breaking Changes

**None**. This is additive:
- New column (crop_coords) has default NULL - existing rows unaffected
- New commands/events don't break existing code
- Old carousel code remains until new is verified working

### Rollback Plan

If issues found after deployment:
1. Database migration is reversible (down.sql drops column)
2. UI can fallback to old carousel implementation
3. ViewModel changes are backward compatible

---

## Success Criteria

**Functional**:
- [ ] Carousel loads and displays images
- [ ] All 4 filters work correctly
- [ ] Pagination loads next pages automatically
- [ ] Scroll position remembered per filter
- [ ] Main panel shows selected image
- [ ] Crop selector works with mouse drag
- [ ] Favorite/blacklist toggles update database
- [ ] Wallpaper setting works (normal + cropped)
- [ ] All manual tests pass

**Performance**:
- [ ] 60 FPS during carousel scroll
- [ ] < 100ms for filter switch (if cached)
- [ ] < 50ms for cached image load
- [ ] < 500ms for carousel page load from DB

**Code Quality**:
- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] No clippy warnings
- [ ] Code reviewed and approved

---

## Future Enhancements (Out of Scope)

These are explicitly NOT included in this design, but noted for future consideration:

- **Image search/filtering by text** - would need full-text search index
- **Bulk operations** - select multiple images, batch favorite/blacklist
- **Image comparison view** - side-by-side of two images
- **Custom sort orders** - by date, by title, by status
- **Keyboard shortcuts** - arrow keys to navigate carousel
- **Thumbnail caching** - pre-generate smaller thumbnails for faster carousel
- **Image metadata editing** - rename titles, edit copyright info
- **Export/share** - save image to disk, copy to clipboard

---

## Appendix: Reference Implementations

### Carousel Example

Reference: `reference/egui-material3/examples/stories/carousel_window.rs`

Key learnings:
- Use `carousel(&mut scroll_offset)` builder pattern
- Items added via `.item()` or `.item_text()`
- Snapping enabled via `.item_snapping(true)`
- Item extent controls spacing

### Original Implementation

Reference: `https://github.com/nikescar/bingtray/raw/refs/heads/main/mobile/src/calc_bingimage.rs` (deprecated)

Key learnings:
- Used `CalcBingimage` for data management (now replaced by ViewModel)
- Stored all images in memory (now using lazy pagination)
- Status icons used emoji (we'll use Material icons)
- Crop selector existed but coords not persisted (now will be)

---

## Glossary

| Term | Definition |
|------|------------|
| Carousel | Horizontal scrolling list of image thumbnails |
| Main Panel | Large image display area below carousel |
| Crop Coords | Rectangle defining wallpaper crop region (normalized 0.0-1.0) |
| Lazy Loading | Load data only when needed (on-demand pagination) |
| Filter | Show subset of images by status (All/Favorite/Blacklisted/Unprocessed) |
| Cache-first | Show cached data immediately, refresh in background |
| Optimistic UI | Update UI immediately, confirm with backend later |

---

**End of Design Spec**

# Bug Fix - Part 5: Fix GitHub Archive Pagination
**Date**: 2026-06-15

## Issue
After downloading the first 20 images, no new images were available even though the GitHub archive contains 1,898 images.

**Log Evidence**:
```
[INFO] Parsed 1898 images from GitHub archive
[INFO] Fetched 20 images from dual sources (Bing + GitHub)
[WARN] All fetched images already exist in database - no new images to add
[ERROR] Failed to set next wallpaper: No unprocessed images available
```

## Root Cause
The `ImageSource::fetch_images()` method always returned the **same first 20 images**:

1. **GitHub archive returns all 1898 images** - newest first, in chronological order
2. **No pagination logic** - always took first 20 after deduplication
3. **No filtering** - didn't skip already-downloaded images

### Flow Breakdown
```
fetch_images(20)
  ├─ Bing API: 8 newest images
  ├─ GitHub: 1898 images (all of them)
  ├─ Deduplicate: Remove Bing duplicates from GitHub
  └─ Take first 20 ← ALWAYS THE SAME 20
```

On second call:
```
fetch_images(20)
  └─ Returns same 20 images again
  └─ Caller filters them out → 0 new images!
```

## Solution

Modified `fetch_images()` to **filter at source** instead of at caller:

### Before
```rust
pub fn fetch_images(&self, count: usize) -> Result<Vec<BingImage>> {
    let merged = deduplicate(bing_images, github_images);
    Ok(merged.into_iter().take(count).collect())
    // ❌ Always returns first 20
}
```

### After
```rust
pub fn fetch_images(&self, count: usize, existing_urls: &[String]) -> Result<Vec<BingImage>> {
    let merged = deduplicate(bing_images, github_images);
    
    // Filter out already-existing URLs and return requested count
    let new_images: Vec<BingImage> = merged
        .into_iter()
        .filter(|img| !existing_urls.contains(&img.url))  // ✅ Skip existing
        .take(count)
        .collect();
    
    Ok(new_images)
}
```

## How It Works Now

**First call** (20 existing URLs in DB):
```
1. GitHub returns 1898 images: [Image1, Image2, ..., Image1898]
2. Deduplicate with Bing API: 1898 images
3. Filter out 20 existing URLs: [Image21, Image22, ..., Image1898]
4. Take 20: [Image21...Image40]
5. Insert into DB → now 40 URLs exist
```

**Second call** (40 existing URLs in DB):
```
1. GitHub returns 1898 images: [Image1, Image2, ..., Image1898]
2. Deduplicate with Bing API: 1898 images
3. Filter out 40 existing URLs: [Image41, Image42, ..., Image1898]
4. Take 20: [Image41...Image60]
5. Insert into DB → now 60 URLs exist
```

This continues until all 1,898 images are exhausted!

## Files Modified

### mobile/src/viewmodel/sources.rs
- Added `existing_urls` parameter to `fetch_images()`
- Filter images at source instead of letting caller handle it
- Added logging for filtered count

### mobile/src/viewmodel/commands.rs
- Updated `download_and_set_next_wallpaper_sync()` to pass existing URLs

### mobile/src/viewmodel/cache_manager.rs
- Updated `download_and_cache()` to pass existing URLs

## Testing

Run CLI and process more than 20 images:
```bash
cargo run --manifest-path mobile/Cargo.toml

# Keep/blacklist first 20 images
# When count drops below 7:
[INFO] Unprocessed count (6) < 7, downloading new page
[INFO] Found 20 existing URLs in database
[INFO] Parsed 1898 images from GitHub archive
[INFO] Returning 20 new images (filtered out existing URLs)
[INFO] Found 20 new images (filtered 0 duplicates)
[INFO] Inserted 20 new images into database

# Continue to 40+ images
# Verify new images keep coming from GitHub archive
```

## Impact
- ✅ All 1,898 GitHub archive images now accessible
- ✅ Pagination works correctly - skips already-downloaded images
- ✅ Can process hundreds/thousands of images without exhaustion
- ✅ No performance impact - filtering is fast (HashSet could optimize further if needed)

## Status
✅ GitHub archive pagination working
✅ Infinite browsing now truly infinite (1,898 images available)
✅ All three call sites updated (commands, cache_manager)

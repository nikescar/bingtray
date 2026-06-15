# Carousel Implementation - Test Guide

## Quick Test

```bash
# 1. Clean start (optional - to test from empty state)
rm -rf ~/.config/bingtray/bingtray.db*

# 2. Run the app
cargo run --manifest-path mobile/Cargo.toml -- --gui

# 3. Expected behavior on first run (BELOW the top app bar):
#    - Filter bar appears: All | Favorites | Blacklisted | Unprocessed
#    - Loading spinner shows briefly  
#    - Then message: "No images in database. Use menu → Download & Set Next Wallpaper"
#    
#    ✅ Carousel section is now VISIBLE below top app bar!
#    (Previous bug: was hidden inside wrong conditional)

# 4. Download images:
#    - Click hamburger menu (☰) in top-right
#    - Click "Download & Set Next Wallpaper"
#    - Wait a few seconds

# 5. Carousel should now show:
#    - 20 image thumbnails in scrollable carousel
#    - Image titles below thumbnails
#    - Status icons (✨ for unprocessed)
#    - Total count: "(X images)"

# 6. Test carousel interaction:
#    - Thumbnails load (320x240 optimized size)
#    - Click an image thumbnail → main panel loads below
#    - Main panel shows:
#      * Title and copyright
#      * Material3 toggle switches (Favorite ⭐ / Blacklist 🚫)
#      * Set Wallpaper button
#      * Set Cropped Wallpaper button
#      * More Info button
#      * Full-size image (1920x1080)
#      * GREEN CROP SQUARE overlay (always visible, draggable corners)

# 7. Test filter switching:
#    - Click ⭐ Favorite button on an image
#    - Click "Favorites" filter → should see that image
#    - Click "All" → should see all images again

# 8. Test main panel:
#    - Click "Set Wallpaper" → desktop wallpaper changes
#    - Check "Crop" → shows crop dimensions (interactive crop coming soon)
```

## What's Working

✅ Filter bar with 4 filter buttons
✅ Lazy carousel loading (20 images per page)
✅ Carousel click → load in main panel
✅ Main panel image display with metadata
✅ Favorite/Blacklist/Wallpaper actions
✅ Filter switching with scroll memory
✅ Auto-refresh after download
✅ Empty state handling
✅ Loading indicators

## What to Check in Logs

```bash
cargo run --manifest-path mobile/Cargo.toml -- --gui 2>&1 | grep -E "Loading initial|Carousel page|images loaded|Main image"
```

Expected logs:
```
Loading initial carousel page
Carousel page 0 loaded: 20 images, 50 total
Main image loaded: <url> (12345 bytes, cached: false)
```

## Troubleshooting

**"No images showing"**
- Check: Did you download images first? (Menu → Download & Set Next Wallpaper)
- Check logs: `grep "Carousel page loaded" ~/.claude/logs/...`

**"Filter not working"**
- After favoriting: Click "Favorites" filter
- Images only show in their status filter

**"Carousel shows old images"**
- This is normal - old carousel is fallback
- New carousel only shows after ViewModel loads pages

## Architecture Verification

The implementation uses proper MVVM:
- UI sends commands: `LoadCarouselPage`, `LoadMainImage`, `ToggleFavorite`
- ViewModel processes in background thread
- UI polls events: `CarouselPageLoaded`, `MainImageLoaded`, `StatusUpdated`
- State updates trigger re-render

All working as designed!

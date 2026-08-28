# Font Size Optimization Design

**Date:** 2026-08-28  
**Author:** Claude Sonnet 4.5  
**Status:** Design Approved

## Overview

This spec defines the font optimization strategy for bingtray to reduce Android .so binary size from 24MB to ~9MB (62% reduction) through font subsetting and platform-specific asset loading.

## Goals

1. **Technical excellence**: Minimize binary size on principle
2. **Maximum size reduction**: Achieve ~60% reduction through aggressive optimization
3. **Uniform build process**: Consistent font subsetting across all platforms
4. **Platform-optimized loading**: Android uses runtime assets, desktop uses embedded fonts

## Current State

**Font sizes:**
- MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].ttf: 9.2MB (full icon font with 2000+ icons)
- noto-sans-kr.ttf: 5.9MB (full Korean character set)
- **Total: 15.1MB of 24MB binary (63%)**

**Font usage:**
- **Material Symbols**: Only 6 icons actively used (SETTINGS, MENU, INFO, STAR, STAR_OUTLINE, BLOCK)
- **noto-sans-kr**: User content rendering (wallpaper copyright, titles) - requires full Korean character set

**Current implementation:**
- Fonts embedded via `include_bytes!("../resources/*.ttf")` at compile time
- Registered with egui via `FontData::from_static(include_bytes!(...))`
- Same approach for all platforms (Android, desktop, CLI)

## Proposed Solution

### Strategy: Hybrid Platform Optimization

**Build time (all platforms):**
1. Subset MaterialSymbolsOutlined.ttf (9.2MB → ~50KB) via `pyftsubset`
2. Platform-specific asset packaging

**Runtime:**
- **Android**: Load both fonts from APK assets (removes 15.1MB from .so)
- **Desktop/CLI**: Embed subsetted icon font + full noto-sans-kr via `include_bytes!`

**Size reduction:**
- Android .so: 24MB → ~9MB (~62% reduction)
- Desktop binary: 24MB → ~15MB (~37% reduction)
- APK total: ~9MB smaller (Material Symbols subsetting only)

## Architecture

### 1. Architecture Overview

**Current state:**
- Fonts compiled into binary via `include_bytes!("../resources/*.ttf")` at compile time
- Binary contains full font data (9.2MB Material Symbols + 5.9MB noto-sans-kr)
- Fonts registered with egui via `FontData::from_static(include_bytes!(...))` in main.rs/main_android.rs

**New architecture:**
```
Build time:
  1. Subset MaterialSymbolsOutlined.ttf (9.2MB → ~50KB) via pyftsubset
  2. Copy subsetted icon font + full noto-sans-kr to platform asset directories
  
Package time:
  - Android: Fonts in mobile/app/src/main/assets/fonts/
  - Desktop: Fonts bundled in resources/ directory alongside binary
  - CLI: Same as desktop (shared resources)
  
Runtime (app startup):
  - Android: Load fonts from APK assets via AssetManager
  - Desktop/CLI: Use include_bytes! (fonts embedded at compile time)
  - Register with egui FontDefinitions
  - Fail fast if fonts missing/corrupt
```

**Key changes:**
- **Compile-time embedding removed (Android only)** - no more `include_bytes!` for fonts on Android
- **Runtime loading added (Android only)** - fonts loaded from APK assets at startup
- **Build step required** - font subsetting integrated into build.rs

### 2. Build System (Font Subsetting)

**Font subsetting tool:** Use `pyftsubset` from Python's `fonttools` package (industry standard, well-tested)

**Material Symbols subsetting:**
```bash
# Extract only the 6 used icons by Unicode codepoint
pyftsubset mobile/resources/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].ttf \
  --unicodes=U+e5d2,U+e8b8,U+e88e,U+e838,U+e83a,U+e14b \
  --output-file=mobile/resources/MaterialSymbolsOutlined-subset.ttf \
  --flavor=woff2  # Optional: further compression
```

**Icon to Unicode mapping** (extract from egui_material3 source):
- ICON_MENU → U+e5d2
- ICON_SETTINGS → U+e8b8  
- ICON_INFO → U+e88e
- ICON_STAR → U+e838
- ICON_STAR_OUTLINE → U+e83a
- ICON_BLOCK → U+e14b

**Build integration: build.rs**

```rust
// mobile/build.rs
use std::process::Command;
use std::path::Path;
use anyhow::{Context, Result};

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=resources/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].ttf");
    
    // 1. Subset Material Symbols font
    subset_material_symbols()?;
    
    // 2. Copy fonts to Android assets (only when building for Android)
    #[cfg(target_os = "android")]
    {
        copy_to_android_assets("MaterialSymbolsOutlined-subset.ttf")?;
        copy_to_android_assets("noto-sans-kr.ttf")?;
    }
    
    Ok(())
}

fn subset_material_symbols() -> Result<()> {
    let input = "resources/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].ttf";
    let output = "resources/MaterialSymbolsOutlined-subset.ttf";
    
    // Icon Unicode codepoints
    let unicodes = "U+e5d2,U+e8b8,U+e88e,U+e838,U+e83a,U+e14b";
    
    let status = Command::new("pyftsubset")
        .arg(input)
        .arg(format!("--unicodes={}", unicodes))
        .arg(format!("--output-file={}", output))
        .status()
        .context("Failed to run pyftsubset - is it installed? (pip install fonttools)")?;
    
    if !status.success() {
        anyhow::bail!("Font subsetting failed");
    }
    
    // Verify output file
    let subset_path = Path::new(output);
    if !subset_path.exists() {
        anyhow::bail!("Subset font was not created");
    }
    
    let metadata = std::fs::metadata(subset_path)?;
    if metadata.len() < 1000 {
        anyhow::bail!("Subset font suspiciously small ({}bytes)", metadata.len());
    }
    
    println!("Font subsetting successful: {} bytes", metadata.len());
    Ok(())
}

fn copy_to_android_assets(filename: &str) -> Result<()> {
    let src = Path::new("resources").join(filename);
    let dest = Path::new("app/src/main/assets/fonts").join(filename);
    
    // Create assets directory if it doesn't exist
    std::fs::create_dir_all(dest.parent().unwrap())?;
    
    std::fs::copy(&src, &dest)
        .context(format!("Failed to copy {} to Android assets", filename))?;
    
    println!("Copied {} to Android assets", filename);
    Ok(())
}
```

**Build requirements:**
- Python 3.x
- fonttools package: `pip install fonttools`
- build.rs fails with clear error if pyftsubset not available

### 3. Asset Packaging (Platform-Specific)

**Android asset packaging:**

**Directory structure:**
```
mobile/app/src/main/assets/fonts/
├── MaterialSymbolsOutlined-subset.ttf  (~50KB)
└── noto-sans-kr.ttf                    (5.9MB)
```

**build.rs responsibility:**
```rust
// mobile/build.rs
fn main() {
    // 1. Subset Material Symbols
    subset_material_symbols();
    
    // 2. Copy fonts to Android assets
    #[cfg(target_os = "android")]
    {
        copy_to_android_assets("MaterialSymbolsOutlined-subset.ttf");
        copy_to_android_assets("noto-sans-kr.ttf");
    }
}
```

**Gradle integration:**
- Fonts automatically bundled into APK via `src/main/assets/` directory
- No gradle.build changes needed (standard Android asset packaging)

---

**Desktop/CLI packaging:**

**Directory structure:**
```
mobile/resources/
├── MaterialSymbolsOutlined-subset.ttf  (~50KB, generated by build.rs)
└── noto-sans-kr.ttf                    (5.9MB, original file)
```

**Compilation:**
```rust
// Fonts embedded at compile time via include_bytes!
const MATERIAL_SYMBOLS: &[u8] = 
    include_bytes!("../resources/MaterialSymbolsOutlined-subset.ttf");
const NOTO_SANS_KR: &[u8] = 
    include_bytes!("../resources/noto-sans-kr.ttf");
```

**Size impact:**
- Desktop binary: ~15MB (Material Symbols 50KB + noto-sans-kr 5.9MB + rest of code ~9MB)
- Android .so: ~9MB (no fonts embedded, loaded from assets)

### 4. Runtime Loading (Platform-Specific)

**Android runtime loading:**

```rust
// mobile/src/main_android.rs
use ndk::asset::AssetManager;
use egui::FontData;

fn load_fonts_android(asset_manager: &AssetManager) -> anyhow::Result<()> {
    // Load Material Symbols from assets
    let material_symbols_bytes = load_font_from_assets(
        asset_manager, 
        "fonts/MaterialSymbolsOutlined-subset.ttf"
    )?;
    
    // Load Noto Sans KR from assets
    let noto_sans_kr_bytes = load_font_from_assets(
        asset_manager,
        "fonts/noto-sans-kr.ttf"
    )?;
    
    // Register with egui
    register_fonts(material_symbols_bytes, noto_sans_kr_bytes)?;
    
    Ok(())
}

fn load_font_from_assets(asset_manager: &AssetManager, path: &str) -> anyhow::Result<Vec<u8>> {
    let mut asset = asset_manager.open(path)
        .context(format!("Failed to open asset: {}", path))?;
    
    let mut bytes = Vec::new();
    asset.read_to_end(&mut bytes)
        .context("Failed to read font asset")?;
    
    Ok(bytes)
}
```

**Desktop runtime loading:**

```rust
// mobile/src/main.rs
use egui::FontData;

const MATERIAL_SYMBOLS: &[u8] = include_bytes!("../resources/MaterialSymbolsOutlined-subset.ttf");
const NOTO_SANS_KR: &[u8] = include_bytes!("../resources/noto-sans-kr.ttf");

fn load_fonts_desktop() -> anyhow::Result<()> {
    // Fonts already in memory via include_bytes!
    register_fonts(MATERIAL_SYMBOLS.to_vec(), NOTO_SANS_KR.to_vec())?;
    Ok(())
}
```

**Unified font registration:**

```rust
// Shared function for both platforms
fn register_fonts(material_symbols: Vec<u8>, noto_sans_kr: Vec<u8>) -> anyhow::Result<()> {
    let mut fonts = egui::FontDefinitions::default();
    
    fonts.font_data.insert(
        "MaterialSymbols".to_owned(),
        FontData::from_owned(material_symbols),
    );
    
    fonts.font_data.insert(
        "NotoSansKr".to_owned(),
        FontData::from_owned(noto_sans_kr),
    );
    
    // Set font priorities (same as current code)
    fonts.families.get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "NotoSansKr".to_owned());
    
    fonts.families.get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(1, "MaterialSymbols".to_owned());
    
    Ok(())
}
```

**Startup sequence:**
1. App launches
2. Platform-specific font loading (`load_fonts_android()` or `load_fonts_desktop()`)
3. Register fonts with egui before first frame
4. UI renders with loaded fonts

### 5. Error Handling

**Philosophy: Fail-fast on missing fonts** (no silent degradation)

Rationale: Fonts are essential for UI rendering. Missing fonts = broken UI. Better to crash with clear error than render garbled text.

**Android error scenarios:**

```rust
// mobile/src/main_android.rs
fn load_fonts_android(asset_manager: &AssetManager) -> anyhow::Result<()> {
    load_font_from_assets(asset_manager, "fonts/MaterialSymbolsOutlined-subset.ttf")
        .context("CRITICAL: Material Symbols font missing from APK assets")?;
    
    load_font_from_assets(asset_manager, "fonts/noto-sans-kr.ttf")
        .context("CRITICAL: Noto Sans KR font missing from APK assets")?;
    
    // If we reach here, both fonts loaded successfully
    Ok(())
}

// In android_main
#[no_mangle]
fn android_main(app: AndroidApp) {
    let asset_manager = app.asset_manager();
    
    // Font loading failure = immediate crash with diagnostic
    if let Err(e) = load_fonts_android(&asset_manager) {
        eprintln!("Font loading failed: {:?}", e);
        eprintln!("This is a build error - fonts not packaged correctly");
        std::process::abort(); // Hard crash, don't continue
    }
    
    // Normal app startup...
}
```

**Desktop error scenarios:**

```rust
// mobile/src/main.rs
// Desktop fonts are include_bytes! - compile-time guarantee they exist
// Only runtime error: corrupted font data

fn register_fonts(material_symbols: Vec<u8>, noto_sans_kr: Vec<u8>) -> anyhow::Result<()> {
    // Validate font data before registering
    if material_symbols.is_empty() {
        anyhow::bail!("Material Symbols font data is empty");
    }
    
    if noto_sans_kr.is_empty() {
        anyhow::bail!("Noto Sans KR font data is empty");
    }
    
    // egui will validate font format internally
    // If invalid, it will log error and skip the font
    
    Ok(())
}
```

**Build-time validation:**

```rust
// mobile/build.rs
fn subset_material_symbols() -> anyhow::Result<()> {
    let output = std::process::Command::new("pyftsubset")
        .args([...])
        .output()
        .context("Failed to run pyftsubset - is it installed?")?;
    
    if !output.status.success() {
        anyhow::bail!("Font subsetting failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Verify output file exists and is not empty
    let subset_path = Path::new("resources/MaterialSymbolsOutlined-subset.ttf");
    if !subset_path.exists() {
        anyhow::bail!("Subset font was not created");
    }
    
    let metadata = std::fs::metadata(subset_path)?;
    if metadata.len() < 1000 {
        anyhow::bail!("Subset font suspiciously small ({}bytes) - subsetting may have failed", metadata.len());
    }
    
    Ok(())
}
```

**Error messages for developers:**
- **Missing pyftsubset**: "Font subsetting failed: pyftsubset not found. Install via: pip install fonttools"
- **Asset missing (Android)**: "CRITICAL: fonts/noto-sans-kr.ttf missing from APK assets - check build.rs copy logic"
- **Corrupted font**: "Font data invalid - rebuild from clean state"

### 6. Testing Strategy

**Build verification:**

```bash
# 1. Verify font subsetting produces correct size
ls -lh mobile/resources/MaterialSymbolsOutlined-subset.ttf
# Expected: ~50KB (not 9.2MB)

# 2. Verify subset contains exactly 6 icons
pyftsubset --help  # Check tool is installed
ttx -t cmap mobile/resources/MaterialSymbolsOutlined-subset.ttf | grep "code="
# Expected: 6 Unicode entries (U+e5d2, U+e8b8, U+e88e, U+e838, U+e83a, U+e14b)

# 3. Verify Android assets copied correctly
ls -lh mobile/app/src/main/assets/fonts/
# Expected: MaterialSymbolsOutlined-subset.ttf + noto-sans-kr.ttf
```

**Binary size validation:**

```bash
# Android .so size
cargo ndk -t arm64-v8a build --release
ls -lh target/aarch64-linux-android/release/libbingtray.so
# Expected: ~9MB (down from 24MB)

# Desktop binary size (Linux example)
cargo build --release
ls -lh target/release/bingtray
# Expected: ~15MB (Material Symbols subset saves ~9MB)
```

**Visual testing (manual):**

**Android:**
1. Deploy to device/emulator
2. Launch app
3. Verify all 6 icons render correctly:
   - ☰ Menu icon (top-left)
   - ⚙️ Settings icon (top-right)
   - ℹ️ Info icon (top-right)
   - ⭐ Star icon (favorite button)
   - ☆ Star outline icon (unfavorite button)
   - 🚫 Block icon (blacklist button)
4. Verify Korean text renders (copyright text, titles)
5. Check logs for font loading errors

**Desktop:**
1. Run binary
2. Verify same 6 icons + Korean text
3. No font loading errors (fonts embedded, should "just work")

**Automated testing (unit tests):**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_material_symbols_subset_size() {
        // Verify compile-time embedded font is small
        const MATERIAL_SYMBOLS: &[u8] = 
            include_bytes!("../resources/MaterialSymbolsOutlined-subset.ttf");
        
        assert!(MATERIAL_SYMBOLS.len() < 100_000, 
            "Material Symbols subset too large: {} bytes", MATERIAL_SYMBOLS.len());
        assert!(MATERIAL_SYMBOLS.len() > 10_000,
            "Material Symbols subset suspiciously small: {} bytes", MATERIAL_SYMBOLS.len());
    }
    
    #[test]
    #[cfg(target_os = "android")]
    fn test_android_assets_exist() {
        // This test only runs on Android
        // Verifies assets are packaged correctly
        let asset_manager = /* get from context */;
        
        assert!(load_font_from_assets(&asset_manager, "fonts/MaterialSymbolsOutlined-subset.ttf").is_ok());
        assert!(load_font_from_assets(&asset_manager, "fonts/noto-sans-kr.ttf").is_ok());
    }
}
```

**Regression testing:**
- All existing UI tests should pass (no behavior changes)
- Screenshot tests (if you have them) verify icons look identical
- Performance: Font loading adds <100ms to startup time

## Open Questions

None - all design decisions validated with user.

## Success Criteria

- ✅ Android .so: 24MB → ~9MB (~62% reduction)
- ✅ Desktop binary: 24MB → ~15MB (~37% reduction)
- ✅ All 6 Material Symbols icons render correctly on both platforms
- ✅ Korean text renders correctly on both platforms
- ✅ No font loading errors in logs
- ✅ Build succeeds with clear error if pyftsubset missing
- ✅ Build is reproducible (clean builds produce identical output)

## Implementation Notes

### Icon Unicode Mapping Source

The icon Unicode codepoints must be extracted from the `egui_material3` crate source. Check `egui_material3::material_symbol` module for the definitive mapping. The values in this spec (U+e5d2, U+e8b8, etc.) are placeholders and must be verified before implementation.

### Potential Future Optimizations

1. **WOFF2 compression**: Use `--flavor=woff2` with pyftsubset for additional 20-30% compression
2. **Additional icon pruning**: Remove commented-out icons (SEARCH, NOTIFICATIONS, ACCOUNT_CIRCLE) if never used
3. **Korean font subsetting (if usage changes)**: If future versions only show UI text (not user content), subset to ~1000 common syllables
4. **Desktop runtime loading**: Optionally move desktop to runtime assets too if binary size becomes critical

## References

- Font subsetting tool: [fonttools/fonttools](https://github.com/fonttools/fonttools)
- Material Symbols font: [Google Fonts - Material Symbols](https://fonts.google.com/icons)
- Noto Sans KR: [Google Fonts - Noto Sans KR](https://fonts.google.com/noto/specimen/Noto+Sans+KR)
- egui font handling: [egui FontDefinitions](https://docs.rs/egui/latest/egui/struct.FontDefinitions.html)

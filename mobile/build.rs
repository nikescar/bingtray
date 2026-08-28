// build.rs

use std::error::Error;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    // Windows icon setup (existing)
    #[cfg(target_os = "windows")]
    {
        extern crate winres;
        let mut res = winres::WindowsResource::new();
        res.set_icon("resources/logo.ico");
        res.compile()?;
    }

    // Font subsetting: Material Symbols (6 icons only)
    subset_material_symbols()?;

    // Android: Copy fonts to assets directory for runtime loading
    #[cfg(target_os = "android")]
    copy_fonts_to_android_assets()?;

    Ok(())
}

fn subset_material_symbols() -> Result<(), Box<dyn Error>> {
    use std::collections::HashSet;
    use std::fs;

    let source_font = Path::new("resources/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].ttf");
    let subset_font = Path::new("resources/MaterialSymbolsOutlined_subset.ttf");

    // Load original font binary data
    let font_data = fs::read(source_font)?;

    // Define the 6 Material Icons we actually use
    let chars_to_keep: HashSet<char> = [
        '\u{E8B8}', // ICON_SETTINGS
        '\u{E5D2}', // ICON_MENU
        '\u{E88E}', // ICON_INFO
        '\u{E838}', // ICON_STAR
        '\u{F06F}', // ICON_STAR_OUTLINE
        '\u{E14B}', // ICON_BLOCK
    ]
    .iter()
    .copied()
    .collect();

    // Perform font subsetting using fontcull (no OpenType features needed)
    let subsetted_bytes = fontcull::subset_font_data(&font_data, &chars_to_keep, &[])?;

    // Write the subsetted font
    fs::write(subset_font, subsetted_bytes)?;

    // Rebuild if source font changes
    println!("cargo:rerun-if-changed={}", source_font.display());

    Ok(())
}

#[cfg(target_os = "android")]
fn copy_fonts_to_android_assets() -> Result<(), Box<dyn Error>> {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    // Use CARGO_MANIFEST_DIR to find project root, then navigate to Android assets
    // This ensures fonts are copied to app/src/main/assets/ (Gradle source directory)
    // not target/.../assets/ (Cargo build directory)
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let assets_dir = PathBuf::from(&manifest_dir)
        .join("app/src/main/assets");

    // Create assets directory if it doesn't exist
    fs::create_dir_all(&assets_dir)?;

    // Copy subset Material Symbols (50KB vs 9.6MB)
    let subset_font = Path::new("resources/MaterialSymbolsOutlined_subset.ttf");
    let dest_material = assets_dir.join("MaterialSymbolsOutlined_subset.ttf");
    fs::copy(subset_font, &dest_material)?;

    // Copy full Noto Sans KR (6.1MB - user content needs all Korean characters)
    let noto_font = Path::new("resources/noto-sans-kr.ttf");
    let dest_noto = assets_dir.join("noto-sans-kr.ttf");
    fs::copy(noto_font, &dest_noto)?;

    // Rebuild if fonts change
    println!("cargo:rerun-if-changed={}", subset_font.display());
    println!("cargo:rerun-if-changed={}", noto_font.display());

    Ok(())
}

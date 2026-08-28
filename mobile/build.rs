// build.rs

use std::error::Error;
use std::path::Path;
use std::process::Command;

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
    let source_font = Path::new("resources/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].ttf");
    let subset_font = Path::new("resources/MaterialSymbolsOutlined_subset.ttf");

    // Unicode codepoints for the 6 icons we actually use
    let codepoints = [
        "U+E8B8", // ICON_SETTINGS
        "U+E5D2", // ICON_MENU
        "U+E88E", // ICON_INFO
        "U+E838", // ICON_STAR
        "U+F06F", // ICON_STAR_OUTLINE
        "U+E14B", // ICON_BLOCK
    ];

    // Verify pyftsubset is available
    let pyftsubset_path = if cfg!(target_os = "openbsd") || cfg!(target_os = "freebsd") {
        "/usr/local/bin/pyftsubset"
    } else {
        "pyftsubset"
    };

    let check = Command::new(pyftsubset_path).arg("--help").output();

    if check.is_err() {
        eprintln!("ERROR: pyftsubset not found. Install fonttools:");
        eprintln!("  - Debian/Ubuntu: apt install python3-fonttools");
        eprintln!("  - macOS: pip3 install fonttools");
        eprintln!("  - OpenBSD: pkg_add py3-fonttools");
        return Err("pyftsubset not available".into());
    }

    // Run font subsetting
    let status = Command::new(pyftsubset_path)
        .arg(source_font)
        .arg(format!("--unicodes={}", codepoints.join(",")))
        .arg(format!("--output-file={}", subset_font.display()))
        .status()?;

    if !status.success() {
        return Err(format!("pyftsubset failed with status: {}", status).into());
    }

    // Rebuild if source font changes
    println!("cargo:rerun-if-changed={}", source_font.display());

    Ok(())
}

#[cfg(target_os = "android")]
fn copy_fonts_to_android_assets() -> Result<(), Box<dyn Error>> {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    let out_dir = env::var("OUT_DIR")?;
    let assets_dir = PathBuf::from(&out_dir)
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(|p| p.join("assets"))
        .ok_or("Failed to construct assets directory path")?;

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

use std::fs;
use std::path::Path;

#[test]
fn test_subset_font_exists() {
    let subset_font = Path::new("resources/MaterialSymbolsOutlined_subset.ttf");
    assert!(
        subset_font.exists(),
        "Subset font not found. Run `cargo build` to generate it via build.rs"
    );
}

#[test]
fn test_subset_font_size() {
    let subset_font = Path::new("resources/MaterialSymbolsOutlined_subset.ttf");

    if !subset_font.exists() {
        panic!("Subset font not found. Run `cargo build` to generate it.");
    }

    let metadata = fs::metadata(subset_font)
        .expect("Failed to read subset font metadata");

    let size_kb = metadata.len() / 1024;

    // Subset font should be < 100KB (6 icons only)
    // Currently ~12.8KB, allow some margin for future changes
    assert!(
        size_kb < 100,
        "Subset font too large: {}KB (expected < 100KB). Font may not be subsetted correctly.",
        size_kb
    );

    // Subset font should be > 5KB (sanity check - not empty)
    assert!(
        size_kb > 5,
        "Subset font suspiciously small: {}KB (expected > 5KB). File may be corrupted.",
        size_kb
    );
}

#[test]
fn test_original_font_exists() {
    let original_font = Path::new("resources/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].ttf");
    assert!(
        original_font.exists(),
        "Original Material Symbols font not found. Required for subsetting in build.rs"
    );
}

#[test]
fn test_original_font_larger_than_subset() {
    let original_font = Path::new("resources/MaterialSymbolsOutlined[FILL,GRAD,opsz,wght].ttf");
    let subset_font = Path::new("resources/MaterialSymbolsOutlined_subset.ttf");

    if !original_font.exists() || !subset_font.exists() {
        panic!("Fonts not found. Run `cargo build` first.");
    }

    let original_size = fs::metadata(original_font)
        .expect("Failed to read original font")
        .len();

    let subset_size = fs::metadata(subset_font)
        .expect("Failed to read subset font")
        .len();

    assert!(
        original_size > subset_size,
        "Original font ({} bytes) should be larger than subset font ({} bytes)",
        original_size,
        subset_size
    );

    // Verify significant size reduction (at least 99% reduction)
    let reduction_ratio = (subset_size as f64) / (original_size as f64);
    assert!(
        reduction_ratio < 0.01,
        "Font subsetting ineffective: subset is {:.1}% of original (expected < 1%)",
        reduction_ratio * 100.0
    );
}

#[test]
fn test_noto_sans_kr_exists() {
    let noto_font = Path::new("resources/noto-sans-kr.ttf");
    assert!(
        noto_font.exists(),
        "Noto Sans KR font not found"
    );
}

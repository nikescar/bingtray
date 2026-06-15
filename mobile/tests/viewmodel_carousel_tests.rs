use bingtray::viewmodel::CropCoords;

#[test]
fn test_crop_coords_clamping() {
    // Test: Out of range coords are clamped
    let invalid = CropCoords {
        x: 1.5,
        y: -0.2,
        width: 1.1,
        height: 0.005,  // Too small
    };

    let clamped = invalid.clamp();

    assert_eq!(clamped.x, 1.0);
    assert_eq!(clamped.y, 0.0);
    assert_eq!(clamped.width, 1.0);
    assert_eq!(clamped.height, 0.01);  // Min 1%
}

#[test]
fn test_crop_coords_json_serialization() {
    let coords = CropCoords {
        x: 0.1,
        y: 0.2,
        width: 0.6,
        height: 0.8,
    };

    // Serialize
    let json = coords.to_json().expect("serialize");
    assert!(json.contains("0.1"));
    assert!(json.contains("0.2"));

    // Deserialize
    let parsed = CropCoords::from_json(&json).expect("deserialize");
    assert_eq!(parsed, coords);
}

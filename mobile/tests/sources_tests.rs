use bingtray::viewmodel::sources::{ImageSource, BingApiSource};
use bingtray::BingImage;

#[test]
#[ignore] // Network test
fn test_fetch_from_bing_api() {
    let source = BingApiSource::new(None);
    let images = source.fetch(8).expect("Should fetch from Bing API");

    assert!(!images.is_empty(), "Should return images");
    assert!(images.len() <= 8, "Should respect count limit");

    // Verify URL format
    for img in images {
        assert!(img.url.starts_with("https://"), "URLs should be absolute");
        assert!(!img.title.is_empty(), "Title should not be empty");
    }
}

#[test]
fn test_extract_identifier_from_bing_url() {
    use bingtray::viewmodel::sources::extract_identifier;

    let url = "https://www.bing.com/th?id=OHR.Hnausapollur_EN-US2080493040_1920x1080.jpg&rf=...";
    let id = extract_identifier(url);
    assert_eq!(id, Some("OHR.Hnausapollur".to_string()));
}

#[test]
fn test_extract_identifier_no_match() {
    use bingtray::viewmodel::sources::extract_identifier;

    let url = "https://example.com/image.jpg";
    let id = extract_identifier(url);
    assert_eq!(id, None);
}

// Carousel progressive loading tests

#[derive(Clone)]
struct CarouselImage {
    pub title: String,
    pub copyright: String,
    pub thumbnail_url: String,
    pub full_url: String,
    pub base_url: String,
    pub status: Option<String>,
}

/// Progressive carousel loader that loads 8 images at a time
struct ProgressiveCarouselLoader {
    all_images: Vec<CarouselImage>,
    loaded_images: Vec<CarouselImage>,
    next_batch_index: usize,
    batch_size: usize,
}

impl ProgressiveCarouselLoader {
    /// Create new loader with batch size of 8
    fn new(all_images: Vec<CarouselImage>) -> Self {
        Self {
            all_images,
            loaded_images: Vec::new(),
            next_batch_index: 0,
            batch_size: 8,
        }
    }

    /// Load initial 8 images
    fn load_initial(&mut self) {
        let batch = self.all_images
            .iter()
            .skip(0)
            .take(self.batch_size)
            .cloned()
            .collect::<Vec<_>>();

        self.loaded_images.extend(batch);
        self.next_batch_index = self.batch_size;
    }

    /// Load next 8 images and append to existing
    fn load_next_batch(&mut self) -> usize {
        let batch = self.all_images
            .iter()
            .skip(self.next_batch_index)
            .take(self.batch_size)
            .cloned()
            .collect::<Vec<_>>();

        let loaded_count = batch.len();
        self.loaded_images.extend(batch);
        self.next_batch_index += loaded_count;
        loaded_count
    }

    /// Check if we should load more based on scroll position
    /// Returns true if we've scrolled past a threshold (5th, 13th, 21st, etc.)
    fn should_load_more(&self, scroll_center_index: usize) -> bool {
        // Don't load if we've already loaded all images
        if self.next_batch_index >= self.all_images.len() {
            return false;
        }

        // Thresholds: 5, 13, 21, 29, ...
        // At each threshold, check if we've already loaded the batch for it
        if scroll_center_index < 5 {
            return false;
        }

        // Check if we're at a threshold
        let offset_from_first_threshold = scroll_center_index.saturating_sub(5);
        let is_at_threshold = offset_from_first_threshold % 8 == 0;

        if !is_at_threshold {
            return false;
        }

        // Calculate how many items we should have loaded by this threshold
        // Threshold 5 = initial 8 + 1 batch (16 total)
        // Threshold 13 = initial 8 + 2 batches (24 total)
        // Threshold 21 = initial 8 + 3 batches (32 total)
        let batches_needed = 1 + (offset_from_first_threshold / 8);
        let items_needed = 8 + (batches_needed * 8);

        // Load more if we don't have enough items yet
        self.loaded_images.len() < items_needed
    }

    fn get_loaded_images(&self) -> &[CarouselImage] {
        &self.loaded_images
    }
}

#[test]
fn test_initial_load() {
    // Create 30 dummy images
    let all_images: Vec<CarouselImage> = (0..30)
        .map(|i| CarouselImage {
            title: format!("Image {}", i),
            copyright: String::new(),
            thumbnail_url: format!("https://example.com/{}.jpg", i),
            full_url: format!("https://example.com/{}.jpg", i),
            base_url: format!("/image_{}.jpg", i),
            status: Some("unprocessed".to_string()),
        })
        .collect();

    let mut loader = ProgressiveCarouselLoader::new(all_images);

    // Initial state - no images loaded
    assert_eq!(loader.get_loaded_images().len(), 0);

    // Load initial batch
    loader.load_initial();

    // Should have exactly 8 images
    assert_eq!(loader.get_loaded_images().len(), 8);
    assert_eq!(loader.get_loaded_images()[0].title, "Image 0");
    assert_eq!(loader.get_loaded_images()[7].title, "Image 7");
}

#[test]
fn test_append_on_scroll() {
    let all_images: Vec<CarouselImage> = (0..30)
        .map(|i| CarouselImage {
            title: format!("Image {}", i),
            copyright: String::new(),
            thumbnail_url: format!("https://example.com/{}.jpg", i),
            full_url: format!("https://example.com/{}.jpg", i),
            base_url: format!("/image_{}.jpg", i),
            status: Some("unprocessed".to_string()),
        })
        .collect();

    let mut loader = ProgressiveCarouselLoader::new(all_images);
    loader.load_initial();

    // Scroll to image 3 - should not trigger load
    assert!(!loader.should_load_more(3));

    // Scroll to image 4 - should not trigger load
    assert!(!loader.should_load_more(4));

    // Scroll to image 5 - should trigger load (5th image, first threshold)
    assert!(loader.should_load_more(5));

    // Load next batch
    let loaded = loader.load_next_batch();
    assert_eq!(loaded, 8);
    assert_eq!(loader.get_loaded_images().len(), 16);
    assert_eq!(loader.get_loaded_images()[8].title, "Image 8");
    assert_eq!(loader.get_loaded_images()[15].title, "Image 15");
}

#[test]
fn test_multiple_batches() {
    let all_images: Vec<CarouselImage> = (0..30)
        .map(|i| CarouselImage {
            title: format!("Image {}", i),
            copyright: String::new(),
            thumbnail_url: format!("https://example.com/{}.jpg", i),
            full_url: format!("https://example.com/{}.jpg", i),
            base_url: format!("/image_{}.jpg", i),
            status: Some("unprocessed".to_string()),
        })
        .collect();

    let mut loader = ProgressiveCarouselLoader::new(all_images);
    loader.load_initial();

    // First threshold - 5th image
    assert!(loader.should_load_more(5));
    loader.load_next_batch();
    assert_eq!(loader.get_loaded_images().len(), 16);

    // Not at threshold yet
    assert!(!loader.should_load_more(12));

    // Second threshold - 13th image
    assert!(loader.should_load_more(13));
    loader.load_next_batch();
    assert_eq!(loader.get_loaded_images().len(), 24);

    // Third threshold - 21st image
    assert!(loader.should_load_more(21));
    loader.load_next_batch();
    assert_eq!(loader.get_loaded_images().len(), 30); // Only 6 more available

    // No more images to load
    assert!(!loader.should_load_more(29));
}

#[test]
fn test_threshold_pattern() {
    let all_images: Vec<CarouselImage> = (0..100)
        .map(|i| CarouselImage {
            title: format!("Image {}", i),
            copyright: String::new(),
            thumbnail_url: format!("https://example.com/{}.jpg", i),
            full_url: format!("https://example.com/{}.jpg", i),
            base_url: format!("/image_{}.jpg", i),
            status: Some("unprocessed".to_string()),
        })
        .collect();

    let mut loader = ProgressiveCarouselLoader::new(all_images);
    loader.load_initial();

    // Thresholds should be at: 5, 13, 21, 29, 37, 45, ...
    let expected_thresholds = vec![5, 13, 21, 29, 37, 45, 53, 61, 69, 77, 85, 93];

    for threshold in expected_thresholds {
        // One before threshold - should not trigger
        if threshold > 0 {
            assert!(!loader.should_load_more(threshold - 1));
        }

        // At threshold - should trigger
        assert!(loader.should_load_more(threshold), "Failed at threshold {}", threshold);

        // Load the batch
        loader.load_next_batch();
    }
}

#[test]
fn test_no_duplicate_loads() {
    let all_images: Vec<CarouselImage> = (0..30)
        .map(|i| CarouselImage {
            title: format!("Image {}", i),
            copyright: String::new(),
            thumbnail_url: format!("https://example.com/{}.jpg", i),
            full_url: format!("https://example.com/{}.jpg", i),
            base_url: format!("/image_{}.jpg", i),
            status: Some("unprocessed".to_string()),
        })
        .collect();

    let mut loader = ProgressiveCarouselLoader::new(all_images);
    loader.load_initial();

    // Scroll to 5th image and load
    assert!(loader.should_load_more(5));
    loader.load_next_batch();
    assert_eq!(loader.get_loaded_images().len(), 16);

    // Scrolling around 5th image again should not trigger another load
    assert!(!loader.should_load_more(5));
    assert!(!loader.should_load_more(6));
    assert!(!loader.should_load_more(7));
}

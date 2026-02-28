use crossbeam_queue::SegQueue;
use eframe::egui;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Updates that can be queued for the shared store
#[derive(Clone, Debug)]
pub enum SharedStoreUpdate {
    /// Set the current image bytes
    CurrentImageBytes(Option<Vec<u8>>),
    /// Add image bytes to cache
    CacheImageBytes { url: String, bytes: Vec<u8> },
    /// Clear all caches
    ClearAll,
}

/// Shared store for Bing image data, accessible across the application
#[derive(Default)]
pub struct SharedStore {
    /// Current wallpaper image bytes (in-memory)
    pub current_image_bytes: Mutex<Option<Vec<u8>>>,

    /// Current wallpaper texture for egui rendering
    pub current_texture: Mutex<Option<egui::TextureHandle>>,

    /// Cache of downloaded image bytes by URL
    pub image_byte_cache: Mutex<HashMap<String, Vec<u8>>>,

    /// Cache of loaded textures by URL
    pub texture_cache: Mutex<HashMap<String, egui::TextureHandle>>,

    /// Queue for cross-thread updates
    pub update_queue: SegQueue<SharedStoreUpdate>,
}

impl SharedStore {
    pub fn new() -> Self {
        Self {
            current_image_bytes: Mutex::new(None),
            current_texture: Mutex::new(None),
            image_byte_cache: Mutex::new(HashMap::new()),
            texture_cache: Mutex::new(HashMap::new()),
            update_queue: SegQueue::new(),
        }
    }

    /// Get the global SharedStore instance
    pub fn global() -> &'static Arc<SharedStore> {
        static INSTANCE: OnceLock<Arc<SharedStore>> = OnceLock::new();
        INSTANCE.get_or_init(|| Arc::new(SharedStore::new()))
    }

    /// Process all pending updates from the queue
    pub fn process_updates(&self) {
        while let Some(update) = self.update_queue.pop() {
            match update {
                SharedStoreUpdate::CurrentImageBytes(bytes) => {
                    if let Ok(mut current) = self.current_image_bytes.lock() {
                        *current = bytes;
                    }
                }
                SharedStoreUpdate::CacheImageBytes { url, bytes } => {
                    if let Ok(mut cache) = self.image_byte_cache.lock() {
                        cache.insert(url, bytes);
                    }
                }
                SharedStoreUpdate::ClearAll => {
                    self.clear_all();
                }
            }
        }
    }

    // === Current image ===

    pub fn get_current_image_bytes(&self) -> Option<Vec<u8>> {
        self.current_image_bytes
            .lock()
            .ok()
            .and_then(|g| g.clone())
    }

    pub fn set_current_image_bytes(&self, bytes: Option<Vec<u8>>) {
        if let Ok(mut current) = self.current_image_bytes.lock() {
            *current = bytes;
        }
    }

    pub fn queue_current_image_bytes(&self, bytes: Option<Vec<u8>>) {
        self.update_queue
            .push(SharedStoreUpdate::CurrentImageBytes(bytes));
    }

    pub fn get_current_texture(&self) -> Option<egui::TextureHandle> {
        self.current_texture.lock().ok().and_then(|g| g.clone())
    }

    pub fn set_current_texture(&self, texture: Option<egui::TextureHandle>) {
        if let Ok(mut current) = self.current_texture.lock() {
            *current = texture;
        }
    }

    // === Image byte cache ===

    pub fn get_cached_image_bytes(&self, url: &str) -> Option<Vec<u8>> {
        self.image_byte_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(url).cloned())
    }

    pub fn cache_image_bytes(&self, url: String, bytes: Vec<u8>) {
        if let Ok(mut cache) = self.image_byte_cache.lock() {
            cache.insert(url, bytes);
        }
    }

    pub fn queue_cache_image_bytes(&self, url: String, bytes: Vec<u8>) {
        self.update_queue
            .push(SharedStoreUpdate::CacheImageBytes { url, bytes });
    }

    // === Texture cache ===

    pub fn get_texture(&self, url: &str) -> Option<egui::TextureHandle> {
        self.texture_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(url).cloned())
    }

    pub fn set_texture(&self, url: String, texture: egui::TextureHandle) {
        if let Ok(mut cache) = self.texture_cache.lock() {
            cache.insert(url, texture);
        }
    }

    // === Clear operations ===

    pub fn clear_all(&self) {
        if let Ok(mut current_bytes) = self.current_image_bytes.lock() {
            *current_bytes = None;
        }
        if let Ok(mut current_texture) = self.current_texture.lock() {
            *current_texture = None;
        }
        if let Ok(mut cache) = self.image_byte_cache.lock() {
            cache.clear();
        }
        if let Ok(mut cache) = self.texture_cache.lock() {
            cache.clear();
        }
    }

    pub fn clear_textures(&self) {
        if let Ok(mut current_texture) = self.current_texture.lock() {
            *current_texture = None;
        }
        if let Ok(mut cache) = self.texture_cache.lock() {
            cache.clear();
        }
    }
}

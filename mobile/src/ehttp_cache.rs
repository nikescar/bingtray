//! ehttp response cache to avoid re-fetching same URLs
//! This cache persists responses to disk and survives activity recreations

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};
use log::{info, debug, warn};

/// Global singleton cache instance
static GLOBAL_CACHE: OnceLock<EhttpCache> = OnceLock::new();

/// Cache entry with response data and metadata
#[derive(Clone, Serialize, Deserialize)]
struct CacheEntry {
    url: String,
    status: u16,
    status_text: String,
    bytes: Vec<u8>,
    headers: Vec<(String, String)>, // Changed from HashMap to Vec for serialization
    timestamp: u64,
    ttl_seconds: u64,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now > self.timestamp + self.ttl_seconds
    }

    fn to_response(&self) -> ehttp::Response {
        let headers = ehttp::Headers {
            headers: self.headers.clone(),
        };

        ehttp::Response {
            url: self.url.clone(),
            ok: self.status >= 200 && self.status < 300,
            status: self.status,
            status_text: self.status_text.clone(),
            bytes: self.bytes.clone(),
            headers,
        }
    }
}

/// ehttp cache manager
pub struct EhttpCache {
    /// In-memory cache
    memory_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Cache directory
    cache_dir: Option<PathBuf>,
    /// Default TTL in seconds
    default_ttl: u64,
}

impl EhttpCache {
    /// Create a new cache with the given cache directory
    /// Returns a singleton instance - all calls with the same parameters share the same cache
    pub fn new(cache_dir: Option<PathBuf>, default_ttl: u64) -> Self {
        GLOBAL_CACHE.get_or_init(|| {
            info!("🔧 CACHE INIT: Creating global EhttpCache singleton");
            info!("🔧 CACHE CONFIG: TTL={}s ({}h), Disk={:?}",
                 default_ttl, default_ttl / 3600,
                 cache_dir.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or("disabled".to_string()));

            let cache = Self {
                memory_cache: Arc::new(RwLock::new(HashMap::new())),
                cache_dir: cache_dir.clone(),
                default_ttl,
            };

            // Load cache from disk if directory exists
            if let Some(ref dir) = cache_dir {
                if dir.exists() {
                    cache.load_from_disk();
                } else if let Err(e) = std::fs::create_dir_all(dir) {
                    warn!("❌ CACHE INIT: Failed to create cache directory: {}", e);
                } else {
                    info!("✅ CACHE INIT: Created cache directory: {:?}", dir);
                }
            }

            cache
        }).clone()
    }

    /// Get a cached response for the given URL
    pub fn get(&self, url: &str) -> Option<ehttp::Response> {
        let cache = self.memory_cache.read().unwrap();

        if let Some(entry) = cache.get(url) {
            if !entry.is_expired() {
                let age_secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() - entry.timestamp;
                info!("🟢 CACHE HIT: {} (age: {}s, size: {} bytes, ttl: {}s)",
                     url, age_secs, entry.bytes.len(), entry.ttl_seconds);
                return Some(entry.to_response());
            } else {
                let age_secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() - entry.timestamp;
                info!("🟡 CACHE EXPIRED: {} (age: {}s > ttl: {}s)",
                     url, age_secs, entry.ttl_seconds);
            }
        } else {
            info!("🔴 CACHE MISS: {} (not in cache)", url);
        }

        None
    }

    /// Store a response in the cache
    pub fn put(&self, response: &ehttp::Response, ttl_seconds: Option<u64>) {
        let ttl = ttl_seconds.unwrap_or(self.default_ttl);

        let entry = CacheEntry {
            url: response.url.clone(),
            status: response.status,
            status_text: response.status_text.clone(),
            bytes: response.bytes.clone(),
            headers: response.headers.headers.clone(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ttl_seconds: ttl,
        };

        info!("💾 CACHE STORE: {} (status: {}, size: {} bytes, ttl: {}s)",
             response.url, response.status, response.bytes.len(), ttl);

        // Store in memory
        {
            let mut cache = self.memory_cache.write().unwrap();
            cache.insert(response.url.clone(), entry.clone());
        }

        // Persist to disk
        self.save_entry_to_disk(&entry);
    }

    /// Clear all expired entries from cache
    pub fn clear_expired(&self) {
        let mut cache = self.memory_cache.write().unwrap();
        let expired: Vec<String> = cache
            .iter()
            .filter(|(_, entry)| entry.is_expired())
            .map(|(url, _)| url.clone())
            .collect();

        for url in &expired {
            debug!("Removing expired cache entry: {}", url);
            cache.remove(url);
            self.remove_entry_from_disk(url);
        }

        if !expired.is_empty() {
            info!("Cleared {} expired cache entries", expired.len());
        }
    }

    /// Clear all cache entries
    pub fn clear_all(&self) {
        let mut cache = self.memory_cache.write().unwrap();
        cache.clear();

        if let Some(ref dir) = self.cache_dir {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if entry.path().extension().and_then(|s| s.to_str()) == Some("cache") {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
        }

        info!("Cleared all cache entries");
    }

    /// Load cache from disk
    fn load_from_disk(&self) {
        let Some(ref dir) = self.cache_dir else {
            info!("💾 DISK CACHE: Disabled (no cache directory)");
            return;
        };

        info!("💾 DISK CACHE: Loading from {:?}", dir);

        let Ok(entries) = std::fs::read_dir(dir) else {
            warn!("❌ DISK CACHE: Failed to read cache directory");
            return;
        };

        let mut loaded_count = 0;
        let mut expired_count = 0;
        let mut error_count = 0;

        for entry in entries.flatten() {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("cache") {
                if let Ok(data) = std::fs::read(entry.path()) {
                    if let Ok(cache_entry) = bincode::deserialize::<CacheEntry>(&data) {
                        if !cache_entry.is_expired() {
                            debug!("💾 DISK CACHE LOADED: {} ({} bytes)",
                                  cache_entry.url, cache_entry.bytes.len());
                            let mut cache = self.memory_cache.write().unwrap();
                            cache.insert(cache_entry.url.clone(), cache_entry);
                            loaded_count += 1;
                        } else {
                            debug!("🗑️  DISK CACHE EXPIRED: {} - removing file",
                                  cache_entry.url);
                            let _ = std::fs::remove_file(entry.path());
                            expired_count += 1;
                        }
                    } else {
                        error_count += 1;
                    }
                } else {
                    error_count += 1;
                }
            }
        }

        info!("💾 DISK CACHE: Loaded {} entries, removed {} expired, {} errors",
             loaded_count, expired_count, error_count);
    }

    /// Save a cache entry to disk
    fn save_entry_to_disk(&self, entry: &CacheEntry) {
        let Some(ref dir) = self.cache_dir else {
            debug!("💾 DISK CACHE: Disabled (no cache directory)");
            return;
        };

        // Create a safe filename from URL
        let filename = format!("{:x}.cache", md5::compute(entry.url.as_bytes()));
        let filepath = dir.join(filename);

        if let Ok(data) = bincode::serialize(entry) {
            if let Err(e) = std::fs::write(&filepath, &data) {
                warn!("❌ DISK CACHE SAVE FAILED: {} - {}", entry.url, e);
            } else {
                debug!("💾 DISK CACHE SAVED: {} -> {:?} ({} bytes)",
                      entry.url, filepath, data.len());
            }
        }
    }

    /// Remove a cache entry from disk
    fn remove_entry_from_disk(&self, url: &str) {
        let Some(ref dir) = self.cache_dir else {
            return;
        };

        let filename = format!("{:x}.cache", md5::compute(url.as_bytes()));
        let filepath = dir.join(filename);

        if filepath.exists() {
            let _ = std::fs::remove_file(&filepath);
        }
    }

    /// Fetch with cache support
    /// Returns immediately with cached response if available, otherwise fetches from network
    pub fn fetch<F>(&self, request: ehttp::Request, ttl_seconds: Option<u64>, callback: F)
    where
        F: FnOnce(Result<ehttp::Response, String>) + Send + 'static,
    {
        let url = request.url.clone();

        // Check cache first
        if let Some(cached_response) = self.get(&url) {
            info!("✅ RETURNING CACHED RESPONSE: {}", url);
            callback(Ok(cached_response));
            return;
        }

        // Not in cache, fetch from network
        info!("🌐 FETCHING FROM NETWORK: {}", url);
        let cache = self.clone();
        let url_for_log = url.clone();
        ehttp::fetch(request, move |response| {
            match response {
                Ok(ref resp) if resp.ok => {
                    info!("✅ NETWORK SUCCESS: {} (status: {}, size: {} bytes)",
                         url_for_log, resp.status, resp.bytes.len());
                    // Cache successful responses
                    cache.put(resp, ttl_seconds);
                    callback(Ok(resp.clone()));
                }
                Ok(resp) => {
                    // Don't cache failed responses
                    warn!("❌ NETWORK FAILED: {} (status: {}), NOT CACHING",
                         url_for_log, resp.status);
                    callback(Ok(resp));
                }
                Err(ref err) => {
                    warn!("❌ NETWORK ERROR: {} - {}", url_for_log, err);
                    callback(Err(err.clone()));
                }
            }
        });
    }
}

impl Clone for EhttpCache {
    fn clone(&self) -> Self {
        Self {
            memory_cache: Arc::clone(&self.memory_cache),
            cache_dir: self.cache_dir.clone(),
            default_ttl: self.default_ttl,
        }
    }
}

impl Default for EhttpCache {
    fn default() -> Self {
        Self::new(None, 3600) // 1 hour default TTL
    }
}

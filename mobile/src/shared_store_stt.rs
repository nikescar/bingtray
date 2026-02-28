use crate::adb::PackageFingerprint;
use crate::calc_androidpackage::AndroidPackageInfo;
use crate::calc_hybridanalysis::ScannerState as HaScannerState;
use crate::models::{ApkMirrorApp};
use crate::bingtray_app::UadNgLists;
use crossbeam_queue::SegQueue;
use eframe::egui;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Update types for the shared store queue
pub enum SharedStoreUpdate {
    /// Update installed packages
    InstalledPackages(Vec<PackageFingerprint>),
    /// Update UAD-NG lists
    UadNgLists(Option<UadNgLists>),
    /// Update stalkerware indicators

    /// Update cached APKMirror app
    CachedApkMirrorApp { pkg_id: String, app: ApkMirrorApp },
    /// Update cached Android Package app
    CachedAndroidPackageApp { pkg_id: String, app: AndroidPackageInfo },
}

/// Shared store for data that is accessed by both debloat and scan tabs
pub struct SharedStore {
    /// Installed packages list
    pub installed_packages: Mutex<Vec<PackageFingerprint>>,
    /// UAD-NG bloat lists
    pub uad_ng_lists: Mutex<Option<UadNgLists>>,
    /// Texture cache for APKMirror icons
    pub apkmirror_textures: Mutex<HashMap<String, egui::TextureHandle>>,
    /// Texture cache for Android Package icons
    pub android_package_textures: Mutex<HashMap<String, egui::TextureHandle>>,

    /// Cached APKMirror app info
    pub cached_apkmirror_apps: Mutex<HashMap<String, ApkMirrorApp>>,
    /// Cached Android Package app info
    pub cached_android_package_apps: Mutex<HashMap<String, AndroidPackageInfo>>,
    /// VirusTotal scanner state (scan tab only)
    pub vt_scanner_state: Mutex<Option<VtScannerState>>,
    /// Hybrid Analysis scanner state (scan tab only)
    pub ha_scanner_state: Mutex<Option<HaScannerState>>,
    /// Update queue for thread-safe updates from background threads
    pub update_queue: SegQueue<SharedStoreUpdate>,
}

/// Global shared store instance
static SHARED_STORE: OnceLock<Arc<SharedStore>> = OnceLock::new();

/// Get the global shared store instance
pub fn get_shared_store() -> Arc<SharedStore> {
    SHARED_STORE
        .get_or_init(|| Arc::new(SharedStore::new()))
        .clone()
}

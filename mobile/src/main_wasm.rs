//! WASM entry point for Bingtray
//!
//! Provides JavaScript-accessible handle for running Bingtray in the browser

use eframe::wasm_bindgen::{self, prelude::*};

/// Handle to the Bingtray web app from JavaScript
#[derive(Clone)]
#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,
}

#[wasm_bindgen]
impl WebHandle {
    /// Create a new WebHandle instance
    ///
    /// This installs panic hooks and initializes logging to browser console
    #[allow(clippy::new_without_default)]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        // Redirect log messages to console.log and friends
        eframe::WebLogger::init(log::LevelFilter::Info).ok();

        log::info!("Bingtray WASM v{} initializing", env!("CARGO_PKG_VERSION"));

        Self {
            runner: eframe::WebRunner::new(),
        }
    }

    /// Start the Bingtray app on the given canvas
    ///
    /// Call this once from JavaScript to start your app.
    ///
    /// # Arguments
    /// * `canvas_id` - The HTML canvas element ID (e.g., "bingtray_canvas")
    ///
    /// # Example JavaScript:
    /// ```js
    /// import init, { WebHandle } from './bingtray.js';
    ///
    /// async function start() {
    ///     await init();
    ///     const handle = new WebHandle();
    ///     await handle.start("bingtray_canvas");
    /// }
    /// ```
    #[wasm_bindgen]
    pub async fn start(&self, canvas_id: &str) -> Result<(), wasm_bindgen::JsValue> {
        log::info!("Starting Bingtray on canvas: {}", canvas_id);

        self.runner
            .start(
                canvas_id,
                eframe::WebOptions::default(),
                Box::new(|_cc| {
                    log::info!("Creating BingtrayApp instance");

                    // Initialize i18n with Auto language detection
                    if let Err(e) = bingtray::i18n::init_i18n("Auto") {
                        log::error!("Failed to initialize i18n: {}", e);
                    }

                    Ok(Box::<bingtray::BingtrayApp>::default())
                }),
            )
            .await
    }

    /// Destroy the app and free resources
    ///
    /// Call this from JavaScript when you want to clean up the app.
    #[wasm_bindgen]
    pub fn destroy(&self) {
        log::info!("Destroying Bingtray app");
        self.runner.destroy();
    }

    /// Check if the app has panicked
    ///
    /// JavaScript can use this to detect crashes:
    /// ```js
    /// if (handle.has_panicked()) {
    ///     console.error("App crashed!", handle.panic_message());
    /// }
    /// ```
    #[wasm_bindgen]
    pub fn has_panicked(&self) -> bool {
        self.runner.has_panicked()
    }

    /// Get the panic message if the app has crashed
    #[wasm_bindgen]
    pub fn panic_message(&self) -> Option<String> {
        self.runner.panic_summary().map(|s| s.message())
    }

    /// Get the panic callstack if the app has crashed
    #[wasm_bindgen]
    pub fn panic_callstack(&self) -> Option<String> {
        self.runner.panic_summary().map(|s| s.callstack())
    }
}

pub mod db;
pub mod request;
pub mod view;
pub mod storage;

pub use db::SqliteDb;
pub use request::HttpClient;
pub use view::WasmBingtrayApp;

// Re-export commonly used types for WASM
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn main() {
    // Set panic hook if available
    #[cfg(target_arch = "wasm32")]
    {
        // console_error_panic_hook::set_once();
        // Uncomment the above line if console_error_panic_hook feature is enabled
    }
    
    // Initialize logging (using web_sys console directly instead of wasm_logger)
    web_sys::console::log_1(&"WASM Bingtray module initialized".into());
}
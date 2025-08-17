use wasm_bindgen::prelude::*;
use web_sys::{window, Storage};

pub struct LocalStorage;

impl LocalStorage {
    fn get_storage() -> Result<Storage, JsValue> {
        window()
            .ok_or_else(|| JsValue::from_str("No window available"))?
            .local_storage()?
            .ok_or_else(|| JsValue::from_str("Local storage not available"))
    }

    pub fn set_item(key: &str, value: &str) -> Result<(), JsValue> {
        Self::get_storage()?.set_item(key, value)
    }

    pub fn get_item(key: &str) -> Result<Option<String>, JsValue> {
        Self::get_storage()?.get_item(key)
    }

    pub fn remove_item(key: &str) -> Result<(), JsValue> {
        Self::get_storage()?.remove_item(key)
    }

    pub fn clear() -> Result<(), JsValue> {
        Self::get_storage()?.clear()
    }

    pub fn save_market_codes(codes: &[String]) -> Result<(), JsValue> {
        #[cfg(feature = "serde")]
        {
            let json = serde_json::to_string(codes)
                .map_err(|e| JsValue::from_str(&format!("JSON serialize error: {}", e)))?;
            Self::set_item("market_codes", &json)
        }
        #[cfg(not(feature = "serde"))]
        {
            // Simple format: codes separated by newlines
            let simple_format = codes.join("\n");
            Self::set_item("market_codes", &simple_format)
        }
    }

    pub fn load_market_codes() -> Result<Vec<String>, JsValue> {
        match Self::get_item("market_codes")? {
            Some(data) => {
                #[cfg(feature = "serde")]
                {
                    serde_json::from_str(&data)
                        .map_err(|e| JsValue::from_str(&format!("JSON parse error: {}", e)))
                }
                #[cfg(not(feature = "serde"))]
                {
                    // Simple format: codes separated by newlines
                    Ok(data.lines().map(|s| s.to_string()).collect())
                }
            },
            None => Ok(Vec::new())
        }
    }

    pub fn save_blacklist(blacklist: &[String]) -> Result<(), JsValue> {
        #[cfg(feature = "serde")]
        {
            let json = serde_json::to_string(blacklist)
                .map_err(|e| JsValue::from_str(&format!("JSON serialize error: {}", e)))?;
            Self::set_item("blacklist", &json)
        }
        #[cfg(not(feature = "serde"))]
        {
            // Simple format: items separated by newlines
            let simple_format = blacklist.join("\n");
            Self::set_item("blacklist", &simple_format)
        }
    }

    pub fn load_blacklist() -> Result<Vec<String>, JsValue> {
        match Self::get_item("blacklist")? {
            Some(data) => {
                #[cfg(feature = "serde")]
                {
                    serde_json::from_str(&data)
                        .map_err(|e| JsValue::from_str(&format!("JSON parse error: {}", e)))
                }
                #[cfg(not(feature = "serde"))]
                {
                    // Simple format: items separated by newlines
                    Ok(data.lines().map(|s| s.to_string()).collect())
                }
            },
            None => Ok(Vec::new())
        }
    }

    pub fn save_user_preference(key: &str, value: &str) -> Result<(), JsValue> {
        let pref_key = format!("pref_{}", key);
        Self::set_item(&pref_key, value)
    }

    pub fn load_user_preference(key: &str) -> Result<Option<String>, JsValue> {
        let pref_key = format!("pref_{}", key);
        Self::get_item(&pref_key)
    }
}
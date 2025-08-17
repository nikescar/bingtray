pub mod posts;
pub mod schema;

use wasm_bindgen::prelude::*;
use sqlite_wasm_rs::{
    self as ffi,
    sahpool_vfs::{install as install_opfs_sahpool, OpfsSAHPoolCfg},
};
use std::ffi::{CStr, CString};

// Re-export commonly used types
pub use posts::{Posts, StoredPost};

#[wasm_bindgen]
pub struct SqliteDb {
    db: *mut ffi::sqlite3,
    posts: Posts, // Keep the in-memory posts for fallback
}

unsafe impl Send for SqliteDb {}
unsafe impl Sync for SqliteDb {}

#[wasm_bindgen]
impl SqliteDb {
    pub async fn new() -> Result<SqliteDb, JsValue> {
        web_sys::console::log_1(&"[SqliteDb] Initializing SQLite database...".into());
        
        let mut db = std::ptr::null_mut();
        
        // First try to open with memory VFS
        let ret = unsafe {
            ffi::sqlite3_open_v2(
                c"bingtray.db".as_ptr().cast(),
                &mut db as *mut _,
                ffi::SQLITE_OPEN_READWRITE | ffi::SQLITE_OPEN_CREATE,
                std::ptr::null()
            )
        };
        
        if ret != ffi::SQLITE_OK {
            return Err(JsValue::from_str(&format!("Failed to open SQLite database: {}", ret)));
        }
        
        web_sys::console::log_1(&"[SqliteDb] SQLite database opened successfully".into());
        
        // Try to install OPFS SAHPool VFS for persistence
        match install_opfs_sahpool(&OpfsSAHPoolCfg::default(), true).await {
            Ok(_opfs_util) => {
                web_sys::console::log_1(&"[SqliteDb] OPFS SAHPool VFS installed successfully".into());
                
                // Close the memory database and open persistent one
                unsafe {
                    ffi::sqlite3_close(db);
                }
                
                // Open with OPFS VFS
                let ret = unsafe {
                    ffi::sqlite3_open_v2(
                        c"bingtray-persistent.db".as_ptr().cast(),
                        &mut db as *mut _,
                        ffi::SQLITE_OPEN_READWRITE | ffi::SQLITE_OPEN_CREATE,
                        std::ptr::null()
                    )
                };
                
                if ret != ffi::SQLITE_OK {
                    web_sys::console::log_1(&"[SqliteDb] Failed to open persistent database, using memory".into());
                    // Fall back to memory database
                    let ret = unsafe {
                        ffi::sqlite3_open_v2(
                            c"mem.db".as_ptr().cast(),
                            &mut db as *mut _,
                            ffi::SQLITE_OPEN_READWRITE | ffi::SQLITE_OPEN_CREATE,
                            std::ptr::null()
                        )
                    };
                    if ret != ffi::SQLITE_OK {
                        return Err(JsValue::from_str(&format!("Failed to open memory database: {}", ret)));
                    }
                }
            }
            Err(e) => {
                web_sys::console::log_1(&format!("[SqliteDb] OPFS VFS installation failed: {:?}, using memory database", e).into());
                // Continue with memory database
            }
        }
        
        let posts = Posts::new();
        let sqlite_db = SqliteDb { db, posts };
        
        // Create tables
        sqlite_db.create_tables()?;
        
        Ok(sqlite_db)
    }

    // Synchronous constructor for immediate initialization
    pub fn new_sync(posts: Posts) -> SqliteDb {
        web_sys::console::log_1(&"[SqliteDb] Creating synchronous SQLite database (memory only)...".into());
        
        let mut db = std::ptr::null_mut();
        let ret = unsafe {
            ffi::sqlite3_open_v2(
                c"mem.db".as_ptr().cast(),
                &mut db as *mut _,
                ffi::SQLITE_OPEN_READWRITE | ffi::SQLITE_OPEN_CREATE,
                std::ptr::null()
            )
        };
        
        if ret != ffi::SQLITE_OK {
            web_sys::console::log_1(&format!("[SqliteDb] Failed to open sync database: {}, falling back to in-memory Posts", ret).into());
            return SqliteDb { db: std::ptr::null_mut(), posts };
        }
        
        let sqlite_db = SqliteDb { db, posts };
        
        // Create tables synchronously
        if let Err(e) = sqlite_db.create_tables() {
            web_sys::console::log_1(&format!("[SqliteDb] Failed to create tables: {:?}", e).into());
        }
        
        sqlite_db
    }

    fn create_tables(&self) -> Result<(), JsValue> {
        if self.db.is_null() {
            return Ok(()); // Skip if no database
        }

        let create_posts_table = c"
            CREATE TABLE IF NOT EXISTS posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                published INTEGER NOT NULL DEFAULT 0
            )
        ";

        let create_metadata_table = c"
            CREATE TABLE IF NOT EXISTS metadata (
                title TEXT PRIMARY KEY,
                copyright TEXT,
                url TEXT
            )
        ";

        let create_blacklist_table = c"
            CREATE TABLE IF NOT EXISTS blacklist (
                title TEXT PRIMARY KEY
            )
        ";

        let create_market_codes_table = c"
            CREATE TABLE IF NOT EXISTS market_codes (
                code TEXT PRIMARY KEY,
                timestamp INTEGER
            )
        ";

        // Execute table creation
        for sql in &[create_posts_table, create_metadata_table, create_blacklist_table, create_market_codes_table] {
            let ret = unsafe {
                ffi::sqlite3_exec(
                    self.db,
                    sql.as_ptr().cast(),
                    None,
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                )
            };

            if ret != ffi::SQLITE_OK {
                return Err(JsValue::from_str(&format!("Failed to create table: {}", ret)));
            }
        }

        web_sys::console::log_1(&"[SqliteDb] All tables created successfully".into());
        Ok(())
    }

    pub fn init_all_tables(&self) -> Result<(), JsValue> {
        self.create_tables()
    }

    pub fn load_market_codes(&self) -> Result<JsValue, JsValue> {
        if self.db.is_null() {
            return Ok(js_sys::Object::new().into());
        }

        // For now, return empty object - implement SQL query later
        Ok(js_sys::Object::new().into())
    }

    pub fn save_market_codes(&self, _codes: JsValue) -> Result<(), JsValue> {
        // TODO: Implement SQL INSERT for market codes
        Ok(())
    }

    pub fn save_metadata(&self, _title: &str, _copyright: &str, _url: &str) -> Result<(), JsValue> {
        // TODO: Implement SQL INSERT for metadata
        Ok(())
    }

    pub fn get_metadata(&self, _title: &str) -> Result<JsValue, JsValue> {
        // TODO: Implement SQL SELECT for metadata
        Ok(JsValue::NULL)
    }

    pub fn add_to_blacklist(&self, _title: &str) -> Result<(), JsValue> {
        // TODO: Implement SQL INSERT for blacklist
        Ok(())
    }

    pub fn is_blacklisted(&self, _title: &str) -> Result<bool, JsValue> {
        // TODO: Implement SQL SELECT for blacklist
        Ok(false)
    }

    pub fn save_historical_metadata(&self, _json: &str) -> Result<(), JsValue> {
        // TODO: Implement SQL INSERT for historical metadata
        Ok(())
    }

    pub fn get_historical_metadata_count(&self) -> Result<i32, JsValue> {
        // TODO: Implement SQL COUNT
        Ok(0)
    }

    pub fn get_total_pages(&self) -> Result<usize, JsValue> {
        // TODO: Implement pagination
        Ok(0)
    }

    pub fn get_historical_metadata_page(&self, _page: usize) -> Result<Vec<String>, JsValue> {
        // TODO: Implement SQL SELECT with LIMIT/OFFSET
        Ok(Vec::new())
    }

    // Posts functionality with actual SQLite
    pub fn create_post(&self, title: String, body: String, published: bool) -> usize {
        if self.db.is_null() {
            // Fall back to in-memory posts
            return self.posts.create_post(title, body, published).unwrap_or(0);
        }

        let sql = c"INSERT INTO posts (title, body, published) VALUES (?, ?, ?)";
        let mut stmt = std::ptr::null_mut();
        
        let ret = unsafe {
            ffi::sqlite3_prepare_v2(
                self.db,
                sql.as_ptr().cast(),
                -1,
                &mut stmt,
                std::ptr::null_mut()
            )
        };

        if ret != ffi::SQLITE_OK {
            web_sys::console::log_1(&format!("Failed to prepare insert statement: {}", ret).into());
            return 0;
        }

        let title_cstr = CString::new(title).unwrap();
        let body_cstr = CString::new(body).unwrap();

        unsafe {
            ffi::sqlite3_bind_text(stmt, 1, title_cstr.as_ptr(), -1, None);
            ffi::sqlite3_bind_text(stmt, 2, body_cstr.as_ptr(), -1, None);
            ffi::sqlite3_bind_int(stmt, 3, if published { 1 } else { 0 });

            let step_ret = ffi::sqlite3_step(stmt);
            ffi::sqlite3_finalize(stmt);

            if step_ret == ffi::SQLITE_DONE {
                1 // Success
            } else {
                web_sys::console::log_1(&format!("Failed to insert post: {}", step_ret).into());
                0
            }
        }
    }

    pub fn delete_post(&self, id: i32) -> usize {
        if self.db.is_null() {
            return self.posts.delete_post(id).unwrap_or(0);
        }

        let sql = c"DELETE FROM posts WHERE id = ?";
        let mut stmt = std::ptr::null_mut();
        
        let ret = unsafe {
            ffi::sqlite3_prepare_v2(
                self.db,
                sql.as_ptr().cast(),
                -1,
                &mut stmt,
                std::ptr::null_mut()
            )
        };

        if ret != ffi::SQLITE_OK {
            return 0;
        }

        unsafe {
            ffi::sqlite3_bind_int(stmt, 1, id);
            let step_ret = ffi::sqlite3_step(stmt);
            let changes = ffi::sqlite3_changes(self.db);
            ffi::sqlite3_finalize(stmt);

            if step_ret == ffi::SQLITE_DONE {
                changes as usize
            } else {
                0
            }
        }
    }

    pub fn clear_posts(&self) -> usize {
        if self.db.is_null() {
            return self.posts.clear().unwrap_or(0);
        }

        let sql = c"DELETE FROM posts";
        let ret = unsafe {
            ffi::sqlite3_exec(
                self.db,
                sql.as_ptr().cast(),
                None,
                std::ptr::null_mut(),
                std::ptr::null_mut()
            )
        };

        if ret == ffi::SQLITE_OK {
            unsafe { ffi::sqlite3_changes(self.db) as usize }
        } else {
            0
        }
    }

    pub fn list_posts(&self) -> Vec<JsValue> {
        if self.db.is_null() {
            return self.posts.list_posts().unwrap_or_else(|_| Vec::new());
        }

        let sql = c"SELECT id, title, body, published FROM posts ORDER BY id DESC";
        let mut stmt = std::ptr::null_mut();
        
        let ret = unsafe {
            ffi::sqlite3_prepare_v2(
                self.db,
                sql.as_ptr().cast(),
                -1,
                &mut stmt,
                std::ptr::null_mut()
            )
        };

        if ret != ffi::SQLITE_OK {
            web_sys::console::log_1(&format!("Failed to prepare select statement: {}", ret).into());
            return Vec::new();
        }

        let mut posts = Vec::new();

        unsafe {
            while ffi::sqlite3_step(stmt) == ffi::SQLITE_ROW {
                let id = ffi::sqlite3_column_int(stmt, 0);
                let title_ptr = ffi::sqlite3_column_text(stmt, 1);
                let body_ptr = ffi::sqlite3_column_text(stmt, 2);
                let published = ffi::sqlite3_column_int(stmt, 3) == 1;

                let title = if !title_ptr.is_null() {
                    CStr::from_ptr(title_ptr.cast()).to_string_lossy().into_owned()
                } else {
                    String::new()
                };

                let body = if !body_ptr.is_null() {
                    CStr::from_ptr(body_ptr.cast()).to_string_lossy().into_owned()
                } else {
                    String::new()
                };

                let obj = js_sys::Object::new();
                js_sys::Reflect::set(&obj, &"id".into(), &id.into()).unwrap();
                js_sys::Reflect::set(&obj, &"title".into(), &title.into()).unwrap();
                js_sys::Reflect::set(&obj, &"body".into(), &body.into()).unwrap();
                js_sys::Reflect::set(&obj, &"published".into(), &published.into()).unwrap();
                posts.push(obj.into());
            }
            
            ffi::sqlite3_finalize(stmt);
        }

        posts
    }

    pub fn list_tables(&self) -> Result<Vec<String>, JsValue> {
        if self.db.is_null() {
            return Ok(vec!["No SQLite database".to_string()]);
        }

        let sql = c"SELECT name FROM sqlite_master WHERE type='table' ORDER BY name";
        let mut stmt = std::ptr::null_mut();
        
        let ret = unsafe {
            ffi::sqlite3_prepare_v2(
                self.db,
                sql.as_ptr().cast(),
                -1,
                &mut stmt,
                std::ptr::null_mut()
            )
        };

        if ret != ffi::SQLITE_OK {
            return Err(JsValue::from_str(&format!("Failed to prepare list tables statement: {}", ret)));
        }

        let mut tables = Vec::new();
        unsafe {
            while ffi::sqlite3_step(stmt) == ffi::SQLITE_ROW {
                let name_ptr = ffi::sqlite3_column_text(stmt, 0);
                if !name_ptr.is_null() {
                    let name = std::ffi::CStr::from_ptr(name_ptr.cast())
                        .to_string_lossy()
                        .into_owned();
                    tables.push(name);
                }
            }
            ffi::sqlite3_finalize(stmt);
        }

        Ok(tables)
    }

    pub fn describe_tables(&self) -> Result<Vec<JsValue>, JsValue> {
        if self.db.is_null() {
            let obj = js_sys::Object::new();
            js_sys::Reflect::set(&obj, &"table".into(), &"No database".into()).unwrap();
            js_sys::Reflect::set(&obj, &"description".into(), &"SQLite not initialized".into()).unwrap();
            return Ok(vec![obj.into()]);
        }

        let tables = self.list_tables()?;
        let mut descriptions = Vec::new();

        for table in tables {
            let table_cstr = std::ffi::CString::new(format!("PRAGMA table_info({})", table))
                .map_err(|_| JsValue::from_str("Invalid table name"))?;
            
            let mut stmt = std::ptr::null_mut();
            let ret = unsafe {
                ffi::sqlite3_prepare_v2(
                    self.db,
                    table_cstr.as_ptr().cast(),
                    -1,
                    &mut stmt,
                    std::ptr::null_mut()
                )
            };

            if ret != ffi::SQLITE_OK {
                let obj = js_sys::Object::new();
                js_sys::Reflect::set(&obj, &"table".into(), &table.into()).unwrap();
                js_sys::Reflect::set(&obj, &"description".into(), &"Failed to get table info".into()).unwrap();
                descriptions.push(obj.into());
                continue;
            }

            let mut columns = Vec::new();
            unsafe {
                while ffi::sqlite3_step(stmt) == ffi::SQLITE_ROW {
                    let name_ptr = ffi::sqlite3_column_text(stmt, 1);
                    let type_ptr = ffi::sqlite3_column_text(stmt, 2);
                    
                    if !name_ptr.is_null() && !type_ptr.is_null() {
                        let name = std::ffi::CStr::from_ptr(name_ptr.cast())
                            .to_string_lossy()
                            .into_owned();
                        let col_type = std::ffi::CStr::from_ptr(type_ptr.cast())
                            .to_string_lossy()
                            .into_owned();
                        columns.push(format!("{} ({})", name, col_type));
                    }
                }
                ffi::sqlite3_finalize(stmt);
            }

            let description = if columns.is_empty() {
                "No columns found".to_string()
            } else {
                columns.join(", ")
            };
            
            let obj = js_sys::Object::new();
            js_sys::Reflect::set(&obj, &"table".into(), &table.into()).unwrap();
            js_sys::Reflect::set(&obj, &"description".into(), &description.into()).unwrap();
            descriptions.push(obj.into());
        }

        Ok(descriptions)
    }
}

impl Drop for SqliteDb {
    fn drop(&mut self) {
        if !self.db.is_null() {
            unsafe {
                ffi::sqlite3_close(self.db);
            }
            web_sys::console::log_1(&"[SqliteDb] Database closed".into());
        }
    }
}
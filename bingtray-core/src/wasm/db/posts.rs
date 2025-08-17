use wasm_bindgen::prelude::*;
use super::SqliteDb;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct StoredPost {
    pub id: i32,
    pub title: String,
    pub body: String,
    pub published: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct NewPost {
    title: String,
    body: String,
    published: bool
}

#[wasm_bindgen]
pub struct Posts {
    // SQLite database reference
    db: Rc<RefCell<Option<SqliteDb>>>,
    // In-memory storage for fallback when SQLite is not available
    posts: Rc<RefCell<Vec<StoredPost>>>,
    next_id: Rc<RefCell<i32>>,
}

#[wasm_bindgen]
impl Posts {
    pub fn new() -> Self {
        Self { 
            db: Rc::new(RefCell::new(None)),
            posts: Rc::new(RefCell::new(Vec::new())),
            next_id: Rc::new(RefCell::new(1)),
        }
    }

    pub async fn init_sqlite(&self) -> Result<(), JsValue> {
        web_sys::console::log_1(&"Initializing SQLite for posts...".into());
        
        let db = SqliteDb::new().await?;
        
        // Table creation is already handled by SqliteDb::new()
        // which calls create_tables() internally
        
        *self.db.borrow_mut() = Some(db);
        web_sys::console::log_1(&"Posts SQLite database initialized successfully".into());
        Ok(())
    }

    pub fn create_post(&self, title: String, body: String, published: bool) -> Result<usize, JsValue> {
        let db_ref = self.db.borrow();
        if let Some(db) = db_ref.as_ref() {
            // Use SQLite database
            let result = db.create_post(title, body, published);
            Ok(result)
        } else {
            // Fall back to in-memory storage
            let mut posts = self.posts.borrow_mut();
            let mut next_id = self.next_id.borrow_mut();
            
            let post = StoredPost {
                id: *next_id,
                title,
                body,
                published
            };
            
            posts.push(post);
            *next_id += 1;
            Ok(1)
        }
    }

    pub fn delete_post(&self, id: i32) -> Result<usize, JsValue> {
        let db_ref = self.db.borrow();
        if let Some(db) = db_ref.as_ref() {
            // Use SQLite database
            Ok(db.delete_post(id))
        } else {
            // Fall back to in-memory storage
            let mut posts = self.posts.borrow_mut();
            let original_len = posts.len();
            posts.retain(|post| post.id != id);
            Ok(original_len - posts.len())
        }
    }

    pub fn clear(&self) -> Result<usize, JsValue> {
        let db_ref = self.db.borrow();
        if let Some(db) = db_ref.as_ref() {
            // Use SQLite database
            Ok(db.clear_posts())
        } else {
            // Fall back to in-memory storage
            let mut posts = self.posts.borrow_mut();
            let count = posts.len();
            posts.clear();
            Ok(count)
        }
    }

    pub fn list_posts(&self) -> Result<Vec<JsValue>, JsValue> {
        let db_ref = self.db.borrow();
        if let Some(db) = db_ref.as_ref() {
            // Use SQLite database
            Ok(db.list_posts())
        } else {
            // Fall back to in-memory storage
            let posts = self.posts.borrow();
            let result = posts.iter()
                .map(|post| {
                    let obj = js_sys::Object::new();
                    js_sys::Reflect::set(&obj, &"id".into(), &post.id.into()).unwrap();
                    js_sys::Reflect::set(&obj, &"title".into(), &post.title.clone().into()).unwrap();
                    js_sys::Reflect::set(&obj, &"body".into(), &post.body.clone().into()).unwrap();
                    js_sys::Reflect::set(&obj, &"published".into(), &post.published.into()).unwrap();
                    obj.into()
                })
                .collect();
            Ok(result)
        }
    }
}
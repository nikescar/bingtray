use std::collections::{VecDeque, HashMap};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Semaphore;

// shared request queue
pub struct RequestQueue {
    queue: Arc<Mutex<VecDeque<RequestContext>>>,
    semaphore: Arc<Semaphore>,
}

impl RequestQueue {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }

    // Singleton instance
    fn instance() -> &'static Arc<RequestQueue> {
        static INSTANCE: OnceLock<Arc<RequestQueue>> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            Arc::new(RequestQueue {
                queue: Arc::new(Mutex::new(VecDeque::new())),
                semaphore: Arc::new(Semaphore::new(10)), // Default max concurrent
            })
        })
    }

    pub fn global() -> Arc<RequestQueue> {
        Arc::clone(Self::instance())
    }

    pub fn enqueue(&self, request: RequestContext) {
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(request);
    }

    pub fn dequeue(&self) -> Option<RequestContext> {
        let mut queue = self.queue.lock().unwrap();
        queue.pop_front()
    }

    pub fn len(&self) -> usize {
        let queue = self.queue.lock().unwrap();
        queue.len()
    }

    pub fn semaphore(&self) -> &Arc<Semaphore> {
        &self.semaphore
    }
}

// Request context structure
#[derive(Debug)]
pub struct RequestContext {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl RequestContext {
    pub fn new(url: String, method: String) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("User-Agent".to_string(), "BingTray/1.0".to_string());
        
        Self {
            url,
            method,
            headers,
            body: None,
        }
    }

    pub fn with_header(mut self, key: String, value: String) -> Self {
        self.headers.insert(key, value);
        self
    }

    pub fn with_body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }
}



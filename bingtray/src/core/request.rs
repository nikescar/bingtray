use std::collections::{VecDeque, HashMap};
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;
use web_sys::{Request as WebRequest, RequestInit, RequestMode, Response, Headers};

// Flyweight: Intrinsic state (shared across instances)
#[derive(Clone, Debug)]
pub struct RequestTemplate {
    method: String,
    default_headers: HashMap<String, String>,
}

impl RequestTemplate {
    pub fn new(method: String) -> Self {
        let mut default_headers = HashMap::new();
        default_headers.insert("Content-Type".to_string(), "application/json".to_string());
        default_headers.insert("User-Agent".to_string(), "BingTray/1.0".to_string());
        
        Self {
            method,
            default_headers,
        }
    }
}

// Context: Extrinsic state (unique per request)
#[derive(Debug)]
pub struct RequestContext {
    url: String,
    custom_headers: HashMap<String, String>,
    body: Option<String>,
    template: Arc<RequestTemplate>, // Reference to flyweight
}

impl RequestContext {
    pub fn new(url: String, template: Arc<RequestTemplate>) -> Self {
        Self {
            url,
            custom_headers: HashMap::new(),
            body: None,
            template,
        }
    }

    pub fn with_header(mut self, key: String, value: String) -> Self {
        self.custom_headers.insert(key, value);
        self
    }

    pub fn with_body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }
}

// Shared queue across all instances
pub struct RequestQueue {
    queue: Arc<Mutex<VecDeque<RequestContext>>>,
    semaphore: Arc<Semaphore>,
    templates: Arc<Mutex<HashMap<String, Arc<RequestTemplate>>>>, // Flyweight factory
}

impl RequestQueue {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            templates: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    // Flyweight factory method
    pub fn get_template(&self, method: &str) -> Arc<RequestTemplate> {
        let mut templates = self.templates.lock().unwrap();
        
        if let Some(template) = templates.get(method) {
            // Return existing flyweight
            Arc::clone(template)
        } else {
            // Create new flyweight and store it
            let template = Arc::new(RequestTemplate::new(method.to_string()));
            templates.insert(method.to_string(), Arc::clone(&template));
            template
        }
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
}

// Client that shares the queue
#[derive(Clone)]
pub struct HttpClient {
    queue: Arc<RequestQueue>,
    client_id: String,
}

impl HttpClient {
    pub fn new(queue: Arc<RequestQueue>, client_id: String) -> Self {
        Self { queue, client_id }
    }

    pub fn get(&self, url: &str) -> RequestBuilder {
        let template = self.queue.get_template("GET");
        RequestBuilder::new(url.to_string(), template, Arc::clone(&self.queue))
    }

    pub fn post(&self, url: &str) -> RequestBuilder {
        let template = self.queue.get_template("POST");
        RequestBuilder::new(url.to_string(), template, Arc::clone(&self.queue))
    }

    pub fn process_next(&self) -> Option<RequestContext> {
        self.queue.dequeue()
    }

    pub fn queue_size(&self) -> usize {
        self.queue.len()
    }
}

// Builder pattern for creating requests
pub struct RequestBuilder {
    context: RequestContext,
    queue: Arc<RequestQueue>,
}

impl RequestBuilder {
    pub fn new(url: String, template: Arc<RequestTemplate>, queue: Arc<RequestQueue>) -> Self {
        Self {
            context: RequestContext::new(url, template),
            queue,
        }
    }

    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.context = self.context.with_header(key.to_string(), value.to_string());
        self
    }

    pub fn body(mut self, body: &str) -> Self {
        self.context = self.context.with_body(body.to_string());
        self
    }

    pub fn send(self) {
        self.queue.enqueue(self.context);
    }
}

// Example usage
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flyweight_pattern_with_shared_queue() {
        // Create shared queue
        let shared_queue = Arc::new(RequestQueue::new(5));

        // Create multiple clients sharing the same queue
        let client1 = HttpClient::new(Arc::clone(&shared_queue), "client1".to_string());
        let client2 = HttpClient::new(Arc::clone(&shared_queue), "client2".to_string());
        let client3 = HttpClient::new(Arc::clone(&shared_queue), "client3".to_string());

        // Each client adds requests to the shared queue
        client1.get("https://api.example.com/users")
            .header("Authorization", "Bearer token1")
            .send();

        client2.post("https://api.example.com/posts")
            .header("Authorization", "Bearer token2")
            .body(r#"{"title": "Hello", "content": "World"}"#)
            .send();

        client3.get("https://api.example.com/comments")
            .header("Authorization", "Bearer token3")
            .send();

        // All requests are in the shared queue
        assert_eq!(shared_queue.len(), 3);

        // Any client can process requests from the shared queue
        let request1 = client1.process_next().unwrap();
        let request2 = client2.process_next().unwrap();
        let request3 = client3.process_next().unwrap();

        // Verify flyweight sharing: GET templates should be the same instance
        let get_template1 = shared_queue.get_template("GET");
        let get_template2 = shared_queue.get_template("GET");
        assert!(Arc::ptr_eq(&get_template1, &get_template2)); // Same memory address

        println!("Request 1: {:?}", request1);
        println!("Request 2: {:?}", request2);
        println!("Request 3: {:?}", request3);
    }
}
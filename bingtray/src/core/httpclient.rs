use std::sync::Arc;
use super::request::{RequestQueue, RequestContext};

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
        RequestBuilder::new(url.to_string(), "GET".to_string(), Arc::clone(&self.queue))
    }

    pub fn post(&self, url: &str) -> RequestBuilder {
        RequestBuilder::new(url.to_string(), "POST".to_string(), Arc::clone(&self.queue))
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
    pub fn new(url: String, method: String, queue: Arc<RequestQueue>) -> Self {
        Self {
            context: RequestContext::new(url, method),
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
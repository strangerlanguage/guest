use std::{
    collections::HashMap,
    io::{prelude::*, BufReader},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};
/// A simple HTTP server implementation.
///
/// # Example
///
/// Create a server instance and register a GET route:
///
/// ```rust
/// use guest::Server;
///
/// let mut server = Server::new();
/// server.get("/", || "HTTP/1.1 200 OK\r\n\r\nHello, World!".to_string());
/// ```
///
/// # Description
/// This simple example shows how to create a 'Server' instance.
/// Register a GET route and simulate the HTTP request to get the response.
pub struct Server {
    router: Arc<Mutex<HashMap<String, Arc<dyn Fn() -> String + Send + Sync + 'static>>>>,
}

impl Server {
    pub fn new() -> Self {
        Self {
            router: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn get<F>(&mut self, path: &str, handler: F)
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        self.router
            .lock()
            .unwrap()
            .insert(path.to_string(), Arc::new(handler));
    }

    pub fn listener(&self, address: &str) {
        let listener = TcpListener::bind(address).unwrap();
        for stream in listener.incoming() {
            let router = Arc::clone(&self.router);
            let stream = stream.unwrap();
            thread::spawn(move || {
                let server = Server { router };
                server.handle_connection(stream);
            });
        }
    }

    pub fn handle_connection(&self, mut stream: TcpStream) {
        let buf_reader = BufReader::new(&mut stream);

        if let Some(line) = buf_reader.lines().next() {
            let line = line.unwrap();
            let parts: Vec<&str> = line.split(' ').collect();

            let method = parts[0];
            let path = parts[1];

            const NOT_FOUND_RESPONSE: &str = "HTTP/1.1 404 NOT FOUND\r\n\r\n404 Not Found";

            let handler_opt = {
                let router = self.router.lock().unwrap();
                router.get(path).cloned()
            };

            let response = match method {
                "GET" => handler_opt
                    .map(|handler| handler())
                    .unwrap_or_else(|| NOT_FOUND_RESPONSE.to_string()),
                _ => NOT_FOUND_RESPONSE.to_string(),
            };

            stream.write_all(response.as_bytes()).unwrap();
        }
    }
}

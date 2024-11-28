use std::{
    collections::HashMap,
    io::{prelude::*, BufReader},
    net::{SocketAddr, TcpListener, TcpStream},
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
/// use guest_server::Server;
/// use std::time::Duration;
/// use std::thread;
///
/// let mut server = Server::new();
/// server.get("/", home);
///
/// fn home()->String{
///     format!("HTTP/1.1 200 OK\r\n\r\nHello, World!")
/// }
///
/// server.listener(80);
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

    /// # parameter
    /// - 'path' : indicates the route path to be registered, for example, '/home'
    /// - 'handler' : handles the closure of the path request and returns a response of type String
    pub fn get<F>(&mut self, path: &str, handler: F)
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        self.router
            .lock()
            .unwrap()
            .insert(path.to_string(), Arc::new(handler));
    }

    /// # parameter
    /// - 'port' : indicates the port number to listen
    pub fn listener(&self, port: u16) {
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        let listener = TcpListener::bind(addr).unwrap();

        // Listen for incoming connections
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let router = Arc::clone(&self.router);
                    // Handle each connection in a separate thread
                    thread::spawn(move || {
                        let server = Server { router };
                        server.handle_connection(stream);
                    });
                }
                Err(e) => eprintln!("Failed to accept connection: {}", e),
            }
        }
    }

    /// Handle the HTTP connection.
    fn handle_connection(&self, mut stream: TcpStream) {
        let buf_reader = BufReader::new(&mut stream);

        // Read the first line of the request
        if let Some(Ok(line)) = buf_reader.lines().next() {
            let parts: Vec<&str> = line.split_whitespace().collect();

            if parts.len() < 2 {
                // Invalid request line
                self.send_response(
                    &mut stream,
                    "HTTP/1.1 400 Bad Request\r\n\r\n400 Bad Request",
                );
                return;
            }

            let method = parts[0];
            let path = parts[1];

            const NOT_FOUND_RESPONSE: &str = "HTTP/1.1 404 NOT FOUND\r\n\r\n404 Not Found";

            // Lock router once and handle request
            let response = match method {
                "GET" => {
                    let handler = self.router.lock().unwrap().get(path).cloned();
                    handler
                        .map(|h| h())
                        .unwrap_or_else(|| NOT_FOUND_RESPONSE.to_string())
                }
                _ => NOT_FOUND_RESPONSE.to_string(),
            };

            // Send the response back to the client
            self.send_response(&mut stream, &response);
        }
    }

    /// Send an HTTP response to the client.
    fn send_response(&self, stream: &mut TcpStream, response: &str) {
        if let Err(e) = stream.write_all(response.as_bytes()) {
            eprintln!("Failed to send response: {}", e);
        }
    }
}

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
/// use guest_server::{Server,HttpResponse};
/// use std::time::Duration;
/// use std::thread;
///
/// let mut server = Server::new();
/// server.get("/", home);
///
/// fn home()->HttpResponse{
///      HttpResponse::new(200, Some("Hello, World!".to_string()))
/// }
///
/// server.listener(80);
/// ```
///
/// # Description
/// This simple example shows how to create a 'Server' instance.
/// Registers a GET route and simulates an HTTP request to obtain the response.
pub struct Server {
    router: Arc<Mutex<HashMap<String, Arc<dyn Fn() -> HttpResponse + Send + Sync + 'static>>>>,
}

impl Server {
    /// Creates and initializes a new server instance.
    pub fn new() -> Self {
        Self {
            router: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Registers a GET route with a specified path and handler.
    ///
    /// # Parameters
    /// - 'path' : The route path to register, e.g., '/home'.
    /// - 'handler' : The closure that processes the request for this path and returns an HttpResponse.
    pub fn get<F>(&mut self, path: &str, handler: F)
    where
        F: Fn() -> HttpResponse + Send + Sync + 'static,
    {
        self.router
            .lock()
            .unwrap()
            .insert(path.to_string(), Arc::new(handler));
    }

    /// Starts the server and listens on a specified port.
    ///
    /// # Parameters
    /// - 'port' : The port number to listen on.
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

    // Handle the HTTP connection.
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

            // Invalid request line
            let not_found_response = HttpResponse::new(400, None);

            // Lock router once and handle request
            let response = match method {
                "GET" => {
                    let handler = self.router.lock().unwrap().get(path).cloned();
                    handler.map(|h| h()).unwrap_or_else(|| not_found_response)
                }
                _ => not_found_response,
            };
            let res = Self::generate_http_response(&response);
            // Send the response back to the client
            self.send_response(&mut stream, &res);
        }
    }

    // Send an HTTP response to the client.
    fn send_response(&self, stream: &mut TcpStream, response: &str) {
        if let Err(e) = stream.write_all(response.as_bytes()) {
            eprintln!("Failed to send response: {}", e);
        }
    }

    // Generates the full HTTP response as a string, including headers and body.
    fn generate_http_response(response: &HttpResponse) -> String {
        let mut response_string = format!(
            "HTTP/1.1 {} {}\r\n",
            response.status_code,
            response.get_status_message()
        );
        for (key, value) in &response.headers {
            response_string.push_str(&format!("{}: {}\r\n", key, value));
        }
        response_string.push_str("\r\n");

        if let Some(body) = &response.body {
            response_string.push_str(body);
        }

        response_string
    }
}

pub struct HttpResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl HttpResponse {
    /// Create a default HttpResponse.
    /// Defaults: Status code 200, headers Content-Type and Content-Length.
    ///
    /// # Parameters
    /// - 'status_code' : The request status code
    /// - 'body' : The request body
    pub fn new(status_code: u16, body: Option<String>) -> Self {
        let mut headers = HashMap::new();
        let default_content_type = if let Some(ref b) = body {
            if b.starts_with('{') && b.ends_with('}') {
                "application/json".to_string()
            } else {
                "text/plain".to_string()
            }
        } else {
            "text/plain".to_string()
        };
        headers.insert("Content-Type".to_string(), default_content_type);
        if let Some(ref b) = body {
            headers.insert("Content-Length".to_string(), b.len().to_string());
        }

        HttpResponse {
            status_code,
            headers,
            body,
        }
    }

    /// Adds or updates a single header field.
    ///
    /// # Parameters
    /// - 'key' : The request header key
    /// - 'value' : The request header value
    pub fn insert_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Add or overwrite header fields in batches.
    ///
    /// # Parameters
    /// - 'new_headers' : The request headers
    pub fn insert_headers(mut self, new_headers: HashMap<String, String>) -> Self {
        self.headers.extend(new_headers);
        self
    }

    /// Retrieves the description for the status code.
    pub fn get_status_message(&self) -> &'static str {
        match self.status_code {
            200 => "OK",
            201 => "Created",
            400 => "Bad Request",
            404 => "Not Found",
            500 => "Internal Server Error",
            _ => "Unknown Status",
        }
    }
}

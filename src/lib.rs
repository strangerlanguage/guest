use std::{
    collections::HashMap,
    io::{prelude::*, BufReader},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{Arc, RwLock},
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
#[derive(Eq, PartialEq, Hash)]
pub enum HttpMethod {
    GET,
    POST,
}

/// Represents the HTTP server.
pub struct Server {
    routes: Arc<
        RwLock<
            HashMap<(HttpMethod, String), Arc<dyn Fn() -> HttpResponse + Send + Sync + 'static>>,
        >,
    >,
}

impl Server {
    /// Creates and initializes a new server instance.
    ///
    /// # Returns
    /// A new instance of `Server` with an empty route configuration.
    pub fn new() -> Self {
        Self {
            routes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a route with a specific HTTP method, path, and handler.
    ///
    /// # Parameters
    /// - 'method' : The HTTP method (GET, POST) for this route.
    /// - 'path' : The route path (e.g., '/home').
    /// - 'handler' : The closure that processes the request for this path.
    pub fn route<F>(&mut self, method: HttpMethod, path: &str, handler: F)
    where
        F: Fn() -> HttpResponse + Send + Sync + 'static,
    {
        self.routes
            .write()
            .unwrap()
            .insert((method, path.to_string()), Arc::new(handler));
    }

    /// Registers a GET route with a specified path and handler.
    ///
    /// # Parameters
    /// - 'path' : The route path to register, e.g., '/home'.
    /// - 'handler' : The closure that processes the request for this path.
    pub fn get<F>(&mut self, path: &str, handler: F)
    where
        F: Fn() -> HttpResponse + Send + Sync + 'static,
    {
        self.route(HttpMethod::GET, path, handler);
    }

    /// Registers a POST route with a specified path and handler.
    ///
    /// # Parameters
    /// - 'path' : The route path to register, e.g., '/submit'.
    /// - 'handler' : The closure that processes the request for this path.
    ///
    /// # Example
    ///
    /// ```rust
    /// use guest_server::{Server,HttpResponse};
    /// let mut server = Server::new();
    /// server.post("/submit",submit);
    /// fn submit()->HttpResponse{
    ///     HttpResponse::new(200, Some("{\"key\":\"value\"}".to_string())).insert_header("Content-Type","application/json")
    /// }
    /// server.listener(80);
    /// ```
    pub fn post<F>(&mut self, path: &str, handler: F)
    where
        F: Fn() -> HttpResponse + Send + Sync + 'static,
    {
        self.route(HttpMethod::POST, path, handler);
    }

    /// Starts the server and listens for incoming connections on the specified port.
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
                    let routes = Arc::clone(&self.routes);
                    // Handle each connection in a separate thread
                    thread::spawn(move || {
                        let server = Server { routes };
                        server.handle_connection(stream);
                    });
                }
                Err(e) => eprintln!("Failed to accept connection: {}", e),
            }
        }
    }

    /// Handles the HTTP connection by reading the request and sending an appropriate response.
    ///
    /// # Parameters
    /// - 'stream' : The TCP stream for the current connection.
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

            // Lock routes once and handle request
            let method: Option<HttpMethod> = match method {
                "GET" => Some(HttpMethod::GET),
                "POST" => Some(HttpMethod::POST),
                _ => None,
            };

            let response = if let Some(method) = method {
                self.routes
                    .read()
                    .unwrap()
                    .get(&(method, path.to_string()))
                    .cloned()
                    .map_or_else(|| HttpResponse::new(404, None), |h| h())
            } else {
                HttpResponse::new(405, None) // Method Not Allowed
            };

            let res = Self::generate_http_response(&response);
            // Send the response back to the client
            self.send_response(&mut stream, &res);
        }
    }

    /// Sends an HTTP response to the client.
    ///
    /// # Parameters
    /// - 'stream' : The TCP stream to send the response over.
    /// - 'response' : The response content (HTTP status, headers, body).
    fn send_response(&self, stream: &mut TcpStream, response: &str) {
        if let Err(e) = stream.write_all(response.as_bytes()) {
            eprintln!("Failed to send response: {}", e);
        }
    }

    /// Generates the full HTTP response string, including status code, headers, and body.
    ///
    /// # Parameters
    /// - 'response' : The HttpResponse object containing status, headers, and body.
    ///
    /// # Returns
    /// A string representing the full HTTP response.
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

/// Represents an HTTP response, including status code, headers, and body.
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl HttpResponse {
    /// Creates a new HttpResponse with the specified status code and body.
    ///
    /// # Parameters
    /// - 'status_code' : The HTTP status code (e.g., 200, 404).
    /// - 'body' : The response body content (optional).
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
    /// - 'key' : The header key.
    /// - 'value' : The header value.
    pub fn insert_header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Adds or updates multiple header fields in batch.
    ///
    /// # Parameters
    /// - 'new_headers' : A HashMap containing the new header fields.
    pub fn insert_headers(mut self, new_headers: HashMap<String, String>) -> Self {
        self.headers.extend(new_headers);
        self
    }

    /// Retrieves the description message for the status code.
    ///
    /// # Returns
    /// A string representing the status message for the given status code.
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

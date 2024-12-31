use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Error, ErrorKind, Read, Write},
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
/// fn home(query_params: Option<Vec<u8>>) -> HttpResponse {
///       HttpResponse::new(200, Some("Hello, World!".to_string()))
/// }
///
/// server.listener(80);
/// ```
///
/// # Description
/// This simple example shows how to create a 'Server' instance.
/// Registers a GET route and simulates an HTTP request to obtain the response.

#[derive(Eq, PartialEq, Hash, Clone)]
pub enum HttpMethod {
    GET,
    POST,
}

type Routes = Arc<
    RwLock<
        HashMap<
            (HttpMethod, String),
            Arc<dyn Fn(Option<Vec<u8>>) -> HttpResponse + Send + Sync + 'static>,
        >,
    >,
>;

/// Represents an HTTP server.
///
/// This server listens for incoming HTTP requests, dispatches them to the correct handler based on the
/// method and path, and sends back appropriate HTTP responses. It supports GET and POST routes.
///
/// The server is multi-threaded, handling each incoming connection in a new thread.
pub struct Server {
    routes: Routes, // A map storing routes and their associated handler functions.
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
    fn route<F>(&mut self, method: HttpMethod, path: &str, handler: F)
    where
        F: Fn(Option<Vec<u8>>) -> HttpResponse + Send + Sync + 'static,
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
        F: Fn(Option<Vec<u8>>) -> HttpResponse + Send + Sync + 'static,
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
    /// fn submit(body: Option<Vec<u8>>) -> HttpResponse {
    ///     HttpResponse::new(200, Some("{\"key\":\"value\"}".to_string())).insert_header("Content-Type","application/json")
    /// }
    /// server.listener(8080);
    /// ```
    pub fn post<F>(&mut self, path: &str, handler: F)
    where
        F: Fn(Option<Vec<u8>>) -> HttpResponse + Send + Sync + 'static,
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
                    thread::spawn(move || {
                        if let Err(e) = Server::handle_connection(routes, stream) {
                            eprintln!("Connection failed: {}", e);
                        }
                    });
                }
                Err(e) => eprintln!("Failed to accept connection: {}", e),
            }
        }
    }

    /// Handles the incoming TCP connection, processes the HTTP request, and sends back a response.
    ///
    /// # Parameters
    /// - `routes`: The `Routes` object containing the routing information. This is used to match the
    ///   incoming HTTP request's path and method to the appropriate handler function.
    /// - `stream`: The TCP stream representing the connection to the client. This is used to read
    ///   the request and send the response back to the client. The stream is mutable because it will
    ///   be written to as part of generating the HTTP response.
    fn handle_connection(routes: Routes, mut stream: TcpStream) -> Result<(), Error> {
        let mut reader = BufReader::new(&stream);
        let mut buffer_request = Vec::new();
        let mut header_parsed = false;
        let mut content_length = 0;
        let mut method = Option::None;
        let mut path = String::new();

        loop {
            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line)?;

            if bytes_read == 0 {
                break;
            }

            buffer_request.extend_from_slice(line.as_bytes());

            if line == "\r\n" {
                header_parsed = true;
                break;
            }

            if line.starts_with("GET") || line.starts_with("POST") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    method = match parts[0] {
                        "GET" => Some(HttpMethod::GET),
                        "POST" => Some(HttpMethod::POST),
                        _ => None,
                    };
                    path = parts[1].to_string();
                }
            }

            if line.to_lowercase().starts_with("content-length:") {
                if let Ok(length) = line["content-length:".len()..].trim().parse::<usize>() {
                    content_length = length;
                }
            }
        }

        if !header_parsed {
            return Err(Error::new(ErrorKind::InvalidData, "Incomplete header"));
        }

        let mut body = Vec::new();
        if content_length > 0 {
            body.resize(content_length, 0);
            reader.read_exact(&mut body)?;
        }

        buffer_request.extend_from_slice(&body);

        let response = if let Some(method) = method {
            Server::processing_response(&routes, body, method, path)
        } else {
            HttpResponse::new(405, None)
        };

        let res = Server::generate_http_response(&response);
        Server::send_response(&mut stream, res);

        Ok(())
    }

    /// Processes the HTTP response based on the method and path, invoking the registered handler.
    ///
    /// # Parameters
    /// - 'routes' : A shared reference to the routes configuration.
    /// - 'body' : The body of the request as a vector of bytes.
    /// - 'method' : The HTTP method (GET, POST) for the request.
    /// - 'path' : The requested path for the route.
    ///
    /// # Returns
    /// The generated HttpResponse based on the handler or a 404 response if no handler is found.

    fn processing_response(
        routes: &Routes,
        body: Vec<u8>,
        method: HttpMethod,
        path: String,
    ) -> HttpResponse {
        routes
            .read()
            .unwrap()
            .get(&(method, path))
            .cloned()
            .map_or_else(
                || HttpResponse::new(404, None),
                |handler| handler(Some(body)),
            )
    }

    /// Sends an HTTP response to the client.
    ///
    /// # Parameters
    /// - 'stream' : The TCP stream to send the response over.
    /// - 'response' : The response content (HTTP status, headers, body) to be sent.
    ///
    /// # Notes
    /// This function writes the full HTTP response to the provided stream.
    /// It logs an error if the response cannot be sent.
    fn send_response(stream: &mut TcpStream, response: Vec<u8>) {
        if let Err(e) = stream.write_all(&response) {
            eprintln!("Failed to send response: {}", e);
        }
    }

    /// Generates the full HTTP response string, including status code, headers, and body.
    ///
    /// # Parameters
    /// - 'response' : The HttpResponse object containing status, headers, and body.
    ///
    /// # Returns
    /// A vector of bytes representing the full HTTP response.
    fn generate_http_response(response: &HttpResponse) -> Vec<u8> {
        let mut response_string = format!(
            "HTTP/1.1 {} {}\r\n",
            response.status_code,
            response.get_status_message() // Retrieves the status message based on status code
        );
        for (key, value) in &response.headers {
            response_string.push_str(&format!("{}: {}\r\n", key, value)); // Add headers to the response
        }
        response_string.push_str("\r\n");

        let mut res = response_string.into_bytes();
        if let Some(body) = &response.body {
            res.extend_from_slice(body.as_bytes()); // Append the response body if it exists
        }

        res
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

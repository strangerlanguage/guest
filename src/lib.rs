use std::{
    borrow::Cow,
    collections::HashMap,
    io::{prelude::*, Error, ErrorKind},
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
/// fn home(query_params: String) -> HttpResponse {
///       HttpResponse::new(200, Some("Hello, World!".to_string()))
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
            HashMap<(HttpMethod, String), Arc<dyn Fn(String) -> HttpResponse + Send + Sync + 'static>>,
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
        F: Fn(String) -> HttpResponse + Send + Sync + 'static,
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
        F: Fn(String) -> HttpResponse + Send + Sync + 'static,
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
    /// fn submit(body: String) -> HttpResponse {
    ///     HttpResponse::new(200, Some("{\"key\":\"value\"}".to_string())).insert_header("Content-Type","application/json")
    /// }
    /// server.listener(80);
    /// ```
    pub fn post<F>(&mut self, path: &str, handler: F)
    where
        F: Fn(String) -> HttpResponse + Send + Sync + 'static,
    {
        self.route(HttpMethod::POST, path, move |body| handler(body));
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
                        server.handle_connection(stream).unwrap();
                    });
                }
                Err(e) => eprintln!("Failed to accept connection: {}", e),
            }
        }
    }

    /// Extracts the HTTP method, path, and content length from the request header.
    ///
    /// # Parameters
    /// - `request`: A borrowed string slice of the HTTP request.
    ///
    /// # Returns
    /// A tuple containing the optional HTTP method, path, and content length.
    fn get_response_header<'a>(
        request: &'a Cow<'_, str>,
    ) -> Result<(Option<HttpMethod>, &'a str, usize), Error> {
        let mut lines = request.lines();

        match lines.next() {
            Some(first_line) => {
                let parts: Vec<&str> = first_line.split_whitespace().collect();
                if parts.len() < 2 {
                    return Err(Error::new(ErrorKind::InvalidData, "Invalid request header"));
                }
                let method_str = parts[0];
                let path = parts[1];

                let method = match method_str {
                    "GET" => Some(HttpMethod::GET),
                    "POST" => Some(HttpMethod::POST),
                    _ => None,
                };
                let content_length = lines
                    .find_map(|line| {
                        if line.starts_with("Content-Length:") {
                            line["Content-Length:".len()..].trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                Ok((method, path, content_length))
            }
            None => Err(Error::new(ErrorKind::InvalidData, "Empty request header")),
        }
    }

    /// Parses the HTTP request body to extract key-value pairs.
    ///
    /// # Parameters
    /// - `request`: A borrowed string slice of the HTTP request.
    /// - `content_length`: The expected content length of the body.
    ///
    /// # Returns
    /// A vector of key-value tuples parsed from the body.
    fn get_response_body<'a>(
        request: &'a Cow<'a, str>,
        content_length: usize,
    ) -> Result<Vec<(&'a str, &'a str)>, Error> {
        match request.find("\r\n\r\n") {
            Some(body_start) => {
                let body = &request[body_start + 4..body_start + 4 + content_length];
                let params: Vec<(&'a str, &'a str)> = body
                    .split('&')
                    .filter_map(|pair| {
                        let mut parts = pair.split('=');
                        if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                            Some((key, value))
                        } else {
                            None
                        }
                    })
                    .collect();
                Ok(params)
            }
            None => Err(Error::new(ErrorKind::InvalidData, "Malformed body content")),
        }
    }

    /// Handles the HTTP connection by reading the request and sending an appropriate response.
    ///
    /// # Parameters
    /// - 'stream' : The TCP stream for the current connection.
    fn handle_connection(&self, mut stream: TcpStream) -> Result<(), Error> {
        let mut buf = [0; 1024];
        match stream.read(&mut buf) {
            Ok(_) => {
                let request = String::from_utf8_lossy(&buf);
                match Self::get_response_header(&request) {
                    Ok((method, path, content_length)) => {
                        let body = Self::get_response_body(&request, content_length).unwrap();
                        let body_str = body.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("&");
                        let response = if let Some(method) = method {
                            self.routes
                                .read()
                                .unwrap()
                                .get(&(method, path.to_string()))
                                .cloned()
                                .map_or_else(|| HttpResponse::new(404, None), |h| h(body_str))
                        } else {
                            HttpResponse::new(405, None)
                        };

                        let res = Self::generate_http_response(&response);
                        self.send_response(&mut stream, &res);

                        Ok(())
                    }
                    Err(_) => Err(Error::new(ErrorKind::InvalidData, "Invalid request header")),
                }
            }
            Err(error) => Err(error),
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

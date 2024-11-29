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
///     HttpResponse::Body(String::from("Hello, World!"))
/// }
///
/// server.listener(80);
/// ```
///
/// # Description
/// This simple example shows how to create a 'Server' instance.
/// Register a GET route and simulate the HTTP request to get the response.
pub struct Server {
    router: Arc<Mutex<HashMap<String, Arc<dyn Fn() -> HttpResponse + Send + Sync + 'static>>>>,
}

pub enum HttpResponse {
    StatusCode(u16),
    Body(String),
}

impl HttpResponse {
    // Returns the status message associated with a status code.
    pub fn status_message(&self) -> (&'static str, &'static str) {
        match self {
            HttpResponse::StatusCode(200) => ("OK", "Request was successful"),
            HttpResponse::StatusCode(400) => {
                ("Bad Request", "The request was invalid or malformed")
            }
            HttpResponse::StatusCode(404) => {
                ("Not Found", "The requested resource could not be found")
            }
            HttpResponse::StatusCode(500) => {
                ("Internal Server Error", "The server encountered an error")
            }
            _ => ("Unknown", "An unknown error occurred"),
        }
    }
    // Returns the HTTP status code.
    pub fn status_code(&self) -> u16 {
        match self {
            HttpResponse::StatusCode(code) => *code,
            _ => 200,
        }
    }
}

impl Server {
    /// Creates a new server instance.
    pub fn new() -> Self {
        Self {
            router: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Registers a GET route with a path and handler.
    ///
    /// # Parameters
    /// - 'path' : The route path to be registered, e.g., '/home'.
    /// - 'handler' : The closure that handles the path request and returns an `HttpResponse`.
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

            // 默认响应，如果找不到对应的处理函数
            let not_found_response = HttpResponse::Body(String::from("Hello, world!"));

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
        let (status_message, default_body) = response.status_message();
        let body = match response {
            HttpResponse::Body(b) => b,
            _ => &default_body.to_owned(),
        };
        format!(
            "HTTP/1.1 {} {}\r\n\
            {}",
            response.status_code(),
            status_message,
            body
        )
    }
}

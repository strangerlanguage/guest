# Guest Server

A simple HTTP server implementation in Rust, designed for educational purposes or lightweight projects. This library provides a basic mechanism for handling HTTP GET requests and simulating a server response.

## Features

- **Minimalistic design**: Provides just enough functionality to serve HTTP GET requests.
- **Threaded handling**: Each incoming request is handled in its own thread, making it easy to scale for multiple connections.
- **Easy to use**: Simple API for registering GET routes and handling requests.

## Example

Create a `Server` instance, register a GET route, and start the server:

```rust
use guest_server::Server;

let mut server = Server::new();
server.get("/", || "HTTP/1.1 200 OK\r\n\r\nHello, World!".to_string());
server.listener("127.0.0.1:8080");
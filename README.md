# A Simple HTTP Server Implementation

## Example

Create a server instance and register a GET route:

```rust
use guest_server::{Server, HttpResponse};

let mut server = Server::new();

server.get("/", home);

fn home(_: String) -> HttpResponse {
        HttpResponse::new(200, Some("Hello, World!".to_string()))
}

server.listener(80);
```

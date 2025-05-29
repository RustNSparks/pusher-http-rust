Okay, here's a draft for a comprehensive README file for your `pusher-http-rust` library, based on the code you've provided.

```markdown
# Pusher HTTP Rust Client

A Rust client for interacting with the Pusher HTTP API, allowing you to publish events, authorize channels, authenticate users, and handle webhooks from your Rust applications.

## Features

* Trigger events on public, private, and presence channels.
* Trigger events to specific users (User Authentication).
* Trigger batch events for efficiency.
* Support for end-to-end encrypted channels.
* Authorize client subscriptions to private, presence, and encrypted channels.
* Authenticate users for user-specific Pusher features.
* Terminate user connections.
* Validate and process incoming Pusher webhooks.
* Configurable host, port, scheme (HTTP/HTTPS), and timeout.
* Asynchronous API using `async/await`.
* Typed responses and errors.

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
# If publishing to crates.io:
# pusher-http-rust = "0.1.0" # Replace with the desired version
# Or, for local development using a path:
# pusher-http-rust = { path = "../path/to/pusher-http-rust" }

serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
# reqwest is used internally but you might need it for response handling
# reqwest = { version = "0.11", features = ["json"] } # The library already includes reqwest as a direct or indirect dependency
```

Then run `cargo build`.

## Usage

### 1. Initialization

First, configure and create a `Pusher` client:

```rust
use pusher_http_rust::{Pusher, Config, PusherError, events}; //
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), PusherError> {
    let config = Config::new("YOUR_APP_ID", "YOUR_APP_KEY", "YOUR_APP_SECRET") //
        .cluster("YOUR_CLUSTER") // e.g., "eu", "ap1"
        .timeout(std::time::Duration::from_secs(5)); // Optional timeout

    // For encrypted channels, set the master key:
    // let config = config.encryption_master_key_base64("YOUR_BASE64_ENCRYPTION_MASTER_KEY")?;

    let pusher = Pusher::new(config); //

    // Your application logic here...
    Ok(())
}
```

You can also initialize from a Pusher URL:
```rust
    let pusher_from_url = Pusher::from_url("[http://KEY:SECRET@api-CLUSTER.pusher.com/apps/APP_ID](http://KEY:SECRET@api-CLUSTER.pusher.com/apps/APP_ID)", None)?; //
```

### 2. Triggering Events

```rust
    let channels = vec!["my-channel".to_string()];
    let event_name = "new-message";
    let data = json!({ "text": "Hello from Rust!" });

    match pusher.trigger(&channels, event_name, &data, None).await { //
        Ok(response) => {
            println!("Event triggered! Status: {}", response.status());
            // let body = response.text().await?;
            // println!("Response: {}", body);
        }
        Err(e) => eprintln!("Error triggering event: {:?}", e),
    }
```

**Triggering to an Encrypted Channel:**
If `channels` contains a single encrypted channel name (e.g., `"private-encrypted-mychannel"`) and the `encryption_master_key` is set in the `Config`, the data will be automatically encrypted.

**Excluding a Recipient:**
Use `TriggerParams` to exclude a specific `socket_id`.
```rust
    let params = events::TriggerParams {
        socket_id: Some("socket_id_to_exclude".to_string()), //
        info: None,
    };
    pusher.trigger(&channels, event_name, &data, Some(params)).await?;
```

### 3. Triggering Batch Events

```rust
    let batch = vec![
        events::BatchEvent { //
            channel: "channel-a".to_string(),
            name: "event1".to_string(),
            data: json!({"value": 1}).to_string(),
            socket_id: None, info: None,
        },
        events::BatchEvent { //
            channel: "channel-b".to_string(),
            name: "event2".to_string(),
            data: json!({"value": 2}).to_string(),
            socket_id: None, info: None,
        },
    ];

    match pusher.trigger_batch(batch).await { //
        Ok(response) => println!("Batch triggered! Status: {}", response.status()),
        Err(e) => eprintln!("Error triggering batch: {:?}", e),
    }
```

### 4. Authorizing Channels (for Private & Presence Channels)

This is typically done in an HTTP request handler when a client attempts to subscribe.

```rust
// --- Assuming use in a web framework like Axum ---
// async fn pusher_auth_handler(
//     pusher: &Pusher,
//     socket_id: &str,
//     channel_name: &str,
//     presence_data: Option<&serde_json::Value> // For presence channels
// ) -> Result<impl serde::Serialize, PusherError> {

    let socket_id = "123.456"; // From client
    let channel_name = "private-mychannel"; // From client

    // For presence channels, include user data:
    let user_data_for_presence = Some(json!({
        "user_id": "unique_user_id",
        "user_info": { "name": "Alice" }
    }));

    match pusher.authorize_channel(socket_id, channel_name, user_data_for_presence.as_ref()) { //
        Ok(auth_signature) => {
            // `auth_signature` is a `SocketAuth` struct
            // Send this back to the client as JSON.
            // If it's an encrypted channel, `auth_signature.shared_secret` will be populated.
            println!("Auth success: {:?}", auth_signature);
            // Ok(auth_signature)
        }
        Err(e) => {
            eprintln!("Auth error: {:?}", e);
            // Err(e)
        }
    }
// }
```

### 5. Authenticating Users

For user-specific features like "server-to-user" events.

```rust
// --- Assuming use in a web framework ---
// async fn user_auth_handler(
//     pusher: &Pusher,
//     socket_id: &str,
//     user_details: &serde_json::Value // Must contain an "id" field
// ) -> Result<impl serde::Serialize, PusherError> {

    let socket_id = "789.012"; // From client
    let current_user_data = json!({
        "id": "user-bob", // Required "id" field
        "name": "Bob The Builder",
        "email": "bob@example.com"
    });

    match pusher.authenticate_user(socket_id, &current_user_data) { //
        Ok(user_auth_response) => {
            // `user_auth_response` is a `UserAuth` struct
            // Send this back to the client as JSON.
            println!("User auth success: {:?}", user_auth_response);
            // Ok(user_auth_response)
        }
        Err(e) => {
            eprintln!("User auth error: {:?}", e);
            // Err(e)
        }
    }
// }
```

### 6. Sending an Event to a User

Requires user authentication to be set up on the client.

```rust
    let user_id = "user-bob";
    let event_name = "personal-notification";
    let data = json!({ "alert": "Your report is ready!" });

    match pusher.send_to_user(user_id, event_name, &data).await { //
        Ok(response) => println!("Sent to user! Status: {}", response.status()),
        Err(e) => eprintln!("Error sending to user: {:?}", e),
    }
```

### 7. Terminating User Connections

```rust
    let user_id_to_terminate = "user-charlie";
    match pusher.terminate_user_connections(user_id_to_terminate).await { //
        Ok(response) => println!("Terminate user call successful! Status: {}", response.status()),
        Err(e) => eprintln!("Error terminating user connections: {:?}", e),
    }
```

### 8. Handling Webhooks

Your server can receive webhooks from Pusher.

```rust
use std::collections::BTreeMap;
// --- Assuming use in a web framework like Axum ---
// async fn webhook_handler(
//     pusher: &Pusher,
//     request_headers: &BTreeMap<String, String>, // Extracted from incoming request
//     request_body: &str // Raw request body
// ) {
    // Dummy data for example:
    let mut request_headers = BTreeMap::new();
    request_headers.insert("X-Pusher-Key".to_string(), "YOUR_APP_KEY".to_string());
    request_headers.insert("X-Pusher-Signature".to_string(), "RECEIVED_SIGNATURE".to_string()); // Actual signature from Pusher
    request_headers.insert("Content-Type".to_string(), "application/json".to_string());

    let request_body = r#"{"time_ms":1600000000000,"events":[{"name":"channel_occupied","channel":"my-channel"}]}"#; // Actual body from Pusher

    let webhook = pusher.webhook(&request_headers, request_body); //

    if webhook.is_valid(None) { //
        println!("Webhook is valid!");
        if let Ok(events) = webhook.get_events() { //
            println!("Events: {:?}", events);
            // Process events (they are serde_json::Value)
        }
        if let Ok(time) = webhook.get_time() { //
            println!("Webhook time: {:?}", time);
        }
    } else {
        eprintln!("Webhook is invalid!");
        // Respond with a 401 Unauthorized status
    }
// }
```
You can provide an array of `Token`s to `webhook.is_valid(Some(&extra_tokens))` if you need to validate against multiple app credentials.

### 9. Using with Axum

Here's a brief example of how you might integrate with Axum:

```rust
use axum::{
    extract::{Extension, Json, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use pusher_http_rust::{Pusher, Config, auth, webhook::Webhook};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    pusher: Arc<Pusher>,
}

#[tokio::main]
async fn main() {
    let config = Config::new("APP_ID", "APP_KEY", "APP_SECRET").cluster("CLUSTER");
    let pusher_client = Pusher::new(config);
    let app_state = AppState { pusher: Arc::new(pusher_client) };

    let app = Router::new()
        .route("/pusher/auth", post(axum_pusher_auth_handler))
        .route("/pusher/webhook", post(axum_webhook_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize)]
struct AuthRequest {
    socket_id: String,
    channel_name: String,
    presence_data: Option<Value>,
}

async fn axum_pusher_auth_handler(
    State(state): State<AppState>,
    Json(payload): Json<AuthRequest>,
) -> impl IntoResponse {
    match state.pusher.authorize_channel(
        &payload.socket_id,
        &payload.channel_name,
        payload.presence_data.as_ref(),
    ) {
        Ok(auth_response) => (StatusCode::OK, Json(auth_response)).into_response(),
        Err(_) => (StatusCode::FORBIDDEN, Json(json!({"error": "Forbidden"}))).into_response(),
    }
}

async fn axum_webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    let mut header_btreemap = BTreeMap::new();
    for (key, value) in headers.iter() {
        if let Ok(val_str) = value.to_str() {
            header_btreemap.insert(key.as_str().to_string(), val_str.to_string());
        }
    }
    let webhook = state.pusher.webhook(&header_btreemap, &body);
    if webhook.is_valid(None) {
        // Process webhook.get_events()...
        (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, Json(json!({"error": "Unauthorized"}))).into_response()
    }
}
```

## Configuration Options

The `Config` builder provides several methods:

* `new(app_id, key, secret)`: Basic initialization.
* `cluster(cluster_name)`: Sets the Pusher cluster (e.g., "eu", "ap1").
* `use_tls(true/false)`: Whether to use HTTPS (defaults to true).
* `port(port_number)`: Specify a custom port.
* `timeout(duration)`: Set an HTTP request timeout.
* `encryption_master_key_base64(key_base64)`: Set the 32-byte master encryption key, base64 encoded, for encrypted channels.

## Error Handling

The library returns `pusher_http_rust::Result<T>`, which is an alias for `std::result::Result<T, PusherError>`.
`PusherError` is an enum that covers various error types:
* `Request`: Errors during HTTP requests.
* `Webhook`: Errors related to webhook processing.
* `Config`: Configuration errors.
* `Validation`: Input validation errors.
* `Encryption`: Errors during encryption/decryption processes.
* `Json`: Errors from `serde_json`.
* `Http`: Errors from `reqwest`.

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.
(Consider adding more specific guidelines if you intend for this to be an open project, e.g., code style, testing requirements.)

## License

(Specify your license here, e.g., MIT or Apache 2.0. If you haven't chosen one, consider adding one. For example:
This project is licensed under the MIT License - see the LICENSE.md file for details.)

```

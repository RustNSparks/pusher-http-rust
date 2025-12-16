# Pusher HTTP Rust Client

A Rust client for interacting with the Pusher HTTP API, allowing you to publish events, authorize channels, authenticate users, and handle webhooks from your Rust applications.

## Features

- Trigger events on public, private, and presence channels
- Trigger events to specific users (User Authentication)
- Trigger batch events for efficiency
- Support for end-to-end encrypted channels
- **Tag filtering support** for server-side publication filtering
- Authorize client subscriptions to private, presence, and encrypted channels
- Authenticate users for user-specific Pusher features
- Terminate user connections
- Validate and process incoming Pusher webhooks
- Configurable host, port, scheme (HTTP/HTTPS), and timeout
- Asynchronous API using `async/await`
- Typed responses and errors
- **Fast JSON** with SIMD-accelerated `sonic-rs` library

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
pushers = "1.4.0"
sonic-rs = "0.5"
tokio = { version = "1", features = ["full"] }
```

Then run:

```bash
cargo build
```


## Usage

### 1. Initialization

Configure and create a `Pusher` client:

```rust
use pushers::{Config, Pusher, PusherError};
use sonic_rs::json;

#[tokio::main]
async fn main() -> Result<(), PusherError> {
    let mut config_builder = Config::builder() // Use the builder pattern
        .app_id("YOUR_APP_ID")
        .key("YOUR_APP_KEY")
        .secret("YOUR_APP_SECRET")
        .cluster("YOUR_CLUSTER") // e.g. "eu", "ap1"
        .timeout(std::time::Duration::from_secs(5)); // Optional

    // For encrypted channels:
    // config_builder = config_builder
    // .encryption_master_key_base64("YOUR_BASE64_ENCRYPTION_MASTER_KEY")?;

    let config = config_builder.build()?; // Build the config

    let pusher = Pusher::new(config)?; // Pass the built config

    // Your application logic here...
    Ok(())
}
```


You can also initialize from a Pusher URL:

```rust
use pushers::{Pusher, PusherError}; // Adjusted

# #[tokio::main]
# async fn main() -> Result<(), PusherError> {
let pusher_from_url = Pusher::from_url(
    "http://YOUR_APP_KEY:YOUR_APP_SECRET@api-YOUR_[CLUSTER.pusher.com/apps/YOUR_APP_ID](https://CLUSTER.pusher.com/apps/YOUR_APP_ID)", // Corrected URL format
    None, // You can pass additional config options here if needed
)?;
# Ok(())
# }
```


### 2. Triggering Events

```rust
use pushers::{Pusher, Channel, PusherError};
use sonic_rs::json;

# async fn doc_trigger_event(pusher: &Pusher) -> Result<(), PusherError> {
let channels = vec![Channel::from_string("my-channel")?]; // Use Channel type
let event_name = "new-message";
let data = json!({ "text": "Hello from Rust!" });

match pusher.trigger(&channels, event_name, data, None).await { // Pass data directly
    Ok(response) => {
        println!("Event triggered! Status: {}", response.status());
        // You might want to consume the response body, e.g., response.text().await?
    }
    Err(e) => eprintln!("Error triggering event: {:?}", e),
}
# Ok(())
# }
```


**Encrypted channels**
If `channels` contains a single encrypted channel (e.g. `"private-encrypted-mychannel"`) and you’ve set the `encryption_master_key` in the `Config`, the library will encrypt `data` automatically.

**Excluding a recipient**
```rust
use pushers::{Pusher, Channel, PusherError, events::TriggerParams};
use sonic_rs::json;

# async fn doc_trigger_event_exclude(pusher: &Pusher) -> Result<(), PusherError> {
let channels = vec![Channel::from_string("my-channel")?];
let event_name = "new-message";
let data = json!({ "text": "Hello from Rust!" });
let params = TriggerParams {
    socket_id: Some("socket_id_to_exclude".to_string()),
    info: None,
};

pusher
    .trigger(&channels, event_name, data, Some(params)) // Pass data directly
    .await?;
# Ok(())
# }
```


### 3. Triggering Batch Events

```rust
use pushers::{Pusher, PusherError, events::BatchEvent};
use sonic_rs::json;

# async fn doc_trigger_batch(pusher: &Pusher) -> Result<(), PusherError> {
let batch = vec![
    BatchEvent {
        name: "event1".to_string(),
        channel: "channel-a".to_string(),
        data: json!({ "value": 1 }).to_string(),
        socket_id: None,
        info: None,
        tags: None,
    },
    BatchEvent {
        name: "event2".to_string(),
        channel: "channel-b".to_string(),
        data: json!({ "value": 2 }).to_string(),
        socket_id: None,
        info: None,
        tags: None,
    },
];

match pusher.trigger_batch(batch).await {
    Ok(response) => println!("Batch triggered! Status: {}", response.status()),
    Err(e) => eprintln!("Error triggering batch: {:?}", e),
}
# Ok(())
# }
```

### 3.1. Triggering Events with Tag Filtering

Tag filtering allows you to add metadata tags to events, enabling clients to filter which events they receive based on tag values. This can significantly reduce bandwidth usage (60-90%) in high-volume scenarios.

**Triggering a single event with tags:**

```rust
use pushers::{Pusher, Channel, PusherError, events::TriggerParams};
use sonic_rs::json;
use std::collections::HashMap;

# async fn doc_trigger_with_tags(pusher: &Pusher) -> Result<(), PusherError> {
let channels = vec![Channel::from_string("sports-updates")?];
let event_name = "match-event";
let data = json!({
    "match_id": "123",
    "team": "Home",
    "player": "John Doe",
    "minute": 45
});

// Create tags for filtering
let mut tags = HashMap::new();
tags.insert("event_type".to_string(), "goal".to_string());
tags.insert("priority".to_string(), "high".to_string());

let params = TriggerParams::builder()
    .tags(tags)
    .build();

pusher
    .trigger(&channels, event_name, data, Some(params))
    .await?;
# Ok(())
# }
```

**Triggering batch events with tags:**

```rust
use pushers::{Pusher, PusherError, events::BatchEvent};
use sonic_rs::json;
use std::collections::HashMap;

# async fn doc_trigger_batch_with_tags(pusher: &Pusher) -> Result<(), PusherError> {
let mut goal_tags = HashMap::new();
goal_tags.insert("event_type".to_string(), "goal".to_string());
goal_tags.insert("priority".to_string(), "high".to_string());

let mut shot_tags = HashMap::new();
shot_tags.insert("event_type".to_string(), "shot".to_string());
shot_tags.insert("xG".to_string(), "0.85".to_string());

let batch = vec![
    BatchEvent::new("match-event", "match:123", json!({ "type": "goal", "player": "Smith" }))
        .with_tags(goal_tags),
    BatchEvent::new("match-event", "match:123", json!({ "type": "shot", "player": "Jones" }))
        .with_tags(shot_tags),
];

match pusher.trigger_batch(batch).await {
    Ok(response) => println!("Batch with tags triggered! Status: {}", response.status()),
    Err(e) => eprintln!("Error triggering batch: {:?}", e),
}
# Ok(())
# }
```

**Note:** Tag filtering must be enabled on the Sockudo server (`TAG_FILTERING_ENABLED=true`) for clients to filter events. Tags are key-value pairs where both keys and values are strings. Clients can subscribe with filter expressions to receive only events matching their criteria.


### 4. Authorizing Channels

Typically done in your HTTP handler when a client attempts to subscribe:

```rust
use pushers::{Pusher, Channel, PusherError};
use sonic_rs::json;

# fn doc_authorize_channel(pusher: &Pusher) -> Result<(), PusherError> {
// Example values from client
let socket_id = "123.456";
let channel_name_str = "private-mychannel";
let channel = Channel::from_string(channel_name_str)?;

// For presence channels, include user data:
let presence_data = Some(json!({
    "user_id": "unique_user_id",
    "user_info": { "name": "Alice" }
}));

match pusher.authorize_channel(
    socket_id,
    &channel, // Pass Channel struct
    presence_data.as_ref(),
) {
    Ok(auth_signature) => {
        println!("Auth success: {:?}", auth_signature);
        // Return `auth_signature` (which is a SocketAuth struct) as JSON to client
        // e.g., using Axum: Json(auth_signature)
    }
    Err(e) => eprintln!("Auth error: {:?}", e),
}
# Ok(())
# }
```


### 5. Authenticating Users

For server-to-user events:

```rust
use pushers::{Pusher, PusherError};
use sonic_rs::json;

# fn doc_authenticate_user(pusher: &Pusher) -> Result<(), PusherError> {
// Example values from client
let socket_id = "789.012";
let user_data = json!({
    "id": "user-bob",      // required
    "name": "Bob The Builder",
    "email": "bob@example.com"
});

match pusher.authenticate_user(socket_id, &user_data) {
    Ok(user_auth) => {
        println!("User auth success: {:?}", user_auth);
        // Return `user_auth` (which is a UserAuth struct) as JSON to client
    }
    Err(e) => eprintln!("User auth error: {:?}", e),
}
# Ok(())
# }
```


### 6. Sending an Event to a User

```rust
use pushers::{Pusher, PusherError};
use sonic_rs::json;

# async fn doc_send_to_user(pusher: &Pusher) -> Result<(), PusherError> {
let user_id = "user-bob";
let event_name = "personal-notification";
let data = json!({ "alert": "Your report is ready!" });

match pusher.send_to_user(user_id, event_name, data).await { // Pass data directly
    Ok(response) => println!("Sent to user! Status: {}", response.status()),
    Err(e) => eprintln!("Error sending to user: {:?}", e),
}
# Ok(())
# }
```


### 7. Terminating User Connections

```rust
use pushers::{Pusher, PusherError};

# async fn doc_terminate_user(pusher: &Pusher) -> Result<(), PusherError> {
let user_id = "user-charlie";

match pusher.terminate_user_connections(user_id).await {
    Ok(response) => println!("Terminate successful! Status: {}", response.status()),
    Err(e) => eprintln!("Error terminating user: {:?}", e),
}
# Ok(())
# }
```


### 8. Handling Webhooks

```rust
use pushers::{Pusher, PusherError, webhook::WebhookEvent};
use std::collections::BTreeMap;

# fn doc_handle_webhook(pusher: &Pusher) -> Result<(), PusherError> {
// Incoming request data (example)
let mut headers = BTreeMap::new();
headers.insert("X-Pusher-Key".to_string(), "YOUR_APP_KEY".to_string()); // From Config
headers.insert("X-Pusher-Signature".to_string(), "RECEIVED_SIGNATURE".to_string());
headers.insert("Content-Type".to_string(), "application/json".to_string()); // Important for validation

let body = r#"{
    "time_ms": 1600000000000,
    "events":[{"name":"channel_occupied","channel":"my-channel"}]
}"#;

let webhook = pusher.webhook(&headers, body);

// Optionally, provide a list of additional valid tokens if you manage multiple credentials
if webhook.is_valid(None) {
    println!("Webhook is valid!");
    match webhook.get_events() {
        Ok(events) => {
            println!("Events: {:?}", events);
            for event in events {
                match event {
                    WebhookEvent::ChannelOccupied { channel } => {
                        println!("Channel occupied: {}", channel);
                    }
                    // Handle other event types
                    _ => {}
                }
            }
        }
        Err(e) => eprintln!("Error getting events: {:?}", e),
    }

    match webhook.get_time() {
        Ok(time) => println!("Webhook time: {:?}", time),
        Err(e) => eprintln!("Error getting time: {:?}", e),
    }
} else {
    eprintln!("Invalid webhook!");
    // Return HTTP 401 Unauthorized
}
# Ok(())
# }
```


### 9. Example: Integration with Axum

```rust
use axum::{
    extract::{Json, State}, // Removed Extension as State is preferred for app state
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use pushers::{Config, Pusher, PusherError, Channel};
use serde::{Deserialize, Serialize};
use sonic_rs::{json, Value};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
struct AppState {
    pusher: Arc<Pusher>,
}

#[tokio::main]
async fn main() {
    // Remember to replace placeholders with your actual credentials and cluster
    let config = Config::builder()
        .app_id("YOUR_APP_ID")
        .key("YOUR_APP_KEY")
        .secret("YOUR_APP_SECRET")
        .cluster("YOUR_CLUSTER")
        .build()
        .expect("Failed to build Pusher config"); // Handle potential error

    let pusher = Arc::new(Pusher::new(config).expect("Failed to create Pusher client"));

    let app_state = AppState { pusher };

    let app = Router::new()
        .route("/pusher/auth", post(pusher_auth_handler))
        .route("/pusher/webhook", post(pusher_webhook_handler))
        .with_state(app_state); // Use with_state with the created AppState

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize)]
struct AuthRequest {
    socket_id: String,
    channel_name: String,
    #[serde(alias = "channel_data")] // Pusher clients might send presence data as channel_data
    presence_data: Option<Value>,
}

#[derive(Serialize)]
struct AuthResponseError {
    error: String,
}

async fn pusher_auth_handler(
    State(state): State<AppState>,
    Json(payload): Json<AuthRequest>,
) -> impl IntoResponse {
    // It's good practice to parse the channel name into the Channel struct
    let channel = match Channel::from_string(&payload.channel_name) {
        Ok(ch) => ch,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid channel_name"})),
            )
                .into_response();
        }
    };

    match state.pusher.authorize_channel(
        &payload.socket_id,
        &channel, // Pass the parsed Channel struct
        payload.presence_data.as_ref(),
    ) {
        Ok(auth_response) => (StatusCode::OK, Json(auth_response)).into_response(),
        Err(e) => {
            eprintln!("Auth error: {:?}", e); // Log the error
            (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "Forbidden" })),
            )
                .into_response();
        }
    }
}

async fn pusher_webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String, // Axum extracts the body as String by default
) -> impl IntoResponse {
    let mut hdrs_btreemap = BTreeMap::new();
    for (k, v) in headers.iter() {
        if let Ok(s) = v.to_str() {
            // Store header names as they are, or normalize if Pusher requires specific casing
            hdrs_btreemap.insert(k.as_str().to_string(), s.to_string());
        }
    }

    let webhook = state.pusher.webhook(&hdrs_btreemap, &body);

    // It's crucial to check `X-Pusher-Key` and `X-Pusher-Signature` exist and match.
    // The `is_valid` method handles signature verification.
    if webhook.is_valid(None) { // `None` for extra_tokens unless you have a specific setup
        println!("Webhook validated successfully.");
        // Process webhook.get_events() or webhook.get_data() here
        // For example:
        // if let Ok(events) = webhook.get_events() {
        //     for event in events {
        //         println!("Received event: {:?}", event.event_name());
        //     }
        // }
        (StatusCode::OK, Json(json!({ "status": "ok" }))).into_response()
    } else {
        eprintln!("Webhook validation failed. Key: {:?}, Signature: {:?}, Body: {}", webhook.key(), webhook.signature(), webhook.body());
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "Unauthorized" })),
        )
            .into_response()
    }
}
```


## Configuration Options

The `Config` struct is used to configure the Pusher client. You can create it using `Config::builder()`:

-   `app_id(id: impl Into<String>)`
-   `key(key: impl Into<String>)`
-   `secret(secret: impl Into<String>)`
-   `cluster(name: impl AsRef<str>)`: Sets the cluster (e.g., `"eu"`, `"ap1"`). This correctly sets the host to `api-{cluster}.pusher.com`.
-   `host(host: impl Into<String>)`: Sets a custom host if not using a standard cluster.
-   `use_tls(bool)`: Enable HTTPS (default `true`). `ConfigBuilder` sets scheme to "https" by default, this method can override it.
-   `port(number: u16)`: Custom port.
-   `timeout(duration: Duration)`: HTTP request timeout.
-   `encryption_master_key(key: Vec<u8>)`: Sets the 32-byte encryption master key from raw bytes.
-   `encryption_master_key_base64(key: impl AsRef<str>)`: Sets the 32-byte encryption master key from a base64 encoded string.
-   `pool_max_idle_per_host(max: usize)`: Sets the maximum number of idle connections per host for the underlying HTTP client.
-   `enable_retry(enable: bool)`: Enables or disables retry logic for failed requests (default `true`).
-   `max_retries(max: u32)`: Sets the maximum number of retries for failed requests if retry is enabled (default `3`).

Finally, call `.build()` on the `ConfigBuilder` to get a `Result<Config, PusherError>`.

## Error Handling

All fallible methods in this library return `Result<T, PusherError>`.
The `PusherError` enum has the following variants:

- `Request(RequestError)`: Errors related to making HTTP requests (e.g., network issues, non-success status codes). `RequestError` contains `message`, `url`, `status`, and `body`.
- `Webhook(WebhookError)`: Errors during webhook processing, such as signature validation failure or invalid body. `WebhookError` contains `message`, `content_type`, `body`, and `signature`.
- `Config { message: String }`: Errors due to invalid client configuration (e.g., missing app ID, invalid encryption key).
- `Validation { message: String }`: Errors from input validation (e.g., invalid channel name, socket ID, event name too long).
- `Encryption { message: String }`: Errors related to data encryption or decryption for end-to-end encrypted channels.
- `Json(serde_json::Error)`: Errors during JSON serialization or deserialization.
- `Http(reqwest::Error)`: Underlying errors from the `reqwest` HTTP client library.

## Contributing

Contributions are welcome! Please open issues for bugs or feature requests, or submit pull requests for improvements.
For major changes, please discuss these via an issue first to ensure alignment.

When contributing code, please ensure:
- Code is formatted with `cargo fmt`.
- Clippy lints are addressed (`cargo clippy --all-targets --all-features`).
- New functionality is covered by tests.
- Documentation is updated accordingly.

## License

This project is licensed under the GNU Affero General Public License v3.0. See the `LICENSE.md` file for details.

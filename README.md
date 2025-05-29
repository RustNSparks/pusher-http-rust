# Pusher HTTP Rust Client

A Rust client for interacting with the Pusher HTTP API, allowing you to publish events, authorize channels, authenticate users, and handle webhooks from your Rust applications.

## Features

- Trigger events on public, private, and presence channels  
- Trigger events to specific users (User Authentication)  
- Trigger batch events for efficiency  
- Support for end-to-end encrypted channels  
- Authorize client subscriptions to private, presence, and encrypted channels  
- Authenticate users for user-specific Pusher features  
- Terminate user connections  
- Validate and process incoming Pusher webhooks  
- Configurable host, port, scheme (HTTP/HTTPS), and timeout  
- Asynchronous API using `async/await`  
- Typed responses and errors  

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
# If publishing to crates.io:
# pusher-http-rust = "0.1.0" # Replace with the desired version
# Or, for local development:
# pusher-http-rust = { path = "../path/to/pusher-http-rust" }

serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
# reqwest is used internally but you might need it for response handling
# reqwest = { version = "0.11", features = ["json"] }
```

Then run:

```bash
cargo build
```

## Usage

### 1. Initialization

Configure and create a `Pusher` client:

```rust
use pusher_http_rust::{Config, Pusher, PusherError};

#[tokio::main]
async fn main() -> Result<(), PusherError> {
    let config = Config::new("YOUR_APP_ID", "YOUR_APP_KEY", "YOUR_APP_SECRET")
        .cluster("YOUR_CLUSTER")            // e.g. "eu", "ap1"
        .timeout(std::time::Duration::from_secs(5)); // Optional

    // For encrypted channels:
    // let config = config
    //     .encryption_master_key_base64("YOUR_BASE64_ENCRYPTION_MASTER_KEY")?;

    let pusher = Pusher::new(config);

    // Your application logic here...
    Ok(())
}
```

You can also initialize from a Pusher URL:

```rust
let pusher_from_url = Pusher::from_url(
    "http://KEY:SECRET@api-CLUSTER.pusher.com/apps/APP_ID",
    None,
)?;
```

### 2. Triggering Events

```rust
use serde_json::json;

let channels = vec!["my-channel".to_string()];
let event_name = "new-message";
let data = json!({ "text": "Hello from Rust!" });

match pusher.trigger(&channels, event_name, &data, None).await {
    Ok(response) => {
        println!("Event triggered! Status: {}", response.status());
    }
    Err(e) => eprintln!("Error triggering event: {:?}", e),
}
```

**Encrypted channels**  
If `channels` contains a single encrypted channel (e.g. `"private-encrypted-mychannel"`) and you’ve set the `encryption_master_key`, the library will encrypt `data` automatically.

**Excluding a recipient**  
```rust
use pusher_http_rust::events::TriggerParams;

let params = TriggerParams {
    socket_id: Some("socket_id_to_exclude".to_string()),
    info: None,
};

pusher
    .trigger(&channels, event_name, &data, Some(params))
    .await?;
```

### 3. Triggering Batch Events

```rust
use pusher_http_rust::events::BatchEvent;
use serde_json::json;

let batch = vec![
    BatchEvent {
        channel: "channel-a".to_string(),
        name: "event1".to_string(),
        data: json!({ "value": 1 }).to_string(),
        socket_id: None,
        info: None,
    },
    BatchEvent {
        channel: "channel-b".to_string(),
        name: "event2".to_string(),
        data: json!({ "value": 2 }).to_string(),
        socket_id: None,
        info: None,
    },
];

match pusher.trigger_batch(batch).await {
    Ok(response) => println!("Batch triggered! Status: {}", response.status()),
    Err(e) => eprintln!("Error triggering batch: {:?}", e),
}
```

### 4. Authorizing Channels

Typically done in your HTTP handler when a client attempts to subscribe:

```rust
use serde_json::json;

// Example values from client
let socket_id = "123.456";
let channel_name = "private-mychannel";

// For presence channels, include user data:
let presence_data = Some(json!({
    "user_id": "unique_user_id",
    "user_info": { "name": "Alice" }
}));

match pusher.authorize_channel(
    &socket_id,
    &channel_name,
    presence_data.as_ref(),
) {
    Ok(auth_signature) => {
        println!("Auth success: {:?}", auth_signature);
        // Return `auth_signature` as JSON to client
    }
    Err(e) => eprintln!("Auth error: {:?}", e),
}
```

### 5. Authenticating Users

For server-to-user events:

```rust
use serde_json::json;

// Example values from client
let socket_id = "789.012";
let user_data = json!({
    "id": "user-bob",      // required
    "name": "Bob The Builder",
    "email": "bob@example.com"
});

match pusher.authenticate_user(&socket_id, &user_data) {
    Ok(user_auth) => println!("User auth success: {:?}", user_auth),
    Err(e) => eprintln!("User auth error: {:?}", e),
}
```

### 6. Sending an Event to a User

```rust
let user_id = "user-bob";
let event_name = "personal-notification";
let data = json!({ "alert": "Your report is ready!" });

match pusher.send_to_user(user_id, event_name, &data).await {
    Ok(response) => println!("Sent to user! Status: {}", response.status()),
    Err(e) => eprintln!("Error sending to user: {:?}", e),
}
```

### 7. Terminating User Connections

```rust
let user_id = "user-charlie";

match pusher.terminate_user_connections(user_id).await {
    Ok(response) => println!("Terminate successful! Status: {}", response.status()),
    Err(e) => eprintln!("Error terminating user: {:?}", e),
}
```

### 8. Handling Webhooks

```rust
use std::collections::BTreeMap;

// Incoming request data (example)
let mut headers = BTreeMap::new();
headers.insert("X-Pusher-Key".to_string(), "YOUR_APP_KEY".to_string());
headers.insert("X-Pusher-Signature".to_string(), "RECEIVED_SIGNATURE".to_string());

let body = r#"{
    "time_ms": 1600000000000,
    "events":[{"name":"channel_occupied","channel":"my-channel"}]
}"#;

let webhook = pusher.webhook(&headers, body);

if webhook.is_valid(None) {
    println!("Webhook is valid!");
    let events = webhook.get_events().unwrap();
    println!("Events: {:?}", events);

    let time = webhook.get_time().unwrap();
    println!("Webhook time: {:?}", time);
} else {
    eprintln!("Invalid webhook!");
    // Return HTTP 401 Unauthorized
}
```

### 9. Example: Integration with Axum

```rust
use axum::{
    extract::{Extension, Json, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use pusher_http_rust::{Config, Pusher, auth, webhook::Webhook};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
struct AppState {
    pusher: Arc<Pusher>,
}

#[tokio::main]
async fn main() {
    let config = Config::new("APP_ID", "APP_KEY", "APP_SECRET").cluster("CLUSTER");
    let pusher = Arc::new(Pusher::new(config));

    let app = Router::new()
        .route("/pusher/auth", post(pusher_auth_handler))
        .route("/pusher/webhook", post(pusher_webhook_handler))
        .with_state(AppState { pusher });

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Deserialize)]
struct AuthRequest {
    socket_id: String,
    channel_name: String,
    presence_data: Option<Value>,
}

async fn pusher_auth_handler(
    State(state): State<AppState>,
    Json(payload): Json<AuthRequest>,
) -> impl IntoResponse {
    match state.pusher.authorize_channel(
        &payload.socket_id,
        &payload.channel_name,
        payload.presence_data.as_ref(),
    ) {
        Ok(auth) => (StatusCode::OK, Json(auth)).into_response(),
        Err(_) => (StatusCode::FORBIDDEN, Json(json!({ "error": "Forbidden" }))).into_response(),
    }
}

async fn pusher_webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    let mut hdrs = BTreeMap::new();
    for (k, v) in headers.iter() {
        if let Ok(s) = v.to_str() {
            hdrs.insert(k.to_string(), s.to_string());
        }
    }

    let webhook = state.pusher.webhook(&hdrs, &body);
    if webhook.is_valid(None) {
        (StatusCode::OK, Json(json!({ "status": "ok" }))).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, Json(json!({ "error": "Unauthorized" }))).into_response()
    }
}
```

## Configuration Options

`Config` methods:

- `new(app_id, key, secret)` — basic initialization  
- `cluster(name)` — set cluster (e.g. `"eu"`)  
- `use_tls(bool)` — enable HTTPS (default `true`)  
- `port(number)` — custom port  
- `timeout(Duration)` — HTTP request timeout  
- `encryption_master_key_base64(key)` — 32-byte base64 key for encrypted channels  

## Error Handling

All fallible methods return `Result<T, PusherError>`.  
`PusherError` variants:

- `Request` — HTTP request errors  
- `Webhook` — webhook processing errors  
- `Config` — invalid configuration  
- `Validation` — input validation errors  
- `Encryption` — encryption/decryption errors  
- `Json` — `serde_json` errors  
- `Http` — `reqwest` errors  

## Contributing

Contributions are welcome! Please open issues or pull requests.  
For major changes, please discuss via issue first.

## License

This project is licensed under the GNU Affero General Public License v3.0 License. See `LICENSE.md` for details.

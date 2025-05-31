use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crate::{Pusher, PusherError, Result, Channel};
use std::fmt;
use std::sync::Once;

static SODIUM_INIT: Once = Once::new();

/// Initialize sodiumoxide once
fn init_sodium() -> Result<()> {
    SODIUM_INIT.call_once(|| {
        sodiumoxide::init().expect("Failed to initialize sodiumoxide");
    });
    Ok(())
}

/// Event data that can be either a string or JSON
#[derive(Debug, Clone, PartialEq)]
pub enum EventData {
    String(String),
    Json(Value),
}

impl EventData {
    /// Creates event data from a string
    pub fn from_string(s: impl Into<String>) -> Self {
        EventData::String(s.into())
    }

    /// Creates event data from a JSON value
    pub fn from_json(value: Value) -> Self {
        EventData::Json(value)
    }

    /// Converts the event data to a string for transmission
    pub fn to_string(&self) -> String {
        match self {
            EventData::String(s) => s.clone(),
            EventData::Json(v) => serde_json::to_string(v).unwrap_or_default(),
        }
    }

    /// Gets the event data as a JSON value
    pub fn as_json(&self) -> Result<Value> {
        match self {
            EventData::String(s) => serde_json::from_str(s)
                .map_err(|e| PusherError::Json(e)),
            EventData::Json(v) => Ok(v.clone()),
        }
    }
}

impl fmt::Display for EventData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl From<String> for EventData {
    fn from(s: String) -> Self {
        EventData::String(s)
    }
}

impl From<&str> for EventData {
    fn from(s: &str) -> Self {
        EventData::String(s.to_string())
    }
}

impl From<Value> for EventData {
    fn from(v: Value) -> Self {
        EventData::Json(v)
    }
}

// impl<T: Serialize> From<&T> for EventData {
//     fn from(v: &T) -> Self {
//         match serde_json::to_value(v) {
//             Ok(value) => EventData::Json(value),
//             Err(_) => EventData::String(format!("{:?}", v)),
//         }
//     }
// }

/// Event data for triggering
#[derive(Debug, Serialize)]
pub struct Event {
    pub name: String,
    pub data: String,
    pub channels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<String>,
}

/// Batch event data
#[derive(Debug, Serialize, Deserialize)]
pub struct BatchEvent {
    pub name: String,
    pub channel: String,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<String>,
}

impl BatchEvent {
    /// Creates a new batch event with EventData
    pub fn new(
        name: impl Into<String>,
        channel: impl Into<String>,
        data: impl Into<EventData>,
    ) -> Self {
        Self {
            name: name.into(),
            channel: channel.into(),
            data: data.into().to_string(),
            socket_id: None,
            info: None,
        }
    }

    /// Sets the socket ID to exclude
    pub fn with_socket_id(mut self, socket_id: impl Into<String>) -> Self {
        self.socket_id = Some(socket_id.into());
        self
    }

    /// Sets the info parameter
    pub fn with_info(mut self, info: impl Into<String>) -> Self {
        self.info = Some(info.into());
        self
    }
}

/// Parameters for triggering events
#[derive(Debug, Clone, Default)]
pub struct TriggerParams {
    pub socket_id: Option<String>,
    pub info: Option<String>,
}

impl TriggerParams {
    /// Creates a new TriggerParams builder
    pub fn builder() -> TriggerParamsBuilder {
        TriggerParamsBuilder::default()
    }
}

/// Builder for TriggerParams
#[derive(Debug, Default)]
pub struct TriggerParamsBuilder {
    socket_id: Option<String>,
    info: Option<String>,
}

impl TriggerParamsBuilder {
    /// Sets the socket ID to exclude
    pub fn socket_id(mut self, socket_id: impl Into<String>) -> Self {
        self.socket_id = Some(socket_id.into());
        self
    }

    /// Sets the info parameter
    pub fn info(mut self, info: impl Into<String>) -> Self {
        self.info = Some(info.into());
        self
    }

    /// Builds the TriggerParams
    pub fn build(self) -> TriggerParams {
        TriggerParams {
            socket_id: self.socket_id,
            info: self.info,
        }
    }
}

/// Encrypts data for encrypted channels
fn encrypt(pusher: &Pusher, channel: &str, data: &EventData) -> Result<String> {
    init_sodium()?;

    // Ensure master key is present
    let _master_key = pusher.config().encryption_master_key()
        .ok_or_else(|| PusherError::Encryption {
            message: "Set encryptionMasterKey before triggering events on encrypted channels".to_string(),
        })?;

    // Generate a random nonce
    let nonce_bytes = sodiumoxide::randombytes::randombytes(sodiumoxide::crypto::secretbox::NONCEBYTES);
    let nonce = sodiumoxide::crypto::secretbox::Nonce::from_slice(&nonce_bytes)
        .ok_or_else(|| PusherError::Encryption {
            message: "Failed to create nonce from random bytes".to_string(),
        })?;

    // Get channel shared secret
    let shared_secret_bytes = pusher.channel_shared_secret(channel)?;

    // Convert to cryptographic Key type
    let key = sodiumoxide::crypto::secretbox::Key::from_slice(&shared_secret_bytes)
        .ok_or_else(|| PusherError::Encryption {
            message: format!(
                "Channel shared secret must be {} bytes long, but was {} bytes.",
                sodiumoxide::crypto::secretbox::KEYBYTES,
                shared_secret_bytes.len()
            ),
        })?;

    // Get data as bytes
    let data_string = data.to_string();
    let data_bytes = data_string.as_bytes();

    // Encrypt the data
    let ciphertext = sodiumoxide::crypto::secretbox::seal(data_bytes, &nonce, &key);

    // Return encrypted payload as JSON string
    let encrypted_payload = json!({
        "nonce": BASE64.encode(nonce.as_ref()),
        "ciphertext": BASE64.encode(&ciphertext),
    });

    Ok(serde_json::to_string(&encrypted_payload)?)
}

/// Triggers an event on channels
pub async fn trigger<D: Into<EventData>>(
    pusher: &Pusher,
    channels: &[Channel],
    event_name: impl AsRef<str>,
    data: D,
    params: Option<&TriggerParams>,
) -> Result<reqwest::Response> {
    let data = data.into();
    let event_name = event_name.as_ref();
    
    // Validate event name
    if event_name.len() > 200 {
        return Err(PusherError::Validation {
            message: format!("Event name too long: '{}' (max 200 characters)", event_name),
        });
    }

    // Convert channels to strings
    let channel_strings: Vec<String> = channels.iter()
        .map(|c| c.full_name())
        .collect();

    if channels.len() == 1 && channels[0].is_encrypted() {
        let encrypted_data = encrypt(pusher, &channel_strings[0], &data)?;
        
        let mut event = Event {
            name: event_name.to_string(),
            data: encrypted_data,
            channels: channel_strings,
            socket_id: None,
            info: None,
        };

        if let Some(params) = params {
            event.socket_id = params.socket_id.clone();
            event.info = params.info.clone();
        }

        let event_json = serde_json::to_value(event)?;
        pusher.post("/events", &event_json).await
    } else {
        // Check for encrypted channels in multi-channel trigger
        for channel in channels {
            if channel.is_encrypted() {
                return Err(PusherError::Validation {
                    message: "You cannot trigger to multiple channels when using encrypted channels".to_string(),
                });
            }
        }

        let mut event = Event {
            name: event_name.to_string(),
            data: data.to_string(),
            channels: channel_strings,
            socket_id: None,
            info: None,
        };

        if let Some(params) = params {
            event.socket_id = params.socket_id.clone();
            event.info = params.info.clone();
        }

        let event_json = serde_json::to_value(event)?;
        pusher.post("/events", &event_json).await
    }
}

/// Triggers an event on channel names (backward compatibility)
pub async fn trigger_on_channels<D: Into<EventData>>(
    pusher: &Pusher,
    channels: &[String],
    event_name: impl AsRef<str>,
    data: D,
    params: Option<&TriggerParams>,
) -> Result<reqwest::Response> {
    let channels: Result<Vec<Channel>> = channels.iter()
        .map(|c| Channel::from_string(c))
        .collect();
    let channels = channels?;
    trigger(pusher, &channels, event_name, data, params).await
}

/// Triggers a batch of events
pub async fn trigger_batch(
    pusher: &Pusher,
    mut batch: Vec<BatchEvent>,
) -> Result<reqwest::Response> {
    // Validate batch size
    if batch.is_empty() {
        return Err(PusherError::Validation {
            message: "Batch cannot be empty".to_string(),
        });
    }

    if batch.len() > 10 {
        return Err(PusherError::Validation {
            message: format!("Batch too large: {} events (max 10)", batch.len()),
        });
    }

    // Encrypt data for encrypted channels
    for event in &mut batch {
        let channel = Channel::from_string(&event.channel)?;
        if channel.is_encrypted() {
            let data = EventData::String(event.data.clone());
            event.data = encrypt(pusher, &event.channel, &data)?;
        }
    }

    let batch_payload = json!({ "batch": batch });
    pusher.post("/batch_events", &batch_payload).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_event_data_conversions() {
        // Test string
        let data = EventData::from_string("hello");
        assert_eq!(data.to_string(), "hello");

        // Test JSON
        let json_data = json!({"key": "value"});
        let data = EventData::from_json(json_data.clone());
        assert_eq!(data.as_json().unwrap(), json_data);

        // Test From implementations
        let data: EventData = "test".into();
        assert!(matches!(data, EventData::String(_)));

        let data: EventData = json!({"test": 123}).into();
        assert!(matches!(data, EventData::Json(_)));
    }

    #[test]
    fn test_batch_event_builder() {
        let event = BatchEvent::new("test-event", "test-channel", "test-data")
            .with_socket_id("123.456")
            .with_info("test-info");

        assert_eq!(event.name, "test-event");
        assert_eq!(event.channel, "test-channel");
        assert_eq!(event.data, "test-data");
        assert_eq!(event.socket_id, Some("123.456".to_string()));
        assert_eq!(event.info, Some("test-info".to_string()));
    }

    #[test]
    fn test_trigger_params_builder() {
        let params = TriggerParams::builder()
            .socket_id("123.456")
            .info("test-info")
            .build();

        assert_eq!(params.socket_id, Some("123.456".to_string()));
        assert_eq!(params.info, Some("test-info".to_string()));
    }
}
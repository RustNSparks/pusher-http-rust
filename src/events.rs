use serde_json::{json, Value};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crate::{util, Pusher, PusherError, Result};

/// Event data for triggering
#[derive(Debug, serde::Serialize)]
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
#[derive(Debug, serde::Serialize)]
pub struct BatchEvent {
    pub name: String,
    pub channel: String,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<String>,
}

/// Encrypts data for encrypted channels
fn encrypt(pusher: &Pusher, channel: &str, data: &Value) -> Result<String> {
    // 1. Ensure master key is present (though not directly used here, it's good practice
    //    to ensure it's configured if channel_shared_secret relies on it).
    let _master_key = pusher.config().encryption_master_key.as_ref()
        .ok_or_else(|| PusherError::Encryption {
            message: "Set encryptionMasterKey before triggering events on encrypted channels".to_string(),
        })?;

    // 2. Generate a random nonce.
    // For secretbox (XSalsa20Poly1305), the nonce is 24 bytes.
    let nonce_bytes = sodiumoxide::randombytes::randombytes(sodiumoxide::crypto::secretbox::NONCEBYTES);
    let nonce = sodiumoxide::crypto::secretbox::Nonce::from_slice(&nonce_bytes)
        .ok_or_else(|| PusherError::Encryption {
            message: "Failed to create nonce from random bytes (should not happen)".to_string(),
        })?;

    // 3. Get channel shared secret (raw key bytes).
    // This function is assumed to return the correct length (32 bytes for secretbox).
    let shared_secret_bytes = pusher.channel_shared_secret(channel)?;

    // 4. Convert shared secret bytes to a cryptographic Key type.
    // This step validates the key length.
    let key = sodiumoxide::crypto::secretbox::Key::from_slice(&shared_secret_bytes)
        .ok_or_else(|| PusherError::Encryption {
            message: format!(
                "Channel shared secret must be {} bytes long, but was {} bytes.",
                sodiumoxide::crypto::secretbox::KEYBYTES,
                shared_secret_bytes.len()
            ),
        })?;

    // 5. Serialize data to a string (then to bytes).
    let data_string = serde_json::to_string(data)?;
    let data_bytes = data_string.as_bytes();

    // 6. Encrypt the data.
    // `secretbox::seal` performs authenticated encryption (XSalsa20Poly1305).
    let ciphertext = sodiumoxide::crypto::secretbox::seal(data_bytes, &nonce, &key);

    // 7. Return encrypted payload as a JSON string.
    // The nonce and ciphertext are Base64 encoded.
    let encrypted_payload = serde_json::json!({
        "nonce": BASE64.encode(nonce.as_ref()), // nonce.as_ref() gives &[u8]
        "ciphertext": BASE64.encode(&ciphertext),
    });

    Ok(serde_json::to_string(&encrypted_payload)?)
}

/// Ensures data is JSON string
fn ensure_json(data: &Value) -> String {
    match data {
        Value::String(s) => s.clone(),
        _ => serde_json::to_string(data).unwrap_or_default(),
    }
}

/// Triggers an event on channels
pub async fn trigger(
    pusher: &Pusher,
    channels: &[String],
    event_name: &str,
    data: &Value,
    params: Option<&TriggerParams>,
) -> crate::Result<reqwest::Response> {
    if channels.len() == 1 && util::is_encrypted_channel(&channels[0]) {
        let channel = &channels[0];
        let encrypted_data = encrypt(pusher, channel, data)?;
        
        let mut event = Event {
            name: event_name.to_string(),
            data: encrypted_data,
            channels: vec![channel.clone()],
            socket_id: None,
            info: None,
        };

        if let Some(params) = params {
            event.socket_id = params.socket_id.clone();
            event.info = params.info.clone();
        }
        let event = serde_json::to_value(event).unwrap();
        pusher.post("/events", &event).await
    } else {
        // Check for encrypted channels in multi-channel trigger
        for channel in channels {
            if util::is_encrypted_channel(channel) {
                return Err(PusherError::Validation {
                    message: "You cannot trigger to multiple channels when using encrypted channels".to_string(),
                });
            }
        }

        let mut event = Event {
            name: event_name.to_string(),
            data: ensure_json(data),
            channels: channels.to_vec(),
            socket_id: None,
            info: None,
        };

        if let Some(params) = params {
            event.socket_id = params.socket_id.clone();
            event.info = params.info.clone();
        }
        let event = serde_json::to_value(event).unwrap();
        pusher.post("/events", &event).await
    }
}

/// Parameters for triggering events
#[derive(Debug, Clone)]
pub struct TriggerParams {
    pub socket_id: Option<String>,
    pub info: Option<String>,
}

/// Triggers a batch of events
pub async fn trigger_batch(
    pusher: &Pusher,
    mut batch: Vec<BatchEvent>,
) -> crate::Result<reqwest::Response> {
    for event in &mut batch {
        if util::is_encrypted_channel(&event.channel) {
            let data: Value = serde_json::from_str(&event.data)?;
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
    fn test_ensure_json() {
        let string_val = json!("test");
        assert_eq!(ensure_json(&string_val), "test");

        let object_val = json!({"key": "value"});
        assert_eq!(ensure_json(&object_val), r#"{"key":"value"}"#);
    }
}

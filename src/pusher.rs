use std::collections::BTreeMap;
use reqwest::Client;
use serde_json::{json, Value};
use sha2::{Sha256, Digest};
use crate::{
    Config, Token, auth, events, util, webhook::Webhook,
    PusherError, RequestError, Result,
};

/// Main Pusher client
#[derive(Clone)]
pub struct Pusher {
    config: Config,
    client: Client,
}

impl Pusher {
    /// Creates a new Pusher client
    pub fn new(config: Config) -> Self {
        let mut client_builder = Client::builder();
        
        if let Some(timeout) = config.timeout {
            client_builder = client_builder.timeout(timeout);
        }
        
        let client = client_builder.build().unwrap();
        
        Self { config, client }
    }

    /// Creates a Pusher client from URL
    pub fn from_url(url: &str, additional_config: Option<Config>) -> Result<Self> {
        let parsed_url = url::Url::parse(url)
            .map_err(|_| PusherError::Config {
                message: "Invalid Pusher URL".to_string(),
            })?;

        let auth_parts: Vec<&str> = parsed_url.username().split(':').collect();
        if auth_parts.len() != 2 {
            return Err(PusherError::Config {
                message: "Invalid auth format in URL".to_string(),
            });
        }

        let path_segments: Vec<&str> = parsed_url.path().split('/').collect();
        let app_id = path_segments.last()
            .ok_or_else(|| PusherError::Config {
                message: "App ID not found in URL".to_string(),
            })?;

        let mut config = additional_config.unwrap_or_default();
        config.scheme = parsed_url.scheme().to_string();
        config.host = parsed_url.host_str().unwrap_or("api.pusherapp.com").to_string();
        config.port = parsed_url.port();
        config.app_id = app_id.to_string();
        config.token = Token::new(auth_parts[0], auth_parts[1]);

        Ok(Self::new(config))
    }

    /// Creates a Pusher client for a specific cluster
    pub fn for_cluster(cluster: &str, config: Config) -> Self {
        let config = config.cluster(cluster);
        Self::new(config)
    }

    /// Gets the configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Authorizes a channel
    pub fn authorize_channel(
        &self,
        socket_id: &str,
        channel: &str,
        data: Option<&Value>,
    ) -> Result<auth::SocketAuth> {
        util::validate_socket_id(socket_id)?;
        util::validate_channel(channel)?;

        auth::get_socket_signature(self, &self.config.token, channel, socket_id, data)
    }

    /// Authenticates a user
    pub fn authenticate_user(
        &self,
        socket_id: &str,
        user_data: &Value,
    ) -> Result<auth::UserAuth> {
        util::validate_socket_id(socket_id)?;
        
        // Validate user data has ID
        if let Some(id) = user_data.get("id") {
            if let Some(id_str) = id.as_str() {
                util::validate_user_id(id_str)?;
            } else {
                return Err(PusherError::Validation {
                    message: "User data ID must be a string".to_string(),
                });
            }
        } else {
            return Err(PusherError::Validation {
                message: "User data must contain an 'id' field".to_string(),
            });
        }

        auth::get_socket_signature_for_user(&self.config.token, socket_id, user_data)
    }

    /// Sends an event to a user
    pub async fn send_to_user(
        &self,
        user_id: &str,
        event: &str,
        data: &Value,
    ) -> Result<reqwest::Response> {
        if event.len() > 200 {
            return Err(PusherError::Validation {
                message: format!("Event name too long: '{}'", event),
            });
        }
        
        util::validate_user_id(user_id)?;
        
        let channel = format!("#server-to-user-{}", user_id);
        events::trigger(self, &[channel], event, data, None).await
    }

    /// Terminates user connections
    pub async fn terminate_user_connections(&self, user_id: &str) -> Result<reqwest::Response> {
        util::validate_user_id(user_id)?;
        let path = format!("/users/{}/terminate_connections", user_id);
        self.post(&path, &json!({})).await
    }

    /// Triggers an event
    pub async fn trigger(
        &self,
        channels: &[String],
        event: &str,
        data: &Value,
        params: Option<events::TriggerParams>,
    ) -> Result<reqwest::Response> {
        if let Some(ref params) = params {
            if let Some(ref socket_id) = params.socket_id {
                util::validate_socket_id(socket_id)?;
            }
        }

        if event.len() > 200 {
            return Err(PusherError::Validation {
                message: format!("Event name too long: '{}'", event),
            });
        }

        if channels.len() > 100 {
            return Err(PusherError::Validation {
                message: "Can't trigger a message to more than 100 channels".to_string(),
            });
        }

        for channel in channels {
            util::validate_channel(channel)?;
        }

        events::trigger(self, channels, event, data, params.as_ref()).await
    }

    /// Triggers a batch of events
    pub async fn trigger_batch(
        &self,
        batch: Vec<events::BatchEvent>,
    ) -> Result<reqwest::Response> {
        events::trigger_batch(self, batch).await
    }

    /// Makes a POST request
    pub async fn post(&self, path: &str, body: &Value) -> Result<reqwest::Response> {
        self.send_request("POST", path, Some(body), None).await
    }

    /// Makes a GET request
    pub async fn get(
        &self,
        path: &str,
        params: Option<&BTreeMap<String, String>>,
    ) -> Result<reqwest::Response> {
        self.send_request("GET", path, None, params).await
    }

    /// Creates a webhook from request data
    pub fn webhook(&self, headers: &BTreeMap<String, String>, body: &str) -> Webhook {
        Webhook::new(&self.config.token, headers, body)
    }

    /// Generates channel shared secret for encryption
    pub fn channel_shared_secret(&self, channel: &str) -> Result<[u8; 32]> {
        let master_key = self.config.encryption_master_key.as_ref()
            .ok_or_else(|| PusherError::Encryption {
                message: "Encryption master key not set".to_string(),
            })?;

        let mut hasher = Sha256::new();
        hasher.update(channel.as_bytes());
        hasher.update(master_key);
        
        let result = hasher.finalize();
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&result);
        Ok(secret)
    }

    /// Creates signed query string for manual requests
    pub fn create_signed_query_string(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
        params: Option<&BTreeMap<String, String>>,
    ) -> String {
        create_signed_query_string(&self.config.token, method, path, body, params)
    }

    /// Internal method to send HTTP requests
    async fn send_request(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        params: Option<&BTreeMap<String, String>>,
    ) -> Result<reqwest::Response> {
        let full_path = self.config.prefix_path(path);
        let body_str = body.map(|b| serde_json::to_string(b)).transpose()?;
        
        let query_string = create_signed_query_string(
            &self.config.token,
            method,
            &full_path,
            body_str.as_deref(),
            params,
        );
        
        let url = format!("{}{}?{}", self.config.base_url(), full_path, query_string);
        
        let mut request = match method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            _ => return Err(PusherError::Request(RequestError::new(
                "Unsupported HTTP method",
                &url,
                None,
                None,
            ))),
        };

        if let Some(body_str) = body_str {
            request = request
                .header("Content-Type", "application/json")
                .body(body_str);
        }

        let response = request
            .header("X-Pusher-Library", "pusher-rust/0.1.0")
            .send()
            .await?;

        if response.status().is_client_error() || response.status().is_server_error() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(PusherError::Request(RequestError::new(
                format!("HTTP {}", status),
                &url,
                Some(status),
                Some(body),
            )));
        }

        Ok(response)
    }
}

/// Creates a signed query string for Pusher API requests
fn create_signed_query_string(
    token: &Token,
    method: &str,
    path: &str,
    body: Option<&str>,
    params: Option<&BTreeMap<String, String>>,
) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut query_params = BTreeMap::new();
    query_params.insert("auth_key".to_string(), token.key.clone());
    query_params.insert("auth_timestamp".to_string(), timestamp.to_string());
    query_params.insert("auth_version".to_string(), "1.0".to_string());

    if let Some(body) = body {
        query_params.insert("body_md5".to_string(), util::get_md5(body));
    }

    if let Some(params) = params {
        for (key, value) in params {
            query_params.insert(key.clone(), value.clone());
        }
    }

    let query_string = util::to_ordered_array(&query_params).join("&");
    let sign_data = format!("{}\n{}\n{}", method.to_uppercase(), path, query_string);
    let signature = token.sign(&sign_data);
    
    format!("{}&auth_signature={}", query_string, signature)
}

impl std::fmt::Debug for Pusher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pusher")
            .field("config", &self.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pusher_creation() {
        let config = Config::new("123", "key", "secret");
        let pusher = Pusher::new(config);
        assert_eq!(pusher.config().app_id, "123");
    }

    #[tokio::test]
    async fn test_authorize_channel() {
        let config = Config::new("123", "key", "secret");
        let pusher = Pusher::new(config);
        
        let result = pusher.authorize_channel("123.456", "test-channel", None);
        assert!(result.is_ok());
    }
}

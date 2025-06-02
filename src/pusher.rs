use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use reqwest::{Client, Response};
use serde_json::{json, Value};
use sha2::{Sha256, Digest};
use events::EventData;
use crate::{
    Config, Token, auth, events, util, webhook::Webhook,
    PusherError, RequestError, Result, Channel,
};

/// Main Pusher client
#[derive(Clone)]
pub struct Pusher {
    inner: Arc<PusherInner>,
}

struct PusherInner {
    config: Config,
    client: Client,
}

impl Pusher {
    /// Creates a new Pusher client
    pub fn new(config: Config) -> Result<Self> {
        config.validate()?;
        
        let client = Client::builder()
            .timeout(config.timeout())
            .pool_max_idle_per_host(config.pool_max_idle_per_host())
            .build()
            .map_err(|e| PusherError::Config {
                message: format!("Failed to build HTTP client: {}", e),
            })?;
            
        Ok(Self {
            inner: Arc::new(PusherInner { config, client }),
        })
    }

    /// Creates a Pusher client from URL
    pub fn from_url(url: &str, additional_config: Option<Config>) -> Result<Self> {
        let parsed_url = url::Url::parse(url)
            .map_err(|e| PusherError::Config {
                message: format!("Invalid Pusher URL: {}", e),
            })?;

        let auth_parts: Vec<&str> = parsed_url.username().split(':').collect();
        if auth_parts.len() != 2 {
            return Err(PusherError::Config {
                message: "Invalid auth format in URL. Expected KEY:SECRET".to_string(),
            });
        }

        let path_segments: Vec<&str> = parsed_url.path().split('/').collect();
        let app_id = path_segments.last()
            .and_then(|s| if !s.is_empty() { Some(s) } else { None })
            .ok_or_else(|| PusherError::Config {
                message: "App ID not found in URL path".to_string(),
            })?;

        let builder = Config::builder()
            .app_id(*app_id)
            .key(auth_parts[0])
            .secret(auth_parts[1])
            .host(parsed_url.host_str().unwrap_or("api.pusherapp.com"));

        let builder = if parsed_url.scheme() == "https" {
            builder.use_tls(true)
        } else {
            builder.use_tls(false)
        };

        let builder = if let Some(port) = parsed_url.port() {
            builder.port(port)
        } else {
            builder
        };

        // Apply additional config if provided
        let config = if let Some(additional) = additional_config {
            builder
                .timeout(additional.timeout())
                .pool_max_idle_per_host(additional.pool_max_idle_per_host())
                .enable_retry(additional.enable_retry())
                .max_retries(additional.max_retries())
                .build()?
        } else {
            builder.build()?
        };

        Self::new(config)
    }

    /// Gets the configuration
    pub fn config(&self) -> &Config {
        &self.inner.config
    }
    
    /// Creates a new Pusher client for a specific cluster
    pub fn for_cluster(&self, cluster: &str) -> Result<Self> {
        let config = Config::builder()
            .app_id(self.inner.config.app_id())
            .key(&self.inner.config.token().key)
            .secret(&self.inner.config.token().secret_string())
            .cluster(cluster)
            .use_tls(self.inner.config.scheme() == "https")
            .timeout(self.inner.config.timeout())
            .pool_max_idle_per_host(self.inner.config.pool_max_idle_per_host())
            .enable_retry(self.inner.config.enable_retry())
            .max_retries(self.inner.config.max_retries())
            .build()?;
        
        Self::new(config)
    }

    /// Authorizes a channel
    pub fn authorize_channel(
        &self,
        socket_id: &str,
        channel: &Channel,
        data: Option<&Value>,
    ) -> Result<auth::SocketAuth> {
        util::validate_socket_id(socket_id)?;
        auth::get_socket_signature(self, &self.inner.config.token(), &channel.full_name(), socket_id, data)
    }

    /// Authorizes a channel by name (convenience method)
    pub fn authorize_channel_with_name(
        &self,
        socket_id: &str,
        channel_name: &str,
        data: Option<&Value>,
    ) -> Result<auth::SocketAuth> {
        let channel = Channel::from_string(channel_name)?;
        self.authorize_channel(socket_id, &channel, data)
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

        auth::get_socket_signature_for_user(&self.inner.config.token(), socket_id, user_data)
    }

    /// Sends an event to a user
    pub async fn send_to_user<D: Into<EventData>>(
        &self,
        user_id: &str,
        event: &str,
        data: D,
    ) -> Result<Response> {
        if event.len() > 200 {
            return Err(PusherError::Validation {
                message: format!("Event name too long: '{}' (max 200 characters)", event),
            });
        }
        
        util::validate_user_id(user_id)?;
        
        let channel_name = format!("#server-to-user-{}", user_id);
        let channel = Channel::from_string(channel_name)?;
        events::trigger(self, &[channel], event, data, None).await
    }

    /// Terminates user connections
    pub async fn terminate_user_connections(&self, user_id: &str) -> Result<Response> {
        util::validate_user_id(user_id)?;
        let path = format!("/users/{}/terminate_connections", user_id);
        self.post(&path, &json!({})).await
    }

    /// Triggers an event on channels
    pub async fn trigger<D: Into<EventData>>(
        &self,
        channels: &[Channel],
        event: &str,
        data: D,
        params: Option<events::TriggerParams>,
    ) -> Result<Response> {
        if let Some(ref params) = params {
            if let Some(ref socket_id) = params.socket_id {
                util::validate_socket_id(socket_id)?;
            }
        }

        if event.len() > 200 {
            return Err(PusherError::Validation {
                message: format!("Event name too long: '{}' (max 200 characters)", event),
            });
        }

        if channels.is_empty() {
            return Err(PusherError::Validation {
                message: "Must specify at least one channel".to_string(),
            });
        }

        if channels.len() > 100 {
            return Err(PusherError::Validation {
                message: format!("Can't trigger to more than 100 channels (got {})", channels.len()),
            });
        }

        events::trigger(self, channels, event, data, params.as_ref()).await
    }

    /// Triggers an event on channel names (convenience method)
    pub async fn trigger_on_channels<D: Into<EventData>>(
        &self,
        channel_names: &[String],
        event: &str,
        data: D,
        params: Option<events::TriggerParams>,
    ) -> Result<Response> {
        let channels: Result<Vec<Channel>> = channel_names.iter()
            .map(|name| Channel::from_string(name))
            .collect();
        self.trigger(&channels?, event, data, params).await
    }

    /// Triggers a batch of events
    pub async fn trigger_batch(
        &self,
        batch: Vec<events::BatchEvent>,
    ) -> Result<Response> {
        events::trigger_batch(self, batch).await
    }

    /// Makes a POST request
    pub async fn post(&self, path: &str, body: &Value) -> Result<Response> {
        self.send_request("POST", path, Some(body), None).await
    }

    /// Makes a GET request
    pub async fn get(
        &self,
        path: &str,
        params: Option<&BTreeMap<String, String>>,
    ) -> Result<Response> {
        self.send_request("GET", path, None, params).await
    }

    /// Creates a webhook from request data
    pub fn webhook(&self, headers: &BTreeMap<String, String>, body: &str) -> Webhook {
        Webhook::new(&self.inner.config.token(), headers, body)
    }

    /// Generates channel shared secret for encryption
    pub fn channel_shared_secret(&self, channel: &str) -> Result<[u8; 32]> {
        let master_key = self.inner.config.encryption_master_key()
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
        create_signed_query_string(&self.inner.config.token(), method, path, body, params)
    }

    /// Internal method to send HTTP requests with retry logic
    async fn send_request(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        params: Option<&BTreeMap<String, String>>,
    ) -> Result<Response> {
        let full_path = self.inner.config.prefix_path(path);
        let body_str = body.map(|b| serde_json::to_string(b)).transpose()?;
        
        let query_string = create_signed_query_string(
            &self.inner.config.token(),
            method,
            &full_path,
            body_str.as_deref(),
            params,
        );
        
        let url = format!("{}{}?{}", self.inner.config.base_url(), full_path, query_string);
        
        let mut attempt = 0;
        let max_attempts = if self.inner.config.enable_retry() {
            self.inner.config.max_retries() + 1
        } else {
            1
        };
    
        loop {
            attempt += 1;
            
            let mut request = match method {
                "GET" => self.inner.client.get(&url),
                "POST" => self.inner.client.post(&url),
                _ => return Err(PusherError::Request(RequestError::new(
                    format!("Unsupported HTTP method: {}", method),
                    &url,
                    None,
                    None,
                ))),
            };
    
            if let Some(ref body_str) = body_str {
                request = request
                    .header("Content-Type", "application/json")
                    .body(body_str.clone());
            }
    
            let response = request
                .header("X-Pusher-Library", "pushers/1.4.2")
                .send()
                .await;
    
            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        return Ok(resp);
                    }
                    
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    
                    // Don't retry on 4xx errors (client errors)
                    if status >= 400 && status < 500 {
                        return Err(PusherError::Request(RequestError::new(
                            format!("HTTP {}", status),
                            &url,
                            Some(status),
                            Some(body),
                        )));
                    }
                    
                    // Retry on 5xx errors if enabled
                    if attempt >= max_attempts {
                        return Err(PusherError::Request(RequestError::new(
                            format!("HTTP {} after {} attempts", status, attempt),
                            &url,
                            Some(status),
                            Some(body),
                        )));
                    }
                }
                Err(e) => {
                    // Retry on network errors if enabled
                    if attempt >= max_attempts {
                        return Err(PusherError::Http(e));
                    }
                }
            }
            
            // Exponential backoff: 100ms, 200ms, 400ms, etc.
            let delay = Duration::from_millis(100 * (1 << (attempt - 1)));
            tokio::time::sleep(delay).await;
        }
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
            .field("config", &self.inner.config)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pusher_creation() {
        let config = Config::new("123", "key", "secret");
        let pusher = Pusher::new(config).unwrap();
        assert_eq!(pusher.config().app_id(), "123");
    }

    #[tokio::test]
    async fn test_authorize_channel() {
        let config = Config::new("123", "key", "secret");
        let pusher = Pusher::new(config).unwrap();
        
        let result = pusher.authorize_channel("123.456", &Channel::from_string("test-channel").unwrap(), None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_for_cluster() {
        let config = Config::new("123", "key", "secret");
        let pusher = Pusher::new(config).unwrap();
        
        let eu_pusher = pusher.for_cluster("eu").unwrap();
        assert_eq!(eu_pusher.config().host(), "api-eu.pusher.com");
    }
}

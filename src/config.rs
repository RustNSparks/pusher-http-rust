use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crate::{Token, PusherError};

/// Configuration for the Pusher client
#[derive(Clone, Debug)]
pub struct Config {
    pub scheme: String,
    pub host: String,
    pub port: Option<u16>,
    pub app_id: String,
    pub token: Token,
    pub timeout: Option<std::time::Duration>,
    pub encryption_master_key: Option<Vec<u8>>,
}

impl Config {
    /// Creates a new configuration
    pub fn new(
        app_id: impl Into<String>,
        key: impl Into<String>,
        secret: impl Into<String>,
    ) -> Self {
        Self {
            scheme: "https".to_string(),
            host: "api.pusherapp.com".to_string(),
            port: None,
            app_id: app_id.into(),
            token: Token::new(key, secret),
            timeout: Some(std::time::Duration::from_secs(30)),
            encryption_master_key: None,
        }
    }

    /// Sets the cluster
    pub fn cluster(mut self, cluster: &str) -> Self {
        self.host = format!("api-{}.pusher.com", cluster);
        self
    }

    /// Sets whether to use TLS
    pub fn use_tls(mut self, use_tls: bool) -> Self {
        self.scheme = if use_tls { "https" } else { "http" }.to_string();
        self
    }

    /// Sets the port
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Sets the timeout
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the encryption master key from base64
    pub fn encryption_master_key_base64(
        mut self,
        key_base64: &str,
    ) -> crate::Result<Self> {
        let decoded = BASE64.decode(key_base64)
            .map_err(|_| PusherError::Config {
                message: "Invalid base64 encryption key".to_string(),
            })?;

        if decoded.len() != 32 {
            return Err(PusherError::Config {
                message: format!(
                    "Encryption key must be 32 bytes, got {}",
                    decoded.len()
                ),
            });
        }

        self.encryption_master_key = Some(decoded);
        Ok(self)
    }

    /// Gets the base URL
    pub fn base_url(&self) -> String {
        let port = match self.port {
            Some(port) => format!(":{}", port),
            None => String::new(),
        };
        format!("{}://{}{}", self.scheme, self.host, port)
    }

    /// Gets the prefix path for API requests
    pub fn prefix_path(&self, sub_path: &str) -> String {
        format!("/apps/{}{}", self.app_id, sub_path)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new("", "", "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = Config::new("123", "key", "secret")
            .cluster("eu")
            .use_tls(true)
            .port(443);

        assert_eq!(config.scheme, "https");
        assert_eq!(config.host, "api-eu.pusher.com");
        assert_eq!(config.port, Some(443));
        assert_eq!(config.app_id, "123");
    }

    #[test]
    fn test_base_url() {
        let config = Config::new("123", "key", "secret");
        assert_eq!(config.base_url(), "https://api.pusherapp.com");

        let config = config.port(8080);
        assert_eq!(config.base_url(), "https://api.pusherapp.com:8080");
    }
}

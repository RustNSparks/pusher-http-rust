use std::collections::BTreeMap;
use serde_json::Value;
use crate::{Token, WebhookError, Result};

/// Webhook for validating and accessing Pusher webhook data
#[derive(Debug)]
pub struct Webhook {
    token: Token,
    key: Option<String>,
    signature: Option<String>,
    content_type: Option<String>,
    body: String,
    data: Option<Value>,
}

impl Webhook {
    /// Creates a new webhook from request data
    pub fn new(
        token: &Token,
        headers: &BTreeMap<String, String>,
        body: &str,
    ) -> Self {
        let key = headers.get("x-pusher-key").cloned();
        let signature = headers.get("x-pusher-signature").cloned();
        let content_type = headers.get("content-type").cloned();

        let data = if Self::validate_content_type(&content_type) {
            serde_json::from_str(body).ok()
        } else {
            None
        };

        Self {
            token: token.clone(),
            key,
            signature,
            content_type,
            body: body.to_string(),
            data,
        }
    }

    /// Validates the webhook signature and content
    pub fn is_valid(&self, extra_tokens: Option<&[Token]>) -> bool {
        if !self.is_body_valid() {
            return false;
        }

        let tokens_to_check = if let Some(extra) = extra_tokens {
            let mut tokens = vec![&self.token];
            tokens.extend(extra.iter());
            tokens
        } else {
            vec![&self.token]
        };

        for token in tokens_to_check {
            if let (Some(key), Some(signature)) = (&self.key, &self.signature) {
                if key == &token.key && token.verify(&self.body, signature) {
                    return true;
                }
            }
        }

        false
    }

    /// Checks if the content type is valid (application/json)
    pub fn is_content_type_valid(&self) -> bool {
        Self::validate_content_type(&self.content_type)
    }

    // Private helper method with different name to avoid conflict
    fn validate_content_type(content_type: &Option<String>) -> bool {
        content_type.as_deref() == Some("application/json")
    }

    /// Checks if the body is valid JSON
    pub fn is_body_valid(&self) -> bool {
        self.data.is_some()
    }

    /// Gets all webhook data
    pub fn get_data(&self) -> Result<&Value> {
        self.data.as_ref().ok_or_else(|| {
            crate::PusherError::Webhook(WebhookError::new(
                "Invalid webhook body",
                self.content_type.clone(),
                &self.body,
                self.signature.clone(),
            ))
        })
    }

    /// Gets the events array from webhook data
    pub fn get_events(&self) -> Result<&Value> {
        let data = self.get_data()?;
        data.get("events").ok_or_else(|| {
            crate::PusherError::Webhook(WebhookError::new(
                "No events found in webhook data",
                self.content_type.clone(),
                &self.body,
                self.signature.clone(),
            ))
        })
    }

    /// Gets the timestamp from webhook data
    pub fn get_time(&self) -> Result<std::time::SystemTime> {
        let data = self.get_data()?;
        let time_ms = data.get("time_ms")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                crate::PusherError::Webhook(WebhookError::new(
                    "Invalid or missing time_ms in webhook data",
                    self.content_type.clone(),
                    &self.body,
                    self.signature.clone(),
                ))
            })?;

        let duration = std::time::Duration::from_millis(time_ms);
        Ok(std::time::UNIX_EPOCH + duration)
    }

    /// Gets the raw body
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Gets the signature
    pub fn signature(&self) -> Option<&str> {
        self.signature.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_webhook_creation() {
        let token = Token::new("test_key", "test_secret");
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert("x-pusher-key".to_string(), "test_key".to_string());
        
        let body = json!({
            "time_ms": 1234567890,
            "events": []
        }).to_string();

        let webhook = Webhook::new(&token, &headers, &body);
        
        assert!(webhook.is_content_type_valid());
        assert!(webhook.is_body_valid());
    }

    #[test]
    fn test_webhook_validation() {
        let token = Token::new("test_key", "test_secret");
        let body = r#"{"time_ms": 1234567890, "events": []}"#;
        let signature = token.sign(body);
        
        let mut headers = BTreeMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert("x-pusher-key".to_string(), "test_key".to_string());
        headers.insert("x-pusher-signature".to_string(), signature);
        
        let webhook = Webhook::new(&token, &headers, body);
        assert!(webhook.is_valid(None));
    }
}

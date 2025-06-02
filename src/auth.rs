use serde_json::Value;
use crate::{Token, util};

/// Authentication data for socket connections
#[derive(Debug, serde::Serialize)]
pub struct SocketAuth {
    pub auth: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_secret: Option<String>,
}

/// User authentication data
#[derive(Debug, serde::Serialize)]
pub struct UserAuth {
    pub auth: String,
    pub user_data: String,
}

/// Gets socket signature for channel authorization
pub fn get_socket_signature(
    pusher: &crate::Pusher,
    token: &Token,
    channel: &str,
    socket_id: &str,
    data: Option<&Value>,
) -> crate::Result<SocketAuth> {
    let mut signature_data = vec![socket_id.to_string(), channel.to_string()];
    let mut channel_data = None;

    if let Some(data) = data {
        let serialized = serde_json::to_string(data)?;
        signature_data.push(serialized.clone());
        channel_data = Some(serialized);
    }

    let auth_string = signature_data.join(":");
    let signature = token.sign(&auth_string);
    let auth = format!("{}:{}", token.key, signature);

    let mut result = SocketAuth {
        auth,
        channel_data,
        shared_secret: None,
    };

    // Handle encrypted channels
    if util::is_encrypted_channel(channel) {
        #[cfg(feature = "encryption")]
        {
            if pusher.config().encryption_master_key().is_none() {
                return Err(crate::PusherError::Encryption {
                    message: "Cannot generate shared_secret because encryptionMasterKey is not set".to_string(),
                });
            }

            let shared_secret = pusher.channel_shared_secret(channel)?;
            result.shared_secret = Some(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &shared_secret
            ));
        }

        #[cfg(not(feature = "encryption"))]
        {
            return Err(crate::PusherError::Encryption {
                message: "Encryption support is not enabled. Enable the 'encryption' feature to use encrypted channels.".to_string(),
            });
        }
    }

    Ok(result)
}

/// Gets socket signature for user authentication
pub fn get_socket_signature_for_user(
    token: &Token,
    socket_id: &str,
    user_data: &Value,
) -> crate::Result<UserAuth> {
    let serialized_user_data = serde_json::to_string(user_data)?;
    let signature_string = format!("{}::user::{}", socket_id, serialized_user_data);
    let signature = token.sign(&signature_string);

    Ok(UserAuth {
        auth: format!("{}:{}", token.key, signature),
        user_data: serialized_user_data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_get_socket_signature_for_user() {
        let token = Token::new("test_key", "test_secret");
        let user_data = json!({"id": "123", "name": "Test User"});

        let result = get_socket_signature_for_user(&token, "123.456", &user_data).unwrap();

        assert!(result.auth.starts_with("test_key:"));
        assert!(result.user_data.contains("123"));
    }

    #[cfg(feature = "encryption")]
    #[test]
    fn test_encrypted_channel_auth_with_encryption() {
        use crate::{Config, Pusher};

        // This test only runs when encryption is enabled
        let config = Config::builder()
            .app_id("test")
            .key("test_key")
            .secret("test_secret")
            .encryption_master_key_base64("aSBhbSAzMiBieXRlcyBsb25nIGVuY3J5cHRpb24ga2V5")
            .unwrap()
            .build()
            .unwrap();

        let pusher = Pusher::new(config).unwrap();
        let token = Token::new("test_key", "test_secret");

        let result = get_socket_signature(
            &pusher,
            &token,
            "private-encrypted-test",
            "123.456",
            None
        ).unwrap();

        assert!(result.shared_secret.is_some());
    }

    #[cfg(not(feature = "encryption"))]
    #[test]
    fn test_encrypted_channel_auth_without_encryption() {
        use crate::{Config, Pusher};

        // This test only runs when encryption is disabled
        let config = Config::builder()
            .app_id("test")
            .key("test_key")
            .secret("test_secret")
            .build()
            .unwrap();

        let pusher = Pusher::new(config).unwrap();
        let token = Token::new("test_key", "test_secret");

        let result = get_socket_signature(
            &pusher,
            &token,
            "private-encrypted-test",
            "123.456",
            None
        );

        // Should fail with appropriate error message
        assert!(result.is_err());
        if let Err(crate::PusherError::Encryption { message }) = result {
            assert!(message.contains("Encryption support is not enabled"));
        } else {
            panic!("Expected encryption error");
        }
    }
}
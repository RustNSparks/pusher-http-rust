use std::collections::BTreeMap;

/// Converts a map to an ordered array of key=value pairs
pub fn to_ordered_array(map: &BTreeMap<String, String>) -> Vec<String> {
    map.iter()
        .map(|(key, value)| format!("{}={}", key, value))
        .collect()
}

/// Calculates MD5 hash of the input
pub fn get_md5(body: &str) -> String {
    // 1. Create an MD5 hasher and update it with the input body.
    //    The `md5::compute` function handles this in one step.
    //    It takes the input (which can be `&str` or `&[u8]`) and computes the MD5 digest.
    //    Rust strings are UTF-8, so `body.as_bytes()` gives the UTF-8 byte representation.
    let digest = md5::compute(body.as_bytes());

    // 2. Digest the hash into a hexadecimal string.
    //    The `digest` variable is of type `md5::Digest`, which is typically a struct
    //    wrapping a 16-byte array (e.g., `struct Digest([u8; 16])`).
    //    `digest.as_ref()` provides a `&[u8]` slice from the Digest.
    //    `hex::encode` converts this byte slice into a hexadecimal String.
    hex::encode(digest.as_ref())

    // Alternative ways to get the hex string from md5::Digest:
    // - hex::encode(digest.0) // If Digest is `struct Digest([u8; 16])`, this accesses the inner array.
    // - format!("{:x}", digest) // md5::Digest implements LowerHex, so this works directly.
}

/// Constant-time string comparison to prevent timing attacks
pub fn secure_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    let mut result = 0u8;
    for (byte_a, byte_b) in a.bytes().zip(b.bytes()) {
        result |= byte_a ^ byte_b;
    }
    
    result == 0
}

/// Checks if a channel is encrypted
pub fn is_encrypted_channel(channel: &str) -> bool {
    channel.starts_with("private-encrypted-")
}

/// Validates a channel name
pub fn validate_channel(channel: &str) -> crate::Result<()> {
    if channel.is_empty() {
        return Err(crate::PusherError::Validation {
            message: "Channel name cannot be empty".to_string(),
        });
    }
    
    if channel.len() > 200 {
        return Err(crate::PusherError::Validation {
            message: format!("Channel name too long: '{}'", channel),
        });
    }
    
    let valid_pattern = regex::Regex::new(r"^[A-Za-z0-9_\-=@,.;]+$").unwrap();
    if !valid_pattern.is_match(channel) {
        return Err(crate::PusherError::Validation {
            message: format!("Invalid channel name: '{}'", channel),
        });
    }
    
    Ok(())
}

/// Validates a socket ID
pub fn validate_socket_id(socket_id: &str) -> crate::Result<()> {
    let pattern = regex::Regex::new(r"^\d+\.\d+$").unwrap();
    if !pattern.is_match(socket_id) {
        return Err(crate::PusherError::Validation {
            message: format!("Invalid socket id: '{}'", socket_id),
        });
    }
    Ok(())
}

/// Validates a user ID
pub fn validate_user_id(user_id: &str) -> crate::Result<()> {
    if user_id.is_empty() {
        return Err(crate::PusherError::Validation {
            message: "User ID cannot be empty".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_compare() {
        assert!(secure_compare("hello", "hello"));
        assert!(!secure_compare("hello", "world"));
        assert!(!secure_compare("hello", "hello world"));
    }

    #[test]
    fn test_is_encrypted_channel() {
        assert!(is_encrypted_channel("private-encrypted-test"));
        assert!(!is_encrypted_channel("private-test"));
        assert!(!is_encrypted_channel("public-test"));
    }

    #[test]
    fn test_validate_channel() {
        assert!(validate_channel("test-channel").is_ok());
        assert!(validate_channel("").is_err());
        assert!(validate_channel(&"a".repeat(201)).is_err());
    }
}

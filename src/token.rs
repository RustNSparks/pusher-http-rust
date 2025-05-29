use hmac::{Hmac, Mac};
use sha2::Sha256;
use crate::util;

type HmacSha256 = Hmac<Sha256>;

/// Token for signing and verifying data against the app key and secret
#[derive(Clone, Debug)]
pub struct Token {
    pub key: String,
    secret: String,
}

impl Token {
    /// Creates a new token with the given key and secret
    pub fn new(key: impl Into<String>, secret: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            secret: secret.into(),
        }
    }

    /// Signs the string using HMAC-SHA256
    pub fn sign(&self, data: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(data.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// Verifies the signature against the data
    pub fn verify(&self, data: &str, signature: &str) -> bool {
        let expected = self.sign(data);
        util::secure_compare(&expected, signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify() {
        let token = Token::new("test_key", "test_secret");
        let data = "test_data";
        let signature = token.sign(data);
        
        assert!(token.verify(data, &signature));
        assert!(!token.verify("other_data", &signature));
    }
}

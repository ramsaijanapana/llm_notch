//! Per-app-start auth token generation and verification.

use base64::Engine;
use rand::RngExt;

use crate::error::{IpcError, IpcResult};
use crate::limits::AUTH_TOKEN_BYTES;

#[derive(Clone)]
pub struct AuthToken([u8; AUTH_TOKEN_BYTES]);

impl AuthToken {
    pub fn generate() -> Self {
        let mut bytes = [0u8; AUTH_TOKEN_BYTES];
        rand::rng().fill(&mut bytes);
        Self(bytes)
    }

    pub fn from_bytes(bytes: [u8; AUTH_TOKEN_BYTES]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; AUTH_TOKEN_BYTES] {
        &self.0
    }

    pub fn encode_b64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.0)
    }

    pub fn decode_b64(value: &str) -> IpcResult<Self> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(value.as_bytes())
            .map_err(|_| IpcError::AuthFailed)?;
        let array: [u8; AUTH_TOKEN_BYTES] = bytes.try_into().map_err(|_| IpcError::AuthFailed)?;
        Ok(Self(array))
    }

    pub fn constant_time_eq(&self, other: &Self) -> bool {
        let mut diff = 0u8;
        for (a, b) in self.0.iter().zip(other.0.iter()) {
            diff |= a ^ b;
        }
        diff == 0
    }
}

impl std::fmt::Debug for AuthToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AuthToken([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_b64() {
        let token = AuthToken::generate();
        let encoded = token.encode_b64();
        let decoded = AuthToken::decode_b64(&encoded).expect("decode");
        assert!(token.constant_time_eq(&decoded));
    }
}

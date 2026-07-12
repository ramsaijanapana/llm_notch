use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectPolicy {
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub max_attempts: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Authenticating,
    Streaming,
    Backoff { attempt: u16, delay_ms: u64 },
    Failed,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            base_delay_ms: 500,
            max_delay_ms: 30_000,
            max_attempts: 12,
        }
    }
}

impl ReconnectPolicy {
    /// Returns a bounded exponential delay. `jitter_basis_points` is supplied by the caller so
    /// tests remain deterministic and the transport can use its platform RNG.
    pub fn delay_ms(&self, attempt: u16, jitter_basis_points: i16) -> Option<u64> {
        if attempt >= self.max_attempts {
            return None;
        }
        let exponent = u32::from(attempt.min(20));
        let raw = self
            .base_delay_ms
            .saturating_mul(1_u64.checked_shl(exponent).unwrap_or(u64::MAX))
            .min(self.max_delay_ms);
        let jitter = i64::from(jitter_basis_points.clamp(-2_500, 2_500));
        let adjusted = i128::from(raw) * i128::from(10_000 + jitter) / 10_000;
        Some((adjusted.max(0) as u64).min(self.max_delay_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_delay_is_exponential_capped_and_bounded() {
        let policy = ReconnectPolicy::default();
        assert_eq!(policy.delay_ms(0, 0), Some(500));
        assert_eq!(policy.delay_ms(3, 0), Some(4_000));
        assert_eq!(policy.delay_ms(10, 0), Some(30_000));
        assert_eq!(policy.delay_ms(12, 0), None);
        assert_eq!(policy.delay_ms(10, 2_500), Some(30_000));
    }
}

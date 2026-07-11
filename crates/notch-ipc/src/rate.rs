//! Token-bucket rate limiting for per-client and global ingest.

use governor::{Quota, RateLimiter};
use parking_lot::Mutex;
use std::num::NonZeroU32;

use crate::error::{IpcError, IpcResult};
use crate::limits::{MAX_BURST_PER_CLIENT, MAX_EVENTS_PER_SEC, MAX_GLOBAL_EVENTS_PER_SEC};

type DirectLimiter = RateLimiter<
    governor::state::NotKeyed,
    governor::state::InMemoryState,
    governor::clock::DefaultClock,
>;

type KeyedLimiter = RateLimiter<
    u64,
    governor::state::keyed::DefaultKeyedStateStore<u64>,
    governor::clock::DefaultClock,
>;

pub struct IngestRateLimiters {
    per_client: Mutex<KeyedLimiter>,
    global: Mutex<DirectLimiter>,
    next_client_id: Mutex<u64>,
}

impl IngestRateLimiters {
    pub fn new() -> Self {
        let per_client_quota =
            Quota::per_second(NonZeroU32::new(MAX_EVENTS_PER_SEC).expect("events/sec"))
                .allow_burst(NonZeroU32::new(MAX_BURST_PER_CLIENT).expect("burst"));
        let global_quota =
            Quota::per_second(NonZeroU32::new(MAX_GLOBAL_EVENTS_PER_SEC).expect("global/sec"));
        Self {
            per_client: Mutex::new(RateLimiter::keyed(per_client_quota)),
            global: Mutex::new(RateLimiter::direct(global_quota)),
            next_client_id: Mutex::new(1),
        }
    }

    pub fn assign_client_id(&self) -> u64 {
        let mut next = self.next_client_id.lock();
        let id = *next;
        *next = next.saturating_add(1);
        id
    }

    pub fn check(&self, client_id: u64) -> IpcResult<()> {
        self.global
            .lock()
            .check()
            .map_err(|_| IpcError::RateLimited)?;
        self.per_client
            .lock()
            .check_key(&client_id)
            .map_err(|_| IpcError::RateLimited)?;
        Ok(())
    }
}

impl Default for IngestRateLimiters {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_initial_burst() {
        let limiters = IngestRateLimiters::new();
        for _ in 0..MAX_BURST_PER_CLIENT {
            limiters.check(1).expect("within burst");
        }
    }
}

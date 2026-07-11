//! Decision broker core: idempotent waits, expiry, and honest delivery states.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use notch_protocol::{
    DECISION_FAIL_OPEN_TIMEOUT_MS, DecisionDeliveryState, DecisionRequest, DecisionResponse,
    DecisionResponseAction, DecisionResponseRecord,
};
use parking_lot::Mutex;
use thiserror::Error;
use tokio::sync::Notify;
use tracing::debug;

use crate::adapter::{self, AdapterBuildError};
use crate::store::{DecisionStore, StoreError, truncate_summary};
use crate::types::{
    ActiveDecision, DecisionReplyPayload, InternalDeliveryState, PendingDecisionWait,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BrokerError {
    #[error("decision not found")]
    NotFound,
    #[error("decision expired")]
    Expired,
    #[error("decision already finalized")]
    AlreadyFinalized,
    #[error("decision has no actionable payload")]
    NotActionable,
    #[error("duplicate nonce binding mismatch")]
    BindingMismatch,
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("store error: {0}")]
    Store(String),
    #[error("adapter error: {0}")]
    Adapter(String),
}

pub struct DecisionBroker {
    store: DecisionStore,
    active: Mutex<HashMap<String, ActiveDecision>>,
    waiters: Mutex<HashMap<String, Arc<Notify>>>,
    now_ms: fn() -> i64,
}

impl DecisionBroker {
    pub fn open(database_path: impl AsRef<std::path::Path>) -> Result<Self, StoreError> {
        Ok(Self {
            store: DecisionStore::open(database_path)?,
            active: Mutex::new(HashMap::new()),
            waiters: Mutex::new(HashMap::new()),
            now_ms: || chrono::Utc::now().timestamp_millis(),
        })
    }

    pub fn in_memory() -> Result<Self, StoreError> {
        Ok(Self {
            store: DecisionStore::in_memory()?,
            active: Mutex::new(HashMap::new()),
            waiters: Mutex::new(HashMap::new()),
            now_ms: || chrono::Utc::now().timestamp_millis(),
        })
    }

    #[cfg(test)]
    pub fn with_clock(now_ms: fn() -> i64) -> Result<Self, StoreError> {
        Ok(Self {
            store: DecisionStore::in_memory()?,
            active: Mutex::new(HashMap::new()),
            waiters: Mutex::new(HashMap::new()),
            now_ms,
        })
    }

    pub fn list_pending(&self) -> Result<Vec<DecisionRequest>, BrokerError> {
        self.store
            .list_pending_requests((self.now_ms)())
            .map_err(|err| BrokerError::Store(err.to_string()))
    }

    pub async fn handle_wait(&self, pending: PendingDecisionWait) -> DecisionReplyPayload {
        let payload = pending.payload;
        let nonce = payload.nonce.clone();
        if let Some(existing) = self.active.lock().get(&nonce) {
            if existing.connection_id != payload.connection_id {
                return self.fail_open_reply(&nonce, "binding mismatch for nonce");
            }
            if existing.internal_state.is_terminal() {
                return self.replay_terminal(&nonce, existing);
            }
        }

        let expires_at_ms = payload.created_at_ms + DECISION_FAIL_OPEN_TIMEOUT_MS as i64;
        let session_id = payload
            .session_id
            .clone()
            .unwrap_or_else(|| payload.external_session_id.clone());
        let request = DecisionRequest {
            id: nonce.clone(),
            session_id,
            source: payload.source,
            kind: payload.decision_kind,
            summary: truncate_summary(&payload.summary),
            has_actionable_payload: payload.has_actionable_payload,
            created_at_ms: payload.created_at_ms,
            expires_at_ms: Some(expires_at_ms),
        };

        let active = ActiveDecision {
            request: request.clone(),
            internal_state: InternalDeliveryState::Pending,
            vendor_event: payload.vendor_event.clone(),
            external_session_id: payload.external_session_id.clone(),
            respondable_hook: payload.respondable_hook.clone(),
            tool_name: payload.tool_name.clone(),
            connection_id: payload.connection_id.clone(),
            vendor_context: payload.vendor_context.clone(),
            expires_at_ms,
        };

        if let Err(err) = self.store.upsert_active(
            &request,
            &active.vendor_event,
            &active.external_session_id,
            &active.connection_id,
            DecisionDeliveryState::Pending,
        ) {
            debug!(%err, nonce, "decision audit insert failed");
        }

        let notify = {
            let mut waiters = self.waiters.lock();
            waiters
                .entry(nonce.clone())
                .or_insert_with(|| Arc::new(Notify::new()))
                .clone()
        };
        self.active.lock().insert(nonce.clone(), active);

        let wait = notify.notified();
        let timeout = tokio::time::sleep(Duration::from_millis(DECISION_FAIL_OPEN_TIMEOUT_MS));
        tokio::pin!(wait);
        tokio::pin!(timeout);

        tokio::select! {
            _ = &mut wait => {},
            _ = &mut timeout => {
                self.expire_decision(&nonce);
            }
        }

        let reply = self
            .active
            .lock()
            .get(&nonce)
            .map(|active| self.reply_for_active(&nonce, active))
            .unwrap_or_else(|| self.neutral_reply(&nonce, DecisionDeliveryState::Expired));

        self.active.lock().remove(&nonce);
        self.waiters.lock().remove(&nonce);
        reply
    }

    pub fn submit_decision(
        &self,
        request_id: &str,
        response: DecisionResponse,
    ) -> Result<DecisionResponseRecord, BrokerError> {
        self.validate_response(&response)?;
        let now = (self.now_ms)();
        let mut guard = self.active.lock();
        let active = guard.get_mut(request_id).ok_or(BrokerError::NotFound)?;

        if active.internal_state.is_terminal() {
            return Err(BrokerError::AlreadyFinalized);
        }
        if now >= active.expires_at_ms {
            let expired = active.clone();
            drop(guard);
            self.finalize_expired(&expired);
            self.active.lock().remove(request_id);
            self.wake_waiter(request_id);
            return Err(BrokerError::Expired);
        }
        if !active.request.has_actionable_payload {
            return Err(BrokerError::NotActionable);
        }

        active.internal_state = InternalDeliveryState::Chosen {
            response: response.clone(),
            responded_at_ms: now,
        };

        let stdout = match build_stdout_for_active(&active, &response) {
            Ok(stdout) => stdout,
            Err(AdapterBuildError::CapabilityDisabled) => adapter::neutral_stdout(),
            Err(err) => {
                active.internal_state = InternalDeliveryState::Failed {
                    detail: err.to_string(),
                };
                self.persist_state(&active, None, Some(now), Some(err.to_string()));
                self.wake_waiter(request_id);
                return Err(BrokerError::Adapter(err.to_string()));
            }
        };

        active.internal_state = InternalDeliveryState::Delivered {
            stdout_json: stdout.clone(),
            delivered_at_ms: now,
        };
        self.persist_state(&active, Some(&response), Some(now), None);
        self.wake_waiter(request_id);

        Ok(active.response_record(&response, now))
    }

    pub fn observe_effect(&self, request_id: &str, evidence: String) -> Result<(), BrokerError> {
        let mut guard = self.active.lock();
        let active = guard.get_mut(request_id).ok_or(BrokerError::NotFound)?;
        match &active.internal_state {
            InternalDeliveryState::Delivered { .. } => {
                active.internal_state = InternalDeliveryState::EffectObserved {
                    evidence: evidence.clone(),
                    observed_at_ms: (self.now_ms)(),
                };
                let _ = self.store.update_delivery(
                    request_id,
                    DecisionDeliveryState::EffectObserved,
                    None,
                    None,
                    Some(&evidence),
                );
                Ok(())
            }
            _ => Err(BrokerError::AlreadyFinalized),
        }
    }

    fn validate_response(&self, response: &DecisionResponse) -> Result<(), BrokerError> {
        match response {
            DecisionResponse::Action { action } => {
                if !matches!(
                    action,
                    DecisionResponseAction::Allow | DecisionResponseAction::Deny
                ) {
                    return Err(BrokerError::InvalidResponse(
                        "unsupported decision action".into(),
                    ));
                }
                Ok(())
            }
            DecisionResponse::Answer { .. } => Err(BrokerError::InvalidResponse(
                "question answers are unsupported in protocol v1".into(),
            )),
        }
    }

    fn expire_decision(&self, nonce: &str) {
        let Some(mut active) = self.active.lock().remove(nonce) else {
            return;
        };
        if active.internal_state.is_terminal() {
            self.active.lock().insert(nonce.to_string(), active);
            return;
        }
        active.internal_state = InternalDeliveryState::Expired;
        self.finalize_expired(&active);
    }

    fn finalize_expired(&self, active: &ActiveDecision) {
        let _ = self.store.update_delivery(
            &active.request.id,
            DecisionDeliveryState::Expired,
            None,
            None,
            Some("decision window elapsed"),
        );
    }

    fn persist_state(
        &self,
        active: &ActiveDecision,
        response: Option<&DecisionResponse>,
        responded_at_ms: Option<i64>,
        detail: Option<String>,
    ) {
        let _ = self.store.update_delivery(
            &active.request.id,
            active.internal_state.wire_state(),
            response,
            responded_at_ms,
            detail.as_deref(),
        );
    }

    fn wake_waiter(&self, nonce: &str) {
        if let Some(notify) = self.waiters.lock().remove(nonce) {
            notify.notify_waiters();
        }
    }

    fn reply_for_active(&self, nonce: &str, active: &ActiveDecision) -> DecisionReplyPayload {
        match &active.internal_state {
            InternalDeliveryState::Delivered { stdout_json, .. } => DecisionReplyPayload {
                nonce: nonce.into(),
                stdout_json: stdout_json.clone(),
                delivery_state: DecisionDeliveryState::Delivered,
            },
            InternalDeliveryState::Failed { detail: _ } => DecisionReplyPayload {
                nonce: nonce.into(),
                stdout_json: adapter::neutral_stdout(),
                delivery_state: DecisionDeliveryState::Failed,
            },
            InternalDeliveryState::Expired => {
                self.neutral_reply(nonce, DecisionDeliveryState::Expired)
            }
            InternalDeliveryState::Chosen { .. } | InternalDeliveryState::Pending => {
                self.neutral_reply(nonce, DecisionDeliveryState::Expired)
            }
            InternalDeliveryState::EffectObserved { .. } => DecisionReplyPayload {
                nonce: nonce.into(),
                stdout_json: adapter::neutral_stdout(),
                delivery_state: DecisionDeliveryState::EffectObserved,
            },
        }
    }

    fn replay_terminal(&self, nonce: &str, active: &ActiveDecision) -> DecisionReplyPayload {
        match &active.internal_state {
            InternalDeliveryState::Expired | InternalDeliveryState::Failed { .. } => {
                self.neutral_reply(nonce, active.internal_state.wire_state())
            }
            InternalDeliveryState::Delivered { stdout_json, .. } => DecisionReplyPayload {
                nonce: nonce.into(),
                stdout_json: stdout_json.clone(),
                delivery_state: DecisionDeliveryState::Delivered,
            },
            _ => self.neutral_reply(nonce, DecisionDeliveryState::Expired),
        }
    }

    fn neutral_reply(&self, nonce: &str, state: DecisionDeliveryState) -> DecisionReplyPayload {
        DecisionReplyPayload {
            nonce: nonce.into(),
            stdout_json: adapter::neutral_stdout(),
            delivery_state: state,
        }
    }

    fn fail_open_reply(&self, nonce: &str, detail: &str) -> DecisionReplyPayload {
        debug!(nonce, detail, "decision wait failed open");
        DecisionReplyPayload {
            nonce: nonce.into(),
            stdout_json: adapter::neutral_stdout(),
            delivery_state: DecisionDeliveryState::Failed,
        }
    }
}

fn build_stdout_for_active(
    active: &ActiveDecision,
    response: &DecisionResponse,
) -> Result<String, AdapterBuildError> {
    adapter::build_vendor_stdout(
        active.request.source,
        active.respondable_hook.as_deref(),
        response,
        active.vendor_context.as_ref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DecisionWaitPayload;
    use notch_protocol::{AgentSource, DecisionKind};
    use serde_json::json;
    use std::sync::atomic::{AtomicI64, Ordering};

    fn test_broker() -> DecisionBroker {
        static NOW: AtomicI64 = AtomicI64::new(1_000);
        DecisionBroker::with_clock(|| NOW.load(Ordering::Relaxed)).expect("broker")
    }

    fn sample_wait(nonce: &str) -> PendingDecisionWait {
        PendingDecisionWait {
            payload: DecisionWaitPayload {
                nonce: nonce.into(),
                source: AgentSource::ClaudeCode,
                vendor_event: "PermissionRequest".into(),
                external_session_id: "ext-1".into(),
                session_id: Some("sess-1".into()),
                decision_kind: DecisionKind::Permission,
                summary: "Allow file write?".into(),
                has_actionable_payload: true,
                respondable_hook: Some("permissionRequest".into()),
                tool_name: Some("Write".into()),
                connection_id: "conn-1".into(),
                vendor_context: Some(json!({"claude_code_version": "2.1.205"})),
                created_at_ms: 1_000,
            },
        }
    }

    #[tokio::test]
    async fn timeout_fails_open_with_neutral_stdout() {
        let broker = Arc::new(test_broker());
        let wait = sample_wait("timeout-1");
        let broker_task = {
            let broker = Arc::clone(&broker);
            tokio::spawn(async move { broker.handle_wait(wait).await })
        };
        let reply = broker_task.await.expect("task");
        assert_eq!(reply.stdout_json, adapter::neutral_stdout());
        assert_eq!(reply.delivery_state, DecisionDeliveryState::Expired);
        assert!(broker.list_pending().expect("list").is_empty());
    }

    #[tokio::test]
    async fn submit_delivers_vendor_stdout_to_waiter() {
        let broker = Arc::new(test_broker());
        let wait = sample_wait("ack-1");
        let broker_task = {
            let broker = Arc::clone(&broker);
            tokio::spawn(async move { broker.handle_wait(wait).await })
        };
        tokio::task::yield_now().await;
        let record = broker
            .submit_decision(
                "ack-1",
                DecisionResponse::Action {
                    action: DecisionResponseAction::Allow,
                },
            )
            .expect("submit");
        assert_eq!(record.delivery_state, DecisionDeliveryState::Delivered);
        let reply = broker_task.await.expect("task");
        assert_ne!(reply.stdout_json, adapter::neutral_stdout());
        assert_eq!(reply.delivery_state, DecisionDeliveryState::Delivered);
    }

    #[tokio::test]
    async fn duplicate_submit_is_rejected() {
        let broker = Arc::new(test_broker());
        let wait = sample_wait("dup-1");
        let broker_task = {
            let broker = Arc::clone(&broker);
            tokio::spawn(async move {
                let _ = broker.handle_wait(wait).await;
            })
        };
        tokio::task::yield_now().await;
        broker
            .submit_decision(
                "dup-1",
                DecisionResponse::Action {
                    action: DecisionResponseAction::Deny,
                },
            )
            .expect("first");
        let _ = broker_task.await;
        let err = broker
            .submit_decision(
                "dup-1",
                DecisionResponse::Action {
                    action: DecisionResponseAction::Allow,
                },
            )
            .unwrap_err();
        assert_eq!(err, BrokerError::NotFound);
    }

    #[test]
    fn expired_decision_rejects_late_submit() {
        static NOW: AtomicI64 = AtomicI64::new(1_000);
        let broker = DecisionBroker::with_clock(|| NOW.load(Ordering::Relaxed)).expect("broker");
        broker.active.lock().insert(
            "expired-1".into(),
            ActiveDecision {
                request: DecisionRequest {
                    id: "expired-1".into(),
                    session_id: "sess".into(),
                    source: AgentSource::ClaudeCode,
                    kind: DecisionKind::Permission,
                    summary: "late".into(),
                    has_actionable_payload: true,
                    created_at_ms: 1_000,
                    expires_at_ms: Some(3_000),
                },
                internal_state: InternalDeliveryState::Pending,
                vendor_event: "PermissionRequest".into(),
                external_session_id: "ext".into(),
                respondable_hook: Some("permissionRequest".into()),
                tool_name: None,
                connection_id: "conn".into(),
                vendor_context: Some(json!({"claude_code_version": "2.1.205"})),
                expires_at_ms: 3_000,
            },
        );
        NOW.store(3_001, Ordering::Relaxed);
        let err = broker
            .submit_decision(
                "expired-1",
                DecisionResponse::Action {
                    action: DecisionResponseAction::Allow,
                },
            )
            .unwrap_err();
        assert_eq!(err, BrokerError::Expired);
    }

    #[test]
    fn non_actionable_payload_rejects_submit() {
        static NOW: AtomicI64 = AtomicI64::new(1_000);
        let broker = DecisionBroker::with_clock(|| NOW.load(Ordering::Relaxed)).expect("broker");
        broker.active.lock().insert(
            "no-controls".into(),
            ActiveDecision {
                request: DecisionRequest {
                    id: "no-controls".into(),
                    session_id: "sess".into(),
                    source: AgentSource::ClaudeCode,
                    kind: DecisionKind::Permission,
                    summary: "observed".into(),
                    has_actionable_payload: false,
                    created_at_ms: 1_000,
                    expires_at_ms: Some(5_000),
                },
                internal_state: InternalDeliveryState::Pending,
                vendor_event: "PermissionRequest".into(),
                external_session_id: "ext".into(),
                respondable_hook: None,
                tool_name: None,
                connection_id: "conn".into(),
                vendor_context: None,
                expires_at_ms: 5_000,
            },
        );
        let err = broker
            .submit_decision(
                "no-controls",
                DecisionResponse::Action {
                    action: DecisionResponseAction::Allow,
                },
            )
            .unwrap_err();
        assert_eq!(err, BrokerError::NotActionable);
    }
}

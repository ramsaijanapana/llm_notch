//! Internal broker types (not frozen protocol wire types).

use notch_protocol::{
    DecisionDeliveryState, DecisionKind, DecisionRequest, DecisionResponse, DecisionResponseRecord,
};
use serde_json::Value;

/// Payload forwarded from the hook helper over IPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionWaitPayload {
    pub nonce: String,
    pub source: notch_protocol::AgentSource,
    pub vendor_event: String,
    pub external_session_id: String,
    pub session_id: Option<String>,
    pub decision_kind: DecisionKind,
    pub summary: String,
    pub has_actionable_payload: bool,
    pub respondable_hook: Option<String>,
    pub tool_name: Option<String>,
    pub connection_id: String,
    pub vendor_context: Option<Value>,
    pub created_at_ms: i64,
}

/// Reply delivered back to a waiting hook connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionReplyPayload {
    pub nonce: String,
    pub stdout_json: String,
    pub delivery_state: DecisionDeliveryState,
}

/// Awaited decision registration from IPC.
pub struct PendingDecisionWait {
    pub payload: DecisionWaitPayload,
}

/// Internal lifecycle beyond frozen `DecisionDeliveryState` (includes `chosen`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternalDeliveryState {
    Pending,
    Chosen {
        response: DecisionResponse,
        responded_at_ms: i64,
    },
    Delivered {
        stdout_json: String,
        delivered_at_ms: i64,
    },
    EffectObserved {
        evidence: String,
        observed_at_ms: i64,
    },
    Expired,
    Failed {
        detail: String,
    },
}

impl InternalDeliveryState {
    pub fn wire_state(&self) -> DecisionDeliveryState {
        match self {
            Self::Pending | Self::Chosen { .. } => DecisionDeliveryState::Pending,
            Self::Delivered { .. } => DecisionDeliveryState::Delivered,
            Self::EffectObserved { .. } => DecisionDeliveryState::EffectObserved,
            Self::Expired => DecisionDeliveryState::Expired,
            Self::Failed { .. } => DecisionDeliveryState::Failed,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Delivered { .. }
                | Self::EffectObserved { .. }
                | Self::Expired
                | Self::Failed { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveDecision {
    pub request: DecisionRequest,
    pub internal_state: InternalDeliveryState,
    pub vendor_event: String,
    pub external_session_id: String,
    pub respondable_hook: Option<String>,
    pub tool_name: Option<String>,
    pub connection_id: String,
    pub vendor_context: Option<Value>,
    pub expires_at_ms: i64,
}

impl ActiveDecision {
    pub fn response_record(
        &self,
        response: &DecisionResponse,
        responded_at_ms: i64,
    ) -> DecisionResponseRecord {
        DecisionResponseRecord {
            request_id: self.request.id.clone(),
            response: response.clone(),
            responded_at_ms,
            delivery_state: self.internal_state.wire_state(),
            delivery_detail: match &self.internal_state {
                InternalDeliveryState::Failed { detail } => Some(detail.clone()),
                InternalDeliveryState::EffectObserved { evidence, .. } => Some(evidence.clone()),
                _ => None,
            },
        }
    }
}

//! Decision request/response contracts and fail-open hook constants.
//!
//! Overlay attention is an entry point only; Allow/Deny/answer controls appear on
//! a focused decision surface and/or dashboard. The broker enforces: no ephemeral
//! payload → no controls. Never claim vendor ACK without evidence.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::types::AgentSource;

/// Maximum hook wait before fail-open neutral response (milliseconds).
pub const DECISION_FAIL_OPEN_TIMEOUT_MS: u64 = 2_000;

/// Neutral stdout for fail-open vendor hooks.
pub const DECISION_HOOK_NEUTRAL_OUTPUT: &str = "{}";

/// Exit code for fail-open hook wrapper success.
pub const DECISION_HOOK_FAIL_OPEN_EXIT_CODE: i32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum DecisionKind {
    Approval,
    Permission,
    Question,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum DecisionResponseAction {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "type", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase", tag = "type")]
pub enum DecisionResponse {
    Action {
        action: DecisionResponseAction,
    },
    Answer {
        text: String,
    },
}

/// Honest delivery lifecycle; never claim vendor ACK without evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum DecisionDeliveryState {
    Pending,
    Delivered,
    EffectObserved,
    Expired,
    Failed,
}

/// Decision prompt surfaced to UI layers. Ephemeral vendor payload stays backend-only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct DecisionRequest {
    pub id: String,
    pub session_id: String,
    pub source: AgentSource,
    pub kind: DecisionKind,
    /// Redacted summary safe for overlay/dashboard display.
    pub summary: String,
    /// When false, UI must not render Allow/Deny/answer controls.
    pub has_actionable_payload: bool,
    #[ts(type = "number")]
    pub created_at_ms: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub expires_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, rename_all = "camelCase")]
pub struct DecisionResponseRecord {
    pub request_id: String,
    pub response: DecisionResponse,
    #[ts(type = "number")]
    pub responded_at_ms: i64,
    pub delivery_state: DecisionDeliveryState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub delivery_detail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_response_round_trips_tagged_union() {
        let response = DecisionResponse::Answer {
            text: "Use the existing helper".into(),
        };
        let value = serde_json::to_value(&response).expect("serialize");
        assert_eq!(value["type"], "answer");
        assert_eq!(value["text"], "Use the existing helper");

        let decoded: DecisionResponse = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded, response);
    }

    #[test]
    fn decision_request_denies_unknown_fields() {
        let request = DecisionRequest {
            id: "dec-1".into(),
            session_id: "sess-1".into(),
            source: AgentSource::ClaudeCode,
            kind: DecisionKind::Permission,
            summary: "Allow file write?".into(),
            has_actionable_payload: true,
            created_at_ms: 1,
            expires_at_ms: None,
        };
        let mut value = serde_json::to_value(&request).expect("serialize");
        value
            .as_object_mut()
            .expect("object")
            .insert("vendorPayload".into(), serde_json::json!({"secret": true}));

        assert!(serde_json::from_value::<DecisionRequest>(value).is_err());
    }

    #[test]
    fn fail_open_constants_match_wrapper_contract() {
        assert_eq!(DECISION_FAIL_OPEN_TIMEOUT_MS, 2_000);
        assert_eq!(DECISION_HOOK_NEUTRAL_OUTPUT, "{}");
        assert_eq!(DECISION_HOOK_FAIL_OPEN_EXIT_CODE, 0);
    }
}

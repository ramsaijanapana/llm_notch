//! Interactive vendor hook decision path for Claude Code respondable events.

use notch_adapters_claude_code::{capabilities, detect_version, normalize_event};
use notch_ipc::{DecisionWaitPayload, IngestClient, IngestPayload, IpcError};
use notch_protocol::{DECISION_HOOK_NEUTRAL_OUTPUT, DecisionKind};
use serde_json::Value;
use uuid::Uuid;

pub struct InteractiveDecisionPlan {
    pub ingest: IngestPayload,
    pub wait: DecisionWaitPayload,
}

pub fn plan_interactive_decision(
    source: &str,
    vendor_event: &str,
    value: &Value,
) -> Result<Option<InteractiveDecisionPlan>, IpcError> {
    if source != "claudeCode" {
        return Ok(None);
    }
    let normalized = normalize_event(vendor_event, value)
        .map_err(|err| IpcError::FrameRejected(format!("claude normalize failed: {err}")))?;
    let Some(decision_kind) = normalized.decision_kind else {
        return Ok(None);
    };
    let Some(respondable_hook) = normalized.respondable_hook else {
        return Ok(None);
    };

    let version = value
        .get("claude_code_version")
        .or_else(|| value.get("claudeCodeVersion"))
        .and_then(Value::as_str);
    let profile = detect_version(version);
    let caps = capabilities(&profile);
    let has_actionable_payload =
        caps.respond_decisions && caps.decision_response && decision_kind != DecisionKind::Question;

    let respondable_name = match respondable_hook {
        notch_adapters_claude_code::ClaudeRespondableHook::PermissionRequest => "permissionRequest",
        notch_adapters_claude_code::ClaudeRespondableHook::ExitPlanModePreToolUse => {
            "exitPlanModePreToolUse"
        }
    };

    let ingest = IngestPayload {
        source: source.to_string(),
        event: normalized.protocol_event,
        session_id: None,
        external_session_id: Some(normalized.external_session_id.clone()),
        label: None,
        workspace_label: normalized.workspace_label,
        status: normalized.status,
        attention: normalized.attention,
        summary: Some(normalized.summary.clone()),
        tool_name: normalized.tool_name.clone(),
        pid: None,
        process_started_at_ms: None,
        occurred_at_ms: None,
        terminal_session_id: None,
        tab_id: None,
        pane_id: None,
        window_handle: None,
    };
    notch_ipc::validate_ingest_payload(&ingest)?;

    let vendor_context = serde_json::json!({
        "claude_code_version": version,
        "updated_input": value.get("tool_input").or_else(|| value.get("toolInput")).cloned(),
    });
    let vendor_context_json = serde_json::to_string(&vendor_context)
        .map_err(|err| IpcError::FrameRejected(err.to_string()))?;

    let nonce = Uuid::new_v4().simple().to_string();
    let connection_id = format!(
        "{}:{}:{}",
        normalized.external_session_id,
        vendor_event,
        normalized.tool_name.as_deref().unwrap_or("none")
    );

    Ok(Some(InteractiveDecisionPlan {
        ingest,
        wait: DecisionWaitPayload {
            nonce,
            source: source.to_string(),
            vendor_event: vendor_event.to_string(),
            external_session_id: normalized.external_session_id,
            session_id: None,
            decision_kind: match decision_kind {
                DecisionKind::Approval => "approval".into(),
                DecisionKind::Question => "question".into(),
                DecisionKind::Permission => "permission".into(),
            },
            summary: normalized.summary,
            has_actionable_payload,
            respondable_hook: Some(respondable_name.into()),
            tool_name: normalized.tool_name,
            connection_id,
            vendor_context_json: Some(vendor_context_json),
            created_at_ms: chrono::Utc::now().timestamp_millis(),
        },
    }))
}

pub async fn execute_interactive_decision(plan: InteractiveDecisionPlan) -> String {
    let client = match IngestClient::discover() {
        Ok(client) => client,
        Err(_) => return DECISION_HOOK_NEUTRAL_OUTPUT.into(),
    };
    let ingest_id = format!("{}-ingest", plan.wait.nonce);
    if client.send_ingest(&ingest_id, &plan.ingest).await.is_err() {
        return DECISION_HOOK_NEUTRAL_OUTPUT.into();
    }
    client
        .request_decision(&plan.wait)
        .await
        .unwrap_or_else(|_| DECISION_HOOK_NEUTRAL_OUTPUT.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn fixture(name: &str) -> Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../integrations/fixtures/claude-code")
            .join(name);
        let raw = fs::read_to_string(path).expect("read fixture");
        serde_json::from_str(&raw).expect("parse fixture")
    }

    #[test]
    fn permission_request_plans_interactive_flow() {
        let value = fixture("permission-request-input.json");
        let plan = plan_interactive_decision("claudeCode", "PermissionRequest", &value)
            .expect("plan")
            .expect("interactive");
        // Fixture omits Claude version metadata; broker stays observation-only.
        assert!(!plan.wait.has_actionable_payload);
        assert_eq!(
            plan.wait.respondable_hook.as_deref(),
            Some("permissionRequest")
        );
        assert_eq!(plan.ingest.event, "attention");
    }

    #[test]
    fn known_claude_version_enables_actionable_controls() {
        let mut value = fixture("permission-request-input.json");
        value
            .as_object_mut()
            .expect("object")
            .insert("claude_code_version".into(), serde_json::json!("2.1.205"));
        let plan = plan_interactive_decision("claudeCode", "PermissionRequest", &value)
            .expect("plan")
            .expect("interactive");
        assert!(plan.wait.has_actionable_payload);
    }

    #[test]
    fn ordinary_tool_event_does_not_plan_decision() {
        let value = fixture("pre-tool-use-input.json");
        let plan = plan_interactive_decision("claudeCode", "PreToolUse", &value).expect("plan");
        assert!(plan.is_none());
    }
}

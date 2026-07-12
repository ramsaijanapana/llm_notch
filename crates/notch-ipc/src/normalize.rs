//! Normalize bounded hook payloads into protocol session records and events.

use notch_protocol::{
    AgentSession, AgentSource, AttentionKind, EventLevel, ProcessIdentity, SessionEvent,
    SessionEventKind, SessionStatus,
};

use crate::collector::verified_terminal_from_ingest;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{IpcError, IpcResult};
use crate::wire::IngestPayload;

/// Normalized ingest output consumed by the host application.
#[derive(Debug, Clone, PartialEq)]
pub enum NormalizedIngest {
    SessionUpsert(AgentSession),
    SessionRemove {
        session_id: String,
        source: AgentSource,
        external_session_id: Option<String>,
    },
    SessionEvent {
        event: SessionEvent,
        source: AgentSource,
        external_session_id: Option<String>,
        attention: Option<AttentionKind>,
    },
}

pub fn normalize_ingest(payload: &IngestPayload, now_ms: i64) -> IpcResult<NormalizedIngest> {
    let source = parse_source(&payload.source)?;
    let event = payload.event.to_ascii_lowercase();
    match event.as_str() {
        "sessionremove" | "session_remove" | "remove" => {
            let session_id = resolve_session_id(payload, source)?;
            Ok(NormalizedIngest::SessionRemove {
                session_id,
                source,
                external_session_id: payload.external_session_id.clone(),
            })
        }
        "sessionevent" | "event" | "tool" | "attention" | "status" | "lifecycle" => {
            let session_id = resolve_session_id(payload, source)?;
            let kind = parse_event_kind(&event)?;
            let level = parse_level(payload)?;
            let summary = payload
                .summary
                .clone()
                .unwrap_or_else(|| default_summary(kind));
            let event = SessionEvent {
                id: Uuid::new_v4(),
                session_id,
                sequence: 0,
                occurred_at_ms: payload.occurred_at_ms.unwrap_or(now_ms),
                kind,
                level,
                summary,
                tool_name: payload.tool_name.clone(),
            };
            Ok(NormalizedIngest::SessionEvent {
                event,
                source,
                external_session_id: payload.external_session_id.clone(),
                attention: parse_attention(payload),
            })
        }
        "sessionupsert" | "session_upsert" | "sessionstart" | "session_start" | "start"
        | "update" | "statuschange" | "status_change" => Ok(NormalizedIngest::SessionUpsert(
            build_session(payload, source, now_ms)?,
        )),
        "sessionend" | "session_end" | "end" | "complete" | "fail" => {
            let mut session = build_session(payload, source, now_ms)?;
            session.status = parse_status(payload).unwrap_or(SessionStatus::Completed);
            session.ended_at_ms = Some(payload.occurred_at_ms.unwrap_or(now_ms));
            Ok(NormalizedIngest::SessionUpsert(session))
        }
        other => Err(IpcError::FrameRejected(format!(
            "unsupported event `{other}`"
        ))),
    }
}

fn build_session(
    payload: &IngestPayload,
    source: AgentSource,
    now_ms: i64,
) -> IpcResult<AgentSession> {
    let external_session_id = payload
        .external_session_id
        .clone()
        .or_else(|| payload.session_id.clone())
        .ok_or_else(|| IpcError::FrameRejected("externalSessionId required".into()))?;
    let id = payload
        .session_id
        .clone()
        .unwrap_or_else(|| stable_session_id(source, &external_session_id));
    let started_at_ms = payload.occurred_at_ms.unwrap_or(now_ms);
    Ok(AgentSession {
        id,
        source,
        external_session_id,
        label: payload
            .label
            .clone()
            .unwrap_or_else(|| "Agent session".into()),
        workspace_label: payload.workspace_label.clone(),
        status: parse_status(payload).unwrap_or(SessionStatus::Running),
        attention: parse_attention(payload).unwrap_or(AttentionKind::None),
        started_at_ms,
        last_event_at_ms: payload.occurred_at_ms.unwrap_or(now_ms),
        ended_at_ms: None,
        process_root: payload
            .pid
            .zip(payload.process_started_at_ms)
            .map(|(pid, started_at_ms)| ProcessIdentity { pid, started_at_ms }),
        verified_terminal: verified_terminal_from_ingest(payload),
        latest_metric: None,
    })
}

fn resolve_session_id(payload: &IngestPayload, source: AgentSource) -> IpcResult<String> {
    if let Some(id) = &payload.session_id {
        return Ok(id.clone());
    }
    let external = payload
        .external_session_id
        .as_ref()
        .ok_or_else(|| IpcError::FrameRejected("sessionId or externalSessionId required".into()))?;
    Ok(stable_session_id(source, external))
}

pub fn stable_session_id(source: AgentSource, external_session_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{source:?}:{external_session_id}").as_bytes());
    hex::encode(&hasher.finalize()[..16])
}

fn parse_source(value: &str) -> IpcResult<AgentSource> {
    match value.to_ascii_lowercase().as_str() {
        "cursor" => Ok(AgentSource::Cursor),
        "claudecode" | "claude_code" | "claude-code" => Ok(AgentSource::ClaudeCode),
        "codex" => Ok(AgentSource::Codex),
        "gemini" | "geminicli" | "gemini-cli" => Ok(AgentSource::Gemini),
        "antigravitycli" | "antigravity-cli" | "antigravity" => Ok(AgentSource::AntigravityCli),
        "copilotcli" | "copilot-cli" | "copilot" => Ok(AgentSource::CopilotCli),
        "qwen" | "qwen-cli" | "qwencode" => Ok(AgentSource::Qwen),
        "generic" => Ok(AgentSource::Generic),
        "unknown" => Ok(AgentSource::Unknown),
        other => Err(IpcError::FrameRejected(format!(
            "unsupported source `{other}`"
        ))),
    }
}

fn parse_status(payload: &IngestPayload) -> Option<SessionStatus> {
    payload
        .status
        .as_ref()
        .and_then(|value| match value.to_ascii_lowercase().as_str() {
            "starting" => Some(SessionStatus::Starting),
            "running" => Some(SessionStatus::Running),
            "waiting" => Some(SessionStatus::Waiting),
            "paused" => Some(SessionStatus::Paused),
            "completed" => Some(SessionStatus::Completed),
            "failed" => Some(SessionStatus::Failed),
            "stale" => Some(SessionStatus::Stale),
            _ => None,
        })
}

fn parse_attention(payload: &IngestPayload) -> Option<AttentionKind> {
    payload
        .attention
        .as_ref()
        .and_then(|value| match value.to_ascii_lowercase().as_str() {
            "none" => Some(AttentionKind::None),
            "approval" => Some(AttentionKind::Approval),
            "question" => Some(AttentionKind::Question),
            "permission" => Some(AttentionKind::Permission),
            "error" => Some(AttentionKind::Error),
            _ => None,
        })
}

fn parse_event_kind(event: &str) -> IpcResult<SessionEventKind> {
    match event {
        "tool" => Ok(SessionEventKind::Tool),
        "attention" => Ok(SessionEventKind::Attention),
        "status" | "statuschange" | "status_change" => Ok(SessionEventKind::Status),
        "lifecycle" | "sessionevent" | "event" | "sessionstart" | "sessionend" | "start"
        | "end" => Ok(SessionEventKind::Lifecycle),
        other => Err(IpcError::FrameRejected(format!(
            "unsupported event kind `{other}`"
        ))),
    }
}

fn parse_level(payload: &IngestPayload) -> IpcResult<EventLevel> {
    let attention = parse_attention(payload);
    if attention == Some(AttentionKind::Error) {
        return Ok(EventLevel::Error);
    }
    if attention == Some(AttentionKind::Permission) || attention == Some(AttentionKind::Approval) {
        return Ok(EventLevel::Warning);
    }
    Ok(EventLevel::Info)
}

fn default_summary(kind: SessionEventKind) -> String {
    match kind {
        SessionEventKind::Lifecycle => "Session lifecycle update".into(),
        SessionEventKind::Tool => "Tool activity".into(),
        SessionEventKind::Attention => "Attention required".into(),
        SessionEventKind::Status => "Status update".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::IngestPayload;
    use notch_protocol::VerifiedTerminalContext;

    fn base_payload() -> IngestPayload {
        IngestPayload {
            source: "cursor".into(),
            event: "sessionStart".into(),
            session_id: None,
            external_session_id: Some("ext-1".into()),
            label: Some("Build feature".into()),
            workspace_label: Some("llm_notch".into()),
            status: Some("running".into()),
            attention: Some("none".into()),
            summary: None,
            tool_name: None,
            pid: Some(42),
            process_started_at_ms: Some(1_700_000_000_000),
            occurred_at_ms: Some(1_700_000_000_000),
            terminal_session_id: None,
            tab_id: None,
            pane_id: None,
            window_handle: None,
        }
    }

    #[test]
    fn normalizes_session_upsert_without_raw_fields() {
        let payload = base_payload();
        let normalized = normalize_ingest(&payload, 1_700_000_000_000).expect("normalize");
        match normalized {
            NormalizedIngest::SessionUpsert(session) => {
                assert_eq!(session.source, AgentSource::Cursor);
                assert_eq!(session.external_session_id, "ext-1");
                assert!(session.label.contains("Build"));
                assert!(session.verified_terminal.is_none());
            }
            _ => panic!("expected upsert"),
        }
    }

    #[test]
    fn normalizes_verified_terminal_when_collector_fields_present() {
        let mut payload = base_payload();
        payload.terminal_session_id = Some("0".into());
        payload.tab_id = Some("1".into());
        payload.pane_id = Some("0".into());
        let normalized = normalize_ingest(&payload, 1_700_000_000_000).expect("normalize");
        match normalized {
            NormalizedIngest::SessionUpsert(session) => {
                assert_eq!(
                    session.verified_terminal,
                    Some(VerifiedTerminalContext {
                        terminal_session_id: Some("0".into()),
                        tab_id: Some("1".into()),
                        pane_id: Some("0".into()),
                        window_handle: None,
                    })
                );
            }
            _ => panic!("expected upsert"),
        }
    }

    #[test]
    fn normalizes_partial_terminal_metadata_without_inventing_missing_fields() {
        let mut payload = base_payload();
        payload.terminal_session_id = Some("5720ee6d-6474-47b0-88db-fa7e10e60d37".into());
        let normalized = normalize_ingest(&payload, 1_700_000_000_000).expect("normalize");
        match normalized {
            NormalizedIngest::SessionUpsert(session) => {
                let terminal = session.verified_terminal.expect("terminal");
                assert_eq!(
                    terminal.terminal_session_id.as_deref(),
                    Some("5720ee6d-6474-47b0-88db-fa7e10e60d37")
                );
                assert!(terminal.tab_id.is_none());
                assert!(terminal.pane_id.is_none());
            }
            _ => panic!("expected upsert"),
        }
    }

    #[test]
    fn normalizes_gemini_source_aliases() {
        for alias in ["gemini", "geminicli", "gemini-cli"] {
            let payload = IngestPayload {
                source: alias.into(),
                event: "sessionStart".into(),
                session_id: None,
                external_session_id: Some("ext-gemini".into()),
                label: None,
                workspace_label: None,
                status: Some("running".into()),
                attention: None,
                summary: None,
                tool_name: None,
                pid: None,
                process_started_at_ms: None,
                occurred_at_ms: Some(1_700_000_000_000),
                terminal_session_id: None,
                tab_id: None,
                pane_id: None,
                window_handle: None,
            };
            let normalized = normalize_ingest(&payload, 1_700_000_000_000).expect("normalize");
            match normalized {
                NormalizedIngest::SessionUpsert(session) => {
                    assert_eq!(session.source, AgentSource::Gemini);
                }
                _ => panic!("expected upsert"),
            }
        }
    }

    #[test]
    fn normalizes_antigravity_source_aliases() {
        for alias in ["antigravityCli", "antigravity-cli", "antigravity"] {
            let payload = IngestPayload {
                source: alias.into(),
                event: "tool".into(),
                session_id: None,
                external_session_id: Some("conv-1".into()),
                label: None,
                workspace_label: Some("llm_notch".into()),
                status: None,
                attention: None,
                summary: Some("Tool activity observed".into()),
                tool_name: Some("run_command".into()),
                pid: None,
                process_started_at_ms: None,
                occurred_at_ms: Some(1_700_000_000_000),
                terminal_session_id: None,
                tab_id: None,
                pane_id: None,
                window_handle: None,
            };
            let normalized = normalize_ingest(&payload, 1_700_000_000_000).expect("normalize");
            match normalized {
                NormalizedIngest::SessionEvent { source, .. } => {
                    assert_eq!(source, AgentSource::AntigravityCli);
                }
                _ => panic!("expected session event"),
            }
        }
    }

    #[test]
    fn normalizes_copilot_source_aliases() {
        for alias in ["copilotCli", "copilot-cli", "copilot"] {
            let payload = IngestPayload {
                source: alias.into(),
                event: "sessionStart".into(),
                session_id: None,
                external_session_id: Some("copilot-1".into()),
                label: None,
                workspace_label: Some("llm_notch".into()),
                status: Some("running".into()),
                attention: None,
                summary: None,
                tool_name: None,
                pid: None,
                process_started_at_ms: None,
                occurred_at_ms: Some(1_700_000_000_000),
                terminal_session_id: None,
                tab_id: None,
                pane_id: None,
                window_handle: None,
            };
            let normalized = normalize_ingest(&payload, 1_700_000_000_000).expect("normalize");
            match normalized {
                NormalizedIngest::SessionUpsert(session) => {
                    assert_eq!(session.source, AgentSource::CopilotCli);
                }
                _ => panic!("expected upsert"),
            }
        }
    }

    #[test]
    fn normalizes_qwen_source_aliases() {
        for alias in ["qwen", "qwen-cli", "qwencode"] {
            let payload = IngestPayload {
                source: alias.into(),
                event: "sessionStart".into(),
                session_id: None,
                external_session_id: Some("qwen-1".into()),
                label: None,
                workspace_label: Some("llm_notch".into()),
                status: Some("running".into()),
                attention: None,
                summary: None,
                tool_name: None,
                pid: None,
                process_started_at_ms: None,
                occurred_at_ms: Some(1_700_000_000_000),
                terminal_session_id: None,
                tab_id: None,
                pane_id: None,
                window_handle: None,
            };
            let normalized = normalize_ingest(&payload, 1_700_000_000_000).expect("normalize");
            match normalized {
                NormalizedIngest::SessionUpsert(session) => {
                    assert_eq!(session.source, AgentSource::Qwen);
                }
                _ => panic!("expected upsert"),
            }
        }
    }
}

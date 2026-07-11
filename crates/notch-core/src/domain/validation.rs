use notch_protocol::{
    AgentSource, MAX_EVENT_SUMMARY_LEN, MAX_EXTERNAL_SESSION_ID_LEN, MAX_SESSION_ID_LEN,
    MAX_SESSION_LABEL_LEN, MAX_TOOL_NAME_LEN, MAX_WORKSPACE_LABEL_LEN,
};

use crate::error::{CoreError, CoreResult};

pub fn validate_bounded(value: &str, max: usize, field: &str) -> CoreResult<()> {
    if value.is_empty() {
        return Err(CoreError::Validation(format!("{field} must not be empty")));
    }
    if value.len() > max {
        return Err(CoreError::Validation(format!(
            "{field} exceeds {max} bytes"
        )));
    }
    Ok(())
}

pub fn validate_external_session_id(id: &str) -> CoreResult<()> {
    validate_bounded(id, MAX_EXTERNAL_SESSION_ID_LEN, "external_session_id")
}

pub fn validate_session_label(label: &str) -> CoreResult<()> {
    validate_bounded(label, MAX_SESSION_LABEL_LEN, "label")
}

pub fn validate_workspace_label(label: &str) -> CoreResult<()> {
    validate_bounded(label, MAX_WORKSPACE_LABEL_LEN, "workspace_label")
}

pub fn validate_event_summary(summary: &str) -> CoreResult<()> {
    validate_bounded(summary, MAX_EVENT_SUMMARY_LEN, "summary")
}

pub fn validate_tool_name(name: &str) -> CoreResult<()> {
    validate_bounded(name, MAX_TOOL_NAME_LEN, "tool_name")
}

pub fn validate_session_id(id: &str) -> CoreResult<()> {
    validate_bounded(id, MAX_SESSION_ID_LEN, "session_id")
}

/// Deterministic internal session id from source + external id.
pub fn session_id_for(source: AgentSource, external_session_id: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    format!("{source:?}").hash(&mut hasher);
    external_session_id.hash(&mut hasher);
    let hash = hasher.finish();
    let id = format!("s-{hash:016x}");
    id.chars().take(MAX_SESSION_ID_LEN).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::AgentSource;

    #[test]
    fn session_ids_are_deterministic() {
        let a = session_id_for(AgentSource::Cursor, "abc");
        let b = session_id_for(AgentSource::Cursor, "abc");
        assert_eq!(a, b);
        assert!(a.len() <= MAX_SESSION_ID_LEN);
    }
}

//! User-initiated session context navigation commands.

use std::sync::Arc;

use notch_protocol::ContextOpenTier;
use serde::Serialize;
use tauri::State;

use crate::commands::error::CommandError;
use crate::commands::validation::validate_session_id;
use crate::context::open_session_context;
use crate::state::HostState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSessionResult {
    pub context_open_tier: ContextOpenTier,
    pub activated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[tauri::command]
pub fn open_session(
    session_id: String,
    host: State<'_, Arc<HostState>>,
) -> Result<OpenSessionResult, CommandError> {
    validate_session_id(&session_id)?;
    let snapshot = host.snapshot();
    let Some(session) = snapshot
        .sessions
        .iter()
        .find(|session| session.id == session_id)
    else {
        return Err(CommandError::NotFound("session".into()));
    };

    let adapter = snapshot
        .adapters
        .iter()
        .find(|adapter| adapter.source == session.source);

    let result = open_session_context(session, adapter);
    Ok(OpenSessionResult {
        context_open_tier: result.context_open_tier,
        activated: result.activated,
        message: result.message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_session_result_serializes_camel_case() {
        let value = serde_json::to_value(OpenSessionResult {
            context_open_tier: ContextOpenTier::AppActivate,
            activated: true,
            message: Some("Activated editor application.".into()),
        })
        .expect("serialize");
        assert_eq!(value["contextOpenTier"], "appActivate");
        assert_eq!(value["activated"], true);
        assert_eq!(value["message"], "Activated editor application.");
    }
}

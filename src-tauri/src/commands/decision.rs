//! Tauri commands for the focused decision surface.

use std::sync::Arc;

use notch_decision::DecisionBroker;
use notch_protocol::{DecisionRequest, DecisionResponse, DecisionResponseRecord};
use tauri::State;

use crate::commands::error::CommandError;
use crate::commands::validation::validate_session_id;

#[tauri::command]
pub fn list_pending_decisions(
    broker: State<'_, Arc<DecisionBroker>>,
) -> Result<Vec<DecisionRequest>, CommandError> {
    broker
        .list_pending()
        .map_err(|error| CommandError::Internal(error.to_string()))
}

#[tauri::command]
pub fn submit_decision(
    request_id: String,
    response: DecisionResponse,
    broker: State<'_, Arc<DecisionBroker>>,
) -> Result<DecisionResponseRecord, CommandError> {
    validate_session_id(&request_id)?;
    broker
        .submit_decision(&request_id, response)
        .map_err(|error| {
            use notch_decision::broker::BrokerError;
            match error {
                BrokerError::NotFound => CommandError::NotFound("decision".into()),
                BrokerError::Expired => CommandError::Conflict("decision expired".into()),
                BrokerError::AlreadyFinalized => {
                    CommandError::Conflict("decision already finalized".into())
                }
                BrokerError::NotActionable => CommandError::InvalidRequest(
                    "decision has no actionable payload; controls must stay hidden".into(),
                ),
                BrokerError::BindingMismatch => {
                    CommandError::Conflict("decision nonce binding mismatch".into())
                }
                BrokerError::InvalidResponse(message) => CommandError::InvalidRequest(message),
                BrokerError::Store(message) | BrokerError::Adapter(message) => {
                    CommandError::Internal(message)
                }
            }
        })
}

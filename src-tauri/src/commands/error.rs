use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("not available: {0}")]
    NotAvailable(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<crate::stream::PublishError> for CommandError {
    fn from(value: crate::stream::PublishError) -> Self {
        Self::Internal(value.to_string())
    }
}

impl From<crate::stream::SubscribeError> for CommandError {
    fn from(value: crate::stream::SubscribeError) -> Self {
        match value {
            crate::stream::SubscribeError::InvalidWindowLabel => {
                Self::InvalidRequest("invalid window label".into())
            }
            crate::stream::SubscribeError::ReplayGap { after_sequence } => {
                Self::Conflict(format!("replay gap after sequence {after_sequence}"))
            }
            crate::stream::SubscribeError::DeliveryFailed => {
                Self::Internal("stream channel closed during replay".into())
            }
        }
    }
}

impl From<crate::services::ShortcutError> for CommandError {
    fn from(value: crate::services::ShortcutError) -> Self {
        match value {
            crate::services::ShortcutError::Conflict(message) => Self::Conflict(message),
            crate::services::ShortcutError::InvalidAccelerator(message) => {
                Self::InvalidRequest(message)
            }
            crate::services::ShortcutError::Registration(message) => Self::Internal(message),
        }
    }
}

impl From<crate::services::AutostartError> for CommandError {
    fn from(value: crate::services::AutostartError) -> Self {
        Self::Internal(value.to_string())
    }
}

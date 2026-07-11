use thiserror::Error;

/// Errors surfaced by core persistence and orchestration boundaries.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("repository unavailable")]
    RepositoryUnavailable,
    #[error("ingest rejected: {0}")]
    IngestRejected(String),
    #[error("unsupported capability: {0}")]
    UnsupportedCapability(String),
    #[error("invalid transition: {from:?} -> {to:?}")]
    InvalidTransition {
        from: notch_protocol::SessionStatus,
        to: notch_protocol::SessionStatus,
    },
    #[error("session capacity reached ({0})")]
    SessionCapacity(usize),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type CoreResult<T> = Result<T, CoreError>;

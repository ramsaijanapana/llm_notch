use notch_protocol::ConnectorErrorCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConnectorError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("plan not found")]
    PlanNotFound,
    #[error("plan expired")]
    PlanExpired,
    #[error("file changed since preview")]
    FileChangedSincePreview {
        expected: String,
        actual: String,
    },
    #[error("lock contention")]
    LockContention,
    #[error("path escapes scope")]
    PathEscapesScope(String),
    #[error("rollback hash mismatch")]
    RollbackHashMismatch,
    #[error("partial apply failure")]
    PartialApplyFailure,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl ConnectorError {
    pub fn code(&self) -> ConnectorErrorCode {
        match self {
            Self::PlanExpired => ConnectorErrorCode::PlanExpired,
            Self::PlanNotFound => ConnectorErrorCode::PlanNotFound,
            Self::FileChangedSincePreview { .. } => ConnectorErrorCode::FileChangedSincePreview,
            Self::LockContention => ConnectorErrorCode::LockContention,
            Self::PathEscapesScope(_) => ConnectorErrorCode::PathEscapesScope,
            Self::PartialApplyFailure => ConnectorErrorCode::PartialApplyFailure,
            Self::RollbackHashMismatch => ConnectorErrorCode::RollbackHashMismatch,
            Self::InvalidRequest(_) | Self::NotFound(_) => ConnectorErrorCode::InternalError,
            Self::Internal(_) => ConnectorErrorCode::InternalError,
        }
    }
}

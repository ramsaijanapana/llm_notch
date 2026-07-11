use thiserror::Error;

/// Errors raised by the IPC transport layer.
#[derive(Debug, Error)]
pub enum IpcError {
    #[error("transport not initialized")]
    NotInitialized,
    #[error("frame rejected: {0}")]
    FrameRejected(String),
    #[error("authentication failed")]
    AuthFailed,
    #[error("rate limit exceeded")]
    RateLimited,
    #[error("ingest queue full")]
    QueueFull,
    #[error("too many clients")]
    TooManyClients,
    #[error("peer rejected: {0}")]
    PeerRejected(String),
    #[error("read timed out")]
    ReadTimeout,
    #[error("descriptor unavailable")]
    DescriptorUnavailable,
    #[error("spool limit exceeded")]
    SpoolLimitExceeded,
    #[error("connection closed")]
    ConnectionClosed,
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type IpcResult<T> = Result<T, IpcError>;

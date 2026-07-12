//! Secure, transport-neutral foundations for monitoring remote agents over SSH stdio.

mod config;
mod deploy;
mod deploy_exec;
pub mod event_spool;
pub mod hook_ingest;
mod protocol;
mod reconnect;
mod relay_artifact;
mod session;
mod transport;

pub use config::{RemoteArchitecture, RemoteHostConfig, RemoteOs, RemoteTarget, SshHostKeyPolicy};
pub use deploy::{
    DeploymentError, DeploymentPlan, DeploymentStep, RelayArtifact, remote_hook_spool_guidance,
};
pub use deploy_exec::{
    DeployExecError, DeployTransport, DeployTransportError, DeploymentExecutor, DeploymentOutcome,
    OpenSshDeployTransport,
};
pub use event_spool::EventSpoolReader;
pub use hook_ingest::{HookIngestError, RelayHookPayload, normalize_hook_payload};
pub use protocol::{
    MAX_REMOTE_FRAME_BYTES, PROTOCOL_VERSION, RelayControl, RelayFrame, RelayHello, RelayPayload,
    ResumeCursor,
};
pub use reconnect::{ConnectionState, ReconnectPolicy};
pub use relay_artifact::{
    RelayArtifactError, remote_target_triple, resolve_relay_artifact, rust_triple_for_target,
    sidecar_filename_for_target,
};
pub use session::{RelaySession, RelaySessionError, RelaySessionSnapshot, RemoteRelayManager};
pub use transport::{
    DEFAULT_REMOTE_BIN_DIRECTORY, DEFAULT_REMOTE_RUNTIME_DIRECTORY, DirectRelayTransport,
    OpenSshTransport, RemoteConnection, RemoteTransport, TransportError,
};

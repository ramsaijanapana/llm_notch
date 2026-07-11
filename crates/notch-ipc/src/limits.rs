//! IPC wire and resource limits enforced by the ingest transport.

/// IPC wire protocol version (distinct from [`notch_protocol::PROTOCOL_VERSION`]).
pub const IPC_WIRE_VERSION: u16 = 1;

/// Maximum serialized JSON frame body size (64 KiB).
pub const MAX_FRAME_BYTES: usize = 65_536;

/// Read timeout while waiting for the next frame (milliseconds).
pub const READ_TIMEOUT_MS: u64 = 2_000;

/// Maximum concurrent authenticated hook clients.
pub const MAX_CLIENTS: usize = 16;

/// Sustained ingest events per client (events/second).
pub const MAX_EVENTS_PER_SEC: u32 = 20;

/// Burst allowance per client.
pub const MAX_BURST_PER_CLIENT: u32 = 128;

/// Global ingest events per second across all clients.
pub const MAX_GLOBAL_EVENTS_PER_SEC: u32 = 500;

/// Bounded queue of normalized ingest messages awaiting consumption.
pub const MAX_INGEST_QUEUE: usize = 4096;

/// Maximum UTF-8 byte length for wire request identifiers.
pub const MAX_REQUEST_ID_LEN: usize = 64;

/// Maximum UTF-8 byte length for wire error codes.
pub const MAX_ERROR_CODE_LEN: usize = 64;

/// Maximum UTF-8 byte length for wire error messages.
pub const MAX_ERROR_MESSAGE_LEN: usize = 256;

/// 256-bit auth token length in bytes.
pub const AUTH_TOKEN_BYTES: usize = 32;

/// Runtime descriptor filename (user-only permissions).
pub const DESCRIPTOR_FILENAME: &str = "ingest.descriptor.json";

/// Unix domain socket filename inside the runtime directory.
pub const SOCKET_FILENAME: &str = "ingest.sock";

/// Windows named-pipe / local-socket identifier inside the runtime directory.
pub const PIPE_FILENAME: &str = "ingest.pipe";

/// Spool subdirectory for offline hook events.
pub const SPOOL_DIRNAME: &str = "spool";

/// Maximum spooled event files when the host is unavailable.
pub const MAX_SPOOL_FILES: usize = 1_000;

/// Maximum total spool directory size in bytes (10 MiB).
pub const MAX_SPOOL_BYTES: u64 = 10 * 1024 * 1024;

/// Client wait for a durable host acceptance acknowledgement (milliseconds).
pub const ACK_WAIT_MS: u64 = 2_000;

/// Maximum ephemeral vendor context JSON attached to decision waits.
pub const MAX_DECISION_CONTEXT_BYTES: usize = 8_192;

/// Server wait for the host core to accept and persist an ingest (milliseconds).
pub const HOST_ACCEPT_WAIT_MS: u64 = 1_500;

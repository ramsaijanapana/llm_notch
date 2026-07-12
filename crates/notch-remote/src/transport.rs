use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use thiserror::Error;

use crate::hook_ingest::{RelayHookPayload, validate_hook_payload};
use crate::{
    ConnectionState, MAX_REMOTE_FRAME_BYTES, RelayControl, RelayFrame, RelayHello,
    RemoteHostConfig, ResumeCursor,
};

const MAX_HELLO_BYTES: usize = 4 * 1024;
pub const DEFAULT_REMOTE_RUNTIME_DIRECTORY: &str = "~/.llm-notch";
pub const DEFAULT_REMOTE_BIN_DIRECTORY: &str = "~/.llm-notch/bin";
const DEFAULT_REMOTE_RELAY_PATH: &str = "~/.llm-notch/bin/llm-notch-relay";

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum TransportError {
    #[error("SSH executable is unavailable")]
    SshUnavailable,
    #[error("SSH host verification failed")]
    HostVerificationFailed,
    #[error("SSH authentication failed")]
    AuthenticationFailed,
    #[error("relay protocol failed: {0}")]
    Protocol(String),
    #[error("remote transport disconnected")]
    Disconnected,
    #[error("SSH process failed: {0}")]
    Process(String),
}

pub trait RemoteConnection: Send {
    fn state(&self) -> ConnectionState;
    fn hello(&self) -> &RelayHello;
    fn receive(&mut self) -> Result<Option<RelayFrame>, TransportError>;
    fn acknowledge(&mut self, cursor: ResumeCursor) -> Result<(), TransportError>;
    fn inject_hook(&mut self, payload: &RelayHookPayload) -> Result<(), TransportError>;
    fn close(&mut self) -> Result<(), TransportError>;
    fn is_alive(&mut self) -> Result<bool, TransportError>;
}

pub trait RemoteTransport: Send + Sync {
    fn connect(
        &self,
        host: &RemoteHostConfig,
        resume: ResumeCursor,
    ) -> Result<Box<dyn RemoteConnection>, TransportError>;
}

#[derive(Debug, Clone)]
pub struct OpenSshTransport {
    executable: String,
    remote_relay_path: String,
    event_spool_dir: Option<String>,
}

impl Default for OpenSshTransport {
    fn default() -> Self {
        Self {
            executable: "ssh".into(),
            remote_relay_path: DEFAULT_REMOTE_RELAY_PATH.into(),
            event_spool_dir: None,
        }
    }
}

impl OpenSshTransport {
    pub fn new(executable: impl Into<String>) -> Self {
        Self {
            executable: executable.into(),
            ..Self::default()
        }
    }

    pub fn with_remote_relay_path(
        executable: impl Into<String>,
        remote_relay_path: impl Into<String>,
    ) -> Result<Self, TransportError> {
        let remote_relay_path = remote_relay_path.into();
        if !valid_remote_relay_path(&remote_relay_path) {
            return Err(TransportError::Protocol(
                "remote relay path is not an allowed private path".into(),
            ));
        }
        Ok(Self {
            executable: executable.into(),
            remote_relay_path,
            event_spool_dir: None,
        })
    }

    pub fn with_event_spool_dir(
        mut self,
        event_spool_dir: impl Into<String>,
    ) -> Result<Self, TransportError> {
        let event_spool_dir = event_spool_dir.into();
        if !valid_private_remote_path(&event_spool_dir) {
            return Err(TransportError::Protocol(
                "event spool directory is not an allowed private path".into(),
            ));
        }
        self.event_spool_dir = Some(event_spool_dir);
        Ok(self)
    }
}

impl RemoteTransport for OpenSshTransport {
    fn connect(
        &self,
        host: &RemoteHostConfig,
        resume: ResumeCursor,
    ) -> Result<Box<dyn RemoteConnection>, TransportError> {
        let mut args = host
            .ssh_args()
            .map_err(|error| TransportError::Protocol(error.to_string()))?;
        args.extend([
            self.remote_relay_path.clone(),
            "--host-id".into(),
            host.id.clone(),
            "--resume".into(),
            resume.last_sequence.to_string(),
        ]);
        if let Some(event_spool_dir) = &self.event_spool_dir {
            args.extend(["--event-spool".into(), event_spool_dir.clone()]);
        }

        let mut child = Command::new(&self.executable)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    TransportError::SshUnavailable
                } else {
                    TransportError::Process(error.to_string())
                }
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| TransportError::Process("SSH stdin was not piped".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TransportError::Process("SSH stdout was not piped".into()))?;
        let mut stdout = BufReader::new(stdout);
        let hello: RelayHello = match read_json_line(&mut stdout, MAX_HELLO_BYTES) {
            Ok(Some(hello)) => hello,
            Ok(None) => return Err(classify_early_exit(&mut child)),
            Err(error) => {
                let _ = child.kill();
                return Err(error);
            }
        };
        hello
            .validate()
            .map_err(|error| TransportError::Protocol(error.to_string()))?;
        if hello.host_id != host.id || hello.resume != resume {
            let _ = child.kill();
            return Err(TransportError::Protocol(
                "relay hello does not match the requested host or resume cursor".into(),
            ));
        }

        Ok(Box::new(OpenSshConnection {
            child,
            stdin,
            stdout,
            hello,
            cursor: resume,
            state: ConnectionState::Streaming,
        }))
    }
}

/// Spawns a local relay executable over stdio. Intended for tests and local plumbing.
#[derive(Debug, Clone)]
pub struct DirectRelayTransport {
    executable: String,
    event_spool_dir: Option<String>,
}

impl DirectRelayTransport {
    pub fn new(executable: impl Into<String>) -> Self {
        Self {
            executable: executable.into(),
            event_spool_dir: None,
        }
    }

    pub fn with_event_spool_dir(
        executable: impl Into<String>,
        event_spool_dir: impl Into<String>,
    ) -> Self {
        Self {
            executable: executable.into(),
            event_spool_dir: Some(event_spool_dir.into()),
        }
    }
}

impl RemoteTransport for DirectRelayTransport {
    fn connect(
        &self,
        host: &RemoteHostConfig,
        resume: ResumeCursor,
    ) -> Result<Box<dyn RemoteConnection>, TransportError> {
        host.validate()
            .map_err(|error| TransportError::Protocol(error.to_string()))?;
        let mut command = Command::new(&self.executable);
        command.args([
            "--host-id",
            &host.id,
            "--resume",
            &resume.last_sequence.to_string(),
        ]);
        if let Some(event_spool_dir) = &self.event_spool_dir {
            command.args(["--event-spool", event_spool_dir]);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    TransportError::Process(format!(
                        "relay executable is unavailable: {}",
                        self.executable
                    ))
                } else {
                    TransportError::Process(error.to_string())
                }
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| TransportError::Process("relay stdin was not piped".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TransportError::Process("relay stdout was not piped".into()))?;
        let mut stdout = BufReader::new(stdout);
        let hello: RelayHello = match read_json_line(&mut stdout, MAX_HELLO_BYTES) {
            Ok(Some(hello)) => hello,
            Ok(None) => return Err(classify_early_exit(&mut child)),
            Err(error) => {
                let _ = child.kill();
                return Err(error);
            }
        };
        hello
            .validate()
            .map_err(|error| TransportError::Protocol(error.to_string()))?;
        if hello.host_id != host.id || hello.resume != resume {
            let _ = child.kill();
            return Err(TransportError::Protocol(
                "relay hello does not match the requested host or resume cursor".into(),
            ));
        }

        Ok(Box::new(RelayStdioConnection {
            child,
            stdin,
            stdout,
            hello,
            cursor: resume,
            state: ConnectionState::Streaming,
        }))
    }
}

struct RelayStdioConnection {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    hello: RelayHello,
    cursor: ResumeCursor,
    state: ConnectionState,
}

impl RemoteConnection for RelayStdioConnection {
    fn state(&self) -> ConnectionState {
        self.state
    }

    fn hello(&self) -> &RelayHello {
        &self.hello
    }

    fn receive(&mut self) -> Result<Option<RelayFrame>, TransportError> {
        let frame: Option<RelayFrame> = read_json_line(&mut self.stdout, MAX_REMOTE_FRAME_BYTES)?;
        let Some(frame) = frame else {
            self.state = ConnectionState::Disconnected;
            return Ok(None);
        };
        frame
            .validate_after(&self.cursor)
            .map_err(|error| TransportError::Protocol(error.to_string()))?;
        self.cursor.last_sequence = frame.sequence;
        Ok(Some(frame))
    }

    fn acknowledge(&mut self, cursor: ResumeCursor) -> Result<(), TransportError> {
        if cursor.last_sequence > self.cursor.last_sequence {
            return Err(TransportError::Protocol(
                "cannot acknowledge an unreceived sequence".into(),
            ));
        }
        write_json_line(&mut self.stdin, &RelayControl::Acknowledge { cursor })
    }

    fn inject_hook(&mut self, payload: &RelayHookPayload) -> Result<(), TransportError> {
        validate_hook_payload(payload)
            .map_err(|_| TransportError::Protocol("invalid hook payload".into()))?;
        write_json_line(
            &mut self.stdin,
            &RelayControl::InjectHook {
                payload: payload.clone(),
            },
        )
    }

    fn close(&mut self) -> Result<(), TransportError> {
        if self.state == ConnectionState::Disconnected {
            return Ok(());
        }
        let _ = write_json_line(&mut self.stdin, &RelayControl::Shutdown);
        drop(self.child.stdin.take());
        if self
            .child
            .try_wait()
            .map_err(|error| TransportError::Process(error.to_string()))?
            .is_none()
        {
            self.child
                .kill()
                .map_err(|error| TransportError::Process(error.to_string()))?;
        }
        let _ = self.child.wait();
        self.state = ConnectionState::Disconnected;
        Ok(())
    }

    fn is_alive(&mut self) -> Result<bool, TransportError> {
        process_is_alive(&mut self.child)
    }
}

impl Drop for RelayStdioConnection {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

struct OpenSshConnection {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    hello: RelayHello,
    cursor: ResumeCursor,
    state: ConnectionState,
}

impl RemoteConnection for OpenSshConnection {
    fn state(&self) -> ConnectionState {
        self.state
    }

    fn hello(&self) -> &RelayHello {
        &self.hello
    }

    fn receive(&mut self) -> Result<Option<RelayFrame>, TransportError> {
        let frame: Option<RelayFrame> = read_json_line(&mut self.stdout, MAX_REMOTE_FRAME_BYTES)?;
        let Some(frame) = frame else {
            self.state = ConnectionState::Disconnected;
            return Ok(None);
        };
        frame
            .validate_after(&self.cursor)
            .map_err(|error| TransportError::Protocol(error.to_string()))?;
        self.cursor.last_sequence = frame.sequence;
        Ok(Some(frame))
    }

    fn acknowledge(&mut self, cursor: ResumeCursor) -> Result<(), TransportError> {
        if cursor.last_sequence > self.cursor.last_sequence {
            return Err(TransportError::Protocol(
                "cannot acknowledge an unreceived sequence".into(),
            ));
        }
        write_json_line(&mut self.stdin, &RelayControl::Acknowledge { cursor })
    }

    fn inject_hook(&mut self, payload: &RelayHookPayload) -> Result<(), TransportError> {
        validate_hook_payload(payload)
            .map_err(|_| TransportError::Protocol("invalid hook payload".into()))?;
        write_json_line(
            &mut self.stdin,
            &RelayControl::InjectHook {
                payload: payload.clone(),
            },
        )
    }

    fn close(&mut self) -> Result<(), TransportError> {
        if self.state == ConnectionState::Disconnected {
            return Ok(());
        }
        let _ = write_json_line(&mut self.stdin, &RelayControl::Shutdown);
        drop(self.child.stdin.take());
        if self
            .child
            .try_wait()
            .map_err(|error| TransportError::Process(error.to_string()))?
            .is_none()
        {
            self.child
                .kill()
                .map_err(|error| TransportError::Process(error.to_string()))?;
        }
        let _ = self.child.wait();
        self.state = ConnectionState::Disconnected;
        Ok(())
    }

    fn is_alive(&mut self) -> Result<bool, TransportError> {
        process_is_alive(&mut self.child)
    }
}

impl Drop for OpenSshConnection {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn read_json_line<T: serde::de::DeserializeOwned, R: BufRead>(
    reader: &mut R,
    max_bytes: usize,
) -> Result<Option<T>, TransportError> {
    let bytes = match read_bounded_line(reader, max_bytes)? {
        Some(bytes) => bytes,
        None => return Ok(None),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| TransportError::Protocol(format!("invalid relay JSON: {error}")))
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    max_bytes: usize,
) -> Result<Option<Vec<u8>>, TransportError> {
    let mut bytes = Vec::new();
    loop {
        let buffer = reader
            .fill_buf()
            .map_err(|error| TransportError::Process(error.to_string()))?;
        if buffer.is_empty() {
            return Ok(if bytes.is_empty() {
                None
            } else {
                Err(TransportError::Protocol(
                    "relay emitted an oversized or unterminated frame".into(),
                ))?
            });
        }
        if let Some(position) = buffer.iter().position(|byte| *byte == b'\n') {
            bytes.extend_from_slice(&buffer[..=position]);
            reader.consume(position + 1);
            break;
        }
        if bytes.len() + buffer.len() > max_bytes {
            return Err(TransportError::Protocol(
                "relay emitted an oversized or unterminated frame".into(),
            ));
        }
        let consumed = buffer.len();
        bytes.extend_from_slice(buffer);
        reader.consume(consumed);
    }
    if bytes.len() > max_bytes || !bytes.ends_with(b"\n") {
        return Err(TransportError::Protocol(
            "relay emitted an oversized or unterminated frame".into(),
        ));
    }
    Ok(Some(bytes))
}

fn write_json_line<T: serde::Serialize>(
    writer: &mut impl Write,
    value: &T,
) -> Result<(), TransportError> {
    serde_json::to_writer(&mut *writer, value)
        .map_err(|error| TransportError::Protocol(error.to_string()))?;
    writer
        .write_all(b"\n")
        .and_then(|_| writer.flush())
        .map_err(|_| TransportError::Disconnected)
}

fn classify_early_exit(child: &mut Child) -> TransportError {
    let mut stderr = String::new();
    if let Some(stream) = child.stderr.take() {
        let _ = stream.take(8 * 1024).read_to_string(&mut stderr);
    }
    let normalized = stderr.to_ascii_lowercase();
    if normalized.contains("host key verification failed")
        || normalized.contains("remote host identification has changed")
    {
        TransportError::HostVerificationFailed
    } else if normalized.contains("permission denied") {
        TransportError::AuthenticationFailed
    } else {
        TransportError::Process(stderr.trim().to_string())
    }
}

fn valid_private_remote_path(value: &str) -> bool {
    value.starts_with("~/.")
        && value.len() <= 160
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'~' | b'/' | b'.' | b'-' | b'_')
        })
        && !value.contains("..")
}

fn valid_remote_relay_path(value: &str) -> bool {
    valid_private_remote_path(value)
}

fn process_is_alive(child: &mut Child) -> Result<bool, TransportError> {
    match child
        .try_wait()
        .map_err(|error| TransportError::Process(error.to_string()))?
    {
        Some(_) => Ok(false),
        None => Ok(true),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::{PROTOCOL_VERSION, RelayPayload};

    struct FakeConnection {
        hello: RelayHello,
        state: ConnectionState,
        frames: Vec<RelayFrame>,
    }

    impl RemoteConnection for FakeConnection {
        fn state(&self) -> ConnectionState {
            self.state
        }

        fn hello(&self) -> &RelayHello {
            &self.hello
        }

        fn receive(&mut self) -> Result<Option<RelayFrame>, TransportError> {
            Ok(self.frames.pop())
        }

        fn acknowledge(&mut self, cursor: ResumeCursor) -> Result<(), TransportError> {
            self.hello.resume = cursor;
            Ok(())
        }

        fn inject_hook(
            &mut self,
            _payload: &crate::hook_ingest::RelayHookPayload,
        ) -> Result<(), TransportError> {
            Ok(())
        }

        fn close(&mut self) -> Result<(), TransportError> {
            self.state = ConnectionState::Disconnected;
            Ok(())
        }

        fn is_alive(&mut self) -> Result<bool, TransportError> {
            Ok(self.state == ConnectionState::Streaming)
        }
    }

    #[test]
    fn connection_contract_supports_resume_and_clean_close() {
        let mut connection = FakeConnection {
            hello: RelayHello {
                protocol_version: PROTOCOL_VERSION,
                host_id: "host-1".into(),
                connection_nonce: "a".repeat(64),
                resume: ResumeCursor::default(),
            },
            state: ConnectionState::Streaming,
            frames: vec![RelayFrame {
                sequence: 1,
                payload: RelayPayload::Heartbeat,
            }],
        };
        assert!(connection.receive().unwrap().is_some());
        connection
            .acknowledge(ResumeCursor { last_sequence: 1 })
            .unwrap();
        connection.close().unwrap();
        assert_eq!(connection.state(), ConnectionState::Disconnected);
    }

    #[test]
    fn bounded_line_reader_rejects_unterminated_and_oversized_frames() {
        let mut unterminated = Cursor::new(br#"{"protocolVersion":1}"#);
        let unterminated_result: Result<Option<serde_json::Value>, _> =
            read_json_line(&mut unterminated, 128);
        assert!(matches!(
            unterminated_result,
            Err(TransportError::Protocol(_))
        ));

        let mut oversized = Cursor::new(format!("{}\n", "x".repeat(129)));
        let oversized_result: Result<Option<serde_json::Value>, _> =
            read_json_line(&mut oversized, 128);
        assert!(matches!(oversized_result, Err(TransportError::Protocol(_))));
    }

    #[test]
    fn remote_relay_path_cannot_inject_shell_syntax() {
        for path in ["relay", "~/.relay;bad", "~/.relay path", "~/../relay"] {
            assert!(OpenSshTransport::with_remote_relay_path("ssh", path).is_err());
        }
        assert!(
            OpenSshTransport::with_remote_relay_path("ssh", "~/.llm-notch/bin/llm-notch-relay")
                .is_ok()
        );
    }

    #[test]
    fn event_spool_dir_rejects_shell_syntax_and_accepts_private_runtime_dir() {
        for path in ["relay", "~/.relay;bad", "~/.relay space", "~/../relay"] {
            assert!(
                OpenSshTransport::new("ssh")
                    .with_event_spool_dir(path)
                    .is_err()
            );
        }
        assert!(
            OpenSshTransport::new("ssh")
                .with_event_spool_dir(DEFAULT_REMOTE_RUNTIME_DIRECTORY)
                .is_ok()
        );
    }
}

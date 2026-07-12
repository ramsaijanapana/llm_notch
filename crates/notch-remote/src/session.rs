use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::hook_ingest::RelayHookPayload;
use crate::{
    ConnectionState, ReconnectPolicy, RelayFrame, RemoteConnection, RemoteHostConfig,
    RemoteTransport, ResumeCursor, TransportError,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RelaySessionError {
    #[error("relay session is already active for host {0}")]
    AlreadyActive(String),
    #[error("relay session is not active for host {0}")]
    NotActive(String),
    #[error("relay session failed and requires an explicit restart")]
    RequiresRestart,
    #[error("remote transport error: {0}")]
    Transport(#[from] TransportError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelaySessionSnapshot {
    pub host_id: String,
    pub state: ConnectionState,
    pub resume: ResumeCursor,
    pub last_error: Option<String>,
    pub connection_nonce: Option<String>,
    /// True only while the underlying transport process is still running.
    pub process_alive: bool,
}

pub struct RelaySession {
    host: RemoteHostConfig,
    transport: Box<dyn RemoteTransport>,
    reconnect: ReconnectPolicy,
    connection: Option<Box<dyn RemoteConnection>>,
    state: ConnectionState,
    resume: ResumeCursor,
    last_error: Option<TransportError>,
    attempt: u16,
}

impl RelaySession {
    pub fn new(host: RemoteHostConfig, transport: Box<dyn RemoteTransport>) -> Self {
        Self::with_policy(host, transport, ReconnectPolicy::default())
    }

    pub fn with_policy(
        host: RemoteHostConfig,
        transport: Box<dyn RemoteTransport>,
        reconnect: ReconnectPolicy,
    ) -> Self {
        Self::with_resume(host, transport, reconnect, ResumeCursor::default())
    }

    pub fn with_resume(
        host: RemoteHostConfig,
        transport: Box<dyn RemoteTransport>,
        reconnect: ReconnectPolicy,
        resume: ResumeCursor,
    ) -> Self {
        Self {
            host,
            transport,
            reconnect,
            connection: None,
            state: ConnectionState::Disconnected,
            resume,
            last_error: None,
            attempt: 0,
        }
    }

    pub fn host_id(&self) -> &str {
        &self.host.id
    }

    pub fn snapshot(&mut self) -> RelaySessionSnapshot {
        let process_alive = self
            .connection
            .as_mut()
            .map(|connection| connection.is_alive().unwrap_or(false))
            .unwrap_or(false);
        if self.connection.is_some() && !process_alive {
            self.state = ConnectionState::Disconnected;
            self.connection = None;
        }
        RelaySessionSnapshot {
            host_id: self.host.id.clone(),
            state: self.state,
            resume: self.resume,
            last_error: self.last_error.as_ref().map(|error| error.to_string()),
            connection_nonce: self
                .connection
                .as_ref()
                .filter(|_| process_alive)
                .map(|connection| connection.hello().connection_nonce.clone()),
            process_alive,
        }
    }

    pub fn start(&mut self) -> Result<(), RelaySessionError> {
        if matches!(self.state, ConnectionState::Streaming) && self.connection.is_some() {
            return Err(RelaySessionError::AlreadyActive(self.host.id.clone()));
        }
        if self.state == ConnectionState::Failed {
            return Err(RelaySessionError::RequiresRestart);
        }

        self.state = ConnectionState::Connecting;
        self.last_error = None;
        match self.transport.connect(&self.host, self.resume) {
            Ok(connection) => {
                self.connection = Some(connection);
                self.state = ConnectionState::Streaming;
                self.attempt = 0;
                Ok(())
            }
            Err(error) => {
                self.fail(error);
                Err(RelaySessionError::Transport(
                    self.last_error
                        .clone()
                        .unwrap_or(TransportError::Disconnected),
                ))
            }
        }
    }

    pub fn stop(&mut self) -> Result<(), RelaySessionError> {
        if let Some(mut connection) = self.connection.take() {
            connection.close()?;
        }
        self.state = ConnectionState::Disconnected;
        self.attempt = 0;
        self.last_error = None;
        Ok(())
    }

    pub fn receive(&mut self) -> Result<Option<RelayFrame>, RelaySessionError> {
        let Some(connection) = self.connection.as_mut() else {
            return Err(RelaySessionError::NotActive(self.host.id.clone()));
        };
        if !connection.is_alive()? {
            self.handle_disconnect();
            return Ok(None);
        }
        match connection.receive()? {
            Some(frame) => {
                if frame.sequence > self.resume.last_sequence {
                    self.resume.last_sequence = frame.sequence;
                }
                Ok(Some(frame))
            }
            None => {
                self.handle_disconnect();
                Ok(None)
            }
        }
    }

    pub fn acknowledge(&mut self, cursor: ResumeCursor) -> Result<(), RelaySessionError> {
        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| RelaySessionError::NotActive(self.host.id.clone()))?;
        connection.acknowledge(cursor)?;
        if cursor.last_sequence > self.resume.last_sequence {
            self.resume = cursor;
        }
        Ok(())
    }

    pub fn inject_hook(&mut self, payload: RelayHookPayload) -> Result<(), RelaySessionError> {
        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| RelaySessionError::NotActive(self.host.id.clone()))?;
        connection
            .inject_hook(&payload)
            .map_err(RelaySessionError::Transport)
    }

    /// Attempts a reconnect when the session is disconnected or in backoff.
    pub fn tick_reconnect(&mut self, jitter_basis_points: i16) -> Result<bool, RelaySessionError> {
        if matches!(
            self.state,
            ConnectionState::Streaming | ConnectionState::Connecting
        ) {
            return Ok(false);
        }
        if self.state == ConnectionState::Failed {
            return Err(RelaySessionError::RequiresRestart);
        }

        let delay = self
            .reconnect
            .delay_ms(self.attempt, jitter_basis_points)
            .ok_or(RelaySessionError::RequiresRestart)?;
        self.state = ConnectionState::Backoff {
            attempt: self.attempt,
            delay_ms: delay,
        };
        self.attempt = self.attempt.saturating_add(1);
        self.state = ConnectionState::Connecting;
        match self.transport.connect(&self.host, self.resume) {
            Ok(connection) => {
                self.connection = Some(connection);
                self.state = ConnectionState::Streaming;
                self.attempt = 0;
                self.last_error = None;
                Ok(true)
            }
            Err(error) => {
                self.fail(error);
                Err(RelaySessionError::Transport(
                    self.last_error
                        .clone()
                        .unwrap_or(TransportError::Disconnected),
                ))
            }
        }
    }

    fn handle_disconnect(&mut self) {
        if let Some(mut connection) = self.connection.take() {
            let _ = connection.close();
        }
        self.state = ConnectionState::Disconnected;
    }

    fn fail(&mut self, error: TransportError) {
        if let Some(mut connection) = self.connection.take() {
            let _ = connection.close();
        }
        self.last_error = Some(error);
        self.state = if self.attempt >= self.reconnect.max_attempts {
            ConnectionState::Failed
        } else {
            ConnectionState::Disconnected
        };
    }
}

pub struct RemoteRelayManager {
    sessions: Vec<RelaySession>,
}

impl RemoteRelayManager {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
        }
    }

    pub fn register(&mut self, session: RelaySession) -> Result<(), RelaySessionError> {
        if self
            .sessions
            .iter()
            .any(|entry| entry.host_id() == session.host_id())
        {
            return Err(RelaySessionError::AlreadyActive(session.host_id().into()));
        }
        self.sessions.push(session);
        Ok(())
    }

    pub fn get(&self, host_id: &str) -> Option<&RelaySession> {
        self.sessions
            .iter()
            .find(|session| session.host_id() == host_id)
    }

    pub fn get_mut(&mut self, host_id: &str) -> Option<&mut RelaySession> {
        self.sessions
            .iter_mut()
            .find(|session| session.host_id() == host_id)
    }

    pub fn remove(&mut self, host_id: &str) -> Result<(), RelaySessionError> {
        let index = self
            .sessions
            .iter()
            .position(|session| session.host_id() == host_id)
            .ok_or_else(|| RelaySessionError::NotActive(host_id.into()))?;
        self.sessions.remove(index).stop()?;
        Ok(())
    }

    pub fn snapshots(&mut self) -> Vec<RelaySessionSnapshot> {
        self.sessions
            .iter_mut()
            .map(RelaySession::snapshot)
            .collect()
    }
}

impl Default for RemoteRelayManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PROTOCOL_VERSION, RelayHello, RelayPayload, ResumeCursor, TransportError};

    struct ScriptedTransport {
        outcomes: std::collections::VecDeque<Result<Box<dyn RemoteConnection>, TransportError>>,
    }

    struct ScriptedConnection {
        hello: RelayHello,
        alive: bool,
        frames: Vec<Result<Option<RelayFrame>, TransportError>>,
        cursor: ResumeCursor,
    }

    impl RemoteConnection for ScriptedConnection {
        fn state(&self) -> ConnectionState {
            if self.alive {
                ConnectionState::Streaming
            } else {
                ConnectionState::Disconnected
            }
        }

        fn hello(&self) -> &RelayHello {
            &self.hello
        }

        fn receive(&mut self) -> Result<Option<RelayFrame>, TransportError> {
            self.frames.pop().unwrap_or(Ok(None))
        }

        fn acknowledge(&mut self, cursor: ResumeCursor) -> Result<(), TransportError> {
            self.cursor = cursor;
            Ok(())
        }

        fn inject_hook(
            &mut self,
            _payload: &crate::hook_ingest::RelayHookPayload,
        ) -> Result<(), TransportError> {
            Ok(())
        }

        fn close(&mut self) -> Result<(), TransportError> {
            self.alive = false;
            Ok(())
        }

        fn is_alive(&mut self) -> Result<bool, TransportError> {
            Ok(self.alive)
        }
    }

    impl ScriptedTransport {
        fn connect_mut(
            &mut self,
            _host: &RemoteHostConfig,
            _resume: ResumeCursor,
        ) -> Result<Box<dyn RemoteConnection>, TransportError> {
            self.outcomes
                .pop_front()
                .unwrap_or(Err(TransportError::Disconnected))
        }
    }

    struct ScriptedTransportAdapter {
        inner: std::sync::Mutex<ScriptedTransport>,
    }

    impl RemoteTransport for ScriptedTransportAdapter {
        fn connect(
            &self,
            host: &RemoteHostConfig,
            resume: ResumeCursor,
        ) -> Result<Box<dyn RemoteConnection>, TransportError> {
            self.inner.lock().unwrap().connect_mut(host, resume)
        }
    }

    fn host() -> RemoteHostConfig {
        RemoteHostConfig {
            id: "remote-1".into(),
            destination: "dev@example.internal".into(),
            port: None,
            identity_file: None,
            known_hosts_file: None,
            host_key_policy: crate::SshHostKeyPolicy::Strict,
            connect_timeout_seconds: 10,
        }
    }

    fn connection() -> Box<dyn RemoteConnection> {
        Box::new(ScriptedConnection {
            hello: RelayHello {
                protocol_version: PROTOCOL_VERSION,
                host_id: "remote-1".into(),
                connection_nonce: "a".repeat(64),
                resume: ResumeCursor::default(),
            },
            alive: true,
            frames: vec![Ok(Some(RelayFrame {
                sequence: 1,
                payload: RelayPayload::Heartbeat,
            }))],
            cursor: ResumeCursor::default(),
        })
    }

    #[test]
    fn start_does_not_report_streaming_until_connect_succeeds() {
        let transport = Box::new(ScriptedTransportAdapter {
            inner: std::sync::Mutex::new(ScriptedTransport {
                outcomes: std::collections::VecDeque::from([Err(
                    TransportError::AuthenticationFailed,
                )]),
            }),
        });
        let mut session = RelaySession::new(host(), transport);
        assert_eq!(
            session.start(),
            Err(RelaySessionError::Transport(
                TransportError::AuthenticationFailed
            ))
        );
        let snapshot = session.snapshot();
        assert_eq!(snapshot.state, ConnectionState::Disconnected);
        assert!(!snapshot.process_alive);
        assert_eq!(
            snapshot.last_error,
            Some(TransportError::AuthenticationFailed.to_string())
        );
    }

    #[test]
    fn stop_clears_active_connection_honestly() {
        let transport = Box::new(ScriptedTransportAdapter {
            inner: std::sync::Mutex::new(ScriptedTransport {
                outcomes: std::collections::VecDeque::from([Ok(connection())]),
            }),
        });
        let mut session = RelaySession::new(host(), transport);
        session.start().unwrap();
        session.stop().unwrap();
        let snapshot = session.snapshot();
        assert_eq!(snapshot.state, ConnectionState::Disconnected);
        assert!(!snapshot.process_alive);
        assert!(snapshot.connection_nonce.is_none());
    }

    #[test]
    fn receive_marks_disconnect_when_transport_ends() {
        let transport = Box::new(ScriptedTransportAdapter {
            inner: std::sync::Mutex::new(ScriptedTransport {
                outcomes: std::collections::VecDeque::from([Ok(Box::new(ScriptedConnection {
                    hello: RelayHello {
                        protocol_version: PROTOCOL_VERSION,
                        host_id: "remote-1".into(),
                        connection_nonce: "a".repeat(64),
                        resume: ResumeCursor::default(),
                    },
                    alive: true,
                    frames: vec![Ok(None)],
                    cursor: ResumeCursor::default(),
                })
                    as Box<dyn RemoteConnection>)]),
            }),
        });
        let mut session = RelaySession::new(host(), transport);
        session.start().unwrap();
        assert_eq!(session.receive().unwrap(), None);
        assert_eq!(session.snapshot().state, ConnectionState::Disconnected);
    }
}

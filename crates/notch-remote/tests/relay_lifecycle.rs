use std::time::Duration;

use notch_protocol::SessionEventKind;
use notch_remote::{
    ConnectionState, DirectRelayTransport, RelayHookPayload, RelayPayload, RelaySession,
    RemoteHostConfig, RemoteRelayManager, ResumeCursor, SshHostKeyPolicy,
};

fn host_config() -> RemoteHostConfig {
    RemoteHostConfig {
        id: "integration-host".into(),
        destination: "dev@example.internal".into(),
        port: None,
        identity_file: None,
        known_hosts_file: None,
        host_key_policy: SshHostKeyPolicy::Strict,
        connect_timeout_seconds: 10,
    }
}

#[test]
fn relay_binary_handshake_and_shutdown() {
    let transport = Box::new(DirectRelayTransport::new(env!(
        "CARGO_BIN_EXE_llm-notch-relay"
    )));
    let mut session = RelaySession::new(host_config(), transport);
    session.start().expect("relay handshake");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.state, ConnectionState::Streaming);
    assert!(snapshot.process_alive);
    assert_eq!(snapshot.connection_nonce.as_deref().map(str::len), Some(64));
    session.stop().expect("relay shutdown");
    let snapshot = session.snapshot();
    assert_eq!(snapshot.state, ConnectionState::Disconnected);
    assert!(!snapshot.process_alive);
}

#[test]
fn relay_binary_emits_monotonic_heartbeat() {
    let transport = Box::new(DirectRelayTransport::new(env!(
        "CARGO_BIN_EXE_llm-notch-relay"
    )));
    let mut session = RelaySession::new(host_config(), transport);
    session.start().expect("relay handshake");
    let frame = session
        .receive()
        .expect("receive heartbeat")
        .expect("heartbeat frame");
    assert!(matches!(frame.payload, RelayPayload::Heartbeat));
    assert_eq!(frame.sequence, 1);
    session
        .acknowledge(ResumeCursor {
            last_sequence: frame.sequence,
        })
        .expect("acknowledge");
    session.stop().expect("relay shutdown");
}

#[test]
fn relay_binary_resumes_sequence_cursor() {
    let transport = Box::new(DirectRelayTransport::new(env!(
        "CARGO_BIN_EXE_llm-notch-relay"
    )));
    let mut session = RelaySession::new(host_config(), transport);
    session.start().expect("relay handshake");
    let first = session
        .receive()
        .expect("first heartbeat")
        .expect("first frame");
    let resume = session.snapshot().resume;
    session.stop().expect("relay shutdown");

    let transport = Box::new(DirectRelayTransport::new(env!(
        "CARGO_BIN_EXE_llm-notch-relay"
    )));
    let mut resumed = RelaySession::with_resume(
        host_config(),
        transport,
        notch_remote::ReconnectPolicy::default(),
        resume,
    );
    resumed.start().expect("resume handshake");
    let second = resumed
        .receive()
        .expect("resumed heartbeat")
        .expect("resumed frame");
    assert!(second.sequence > first.sequence);
    resumed.stop().expect("relay shutdown");
}

#[test]
fn manager_tracks_multiple_sessions_honestly() {
    let mut manager = RemoteRelayManager::new();
    let first = RelaySession::new(
        RemoteHostConfig {
            id: "host-a".into(),
            ..host_config()
        },
        Box::new(DirectRelayTransport::new(env!(
            "CARGO_BIN_EXE_llm-notch-relay"
        ))),
    );
    let second = RelaySession::new(
        RemoteHostConfig {
            id: "host-b".into(),
            ..host_config()
        },
        Box::new(DirectRelayTransport::new(env!(
            "CARGO_BIN_EXE_llm-notch-relay"
        ))),
    );
    manager.register(first).expect("register first");
    manager.register(second).expect("register second");
    manager
        .get_mut("host-a")
        .expect("host-a")
        .start()
        .expect("start host-a");

    let snapshots = manager.snapshots();
    assert_eq!(snapshots.len(), 2);
    let host_a = snapshots
        .iter()
        .find(|snapshot| snapshot.host_id == "host-a")
        .expect("host-a snapshot");
    let host_b = snapshots
        .iter()
        .find(|snapshot| snapshot.host_id == "host-b")
        .expect("host-b snapshot");
    assert_eq!(host_a.state, ConnectionState::Streaming);
    assert!(host_a.process_alive);
    assert_eq!(host_b.state, ConnectionState::Disconnected);
    assert!(!host_b.process_alive);

    manager.remove("host-a").expect("remove host-a");
    assert!(manager.get("host-a").is_none());
}

#[test]
fn relay_binary_rejects_invalid_arguments() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_llm-notch-relay"))
        .arg("--host-id")
        .output()
        .expect("spawn relay");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing value"));
}

#[test]
#[ignore = "waits for relay heartbeat interval"]
fn relay_binary_heartbeat_interval_is_bounded() {
    let transport = Box::new(DirectRelayTransport::new(env!(
        "CARGO_BIN_EXE_llm-notch-relay"
    )));
    let mut session = RelaySession::new(host_config(), transport);
    session.start().expect("relay handshake");
    let started = std::time::Instant::now();
    let _ = session
        .receive()
        .expect("first heartbeat")
        .expect("first frame");
    let _ = session
        .receive()
        .expect("second heartbeat")
        .expect("second frame");
    assert!(started.elapsed() >= Duration::from_secs(4));
    session.stop().expect("relay shutdown");
}

#[test]
fn relay_binary_forwards_injected_hook_event() {
    let transport = Box::new(DirectRelayTransport::new(env!(
        "CARGO_BIN_EXE_llm-notch-relay"
    )));
    let mut session = RelaySession::new(host_config(), transport);
    session.start().expect("relay handshake");

    session
        .inject_hook(RelayHookPayload {
            source: "codex".into(),
            event: "tool".into(),
            session_id: None,
            external_session_id: Some("remote-session-42".into()),
            summary: Some("Executed cargo test".into()),
            occurred_at_ms: Some(1_700_000_000_123),
            tool_name: Some("run_command".into()),
            attention: None,
        })
        .expect("inject hook");

    let frame = session
        .receive()
        .expect("receive session event")
        .expect("session event frame");
    assert_eq!(frame.sequence, 1);
    assert_eq!(
        frame.payload,
        RelayPayload::SessionEvent {
            session_id: "remote-session-42".into(),
            source: "codex".into(),
            summary: "Executed cargo test".into(),
            occurred_at_ms: 1_700_000_000_123,
            kind: Some(SessionEventKind::Tool),
            tool_name: Some("run_command".into()),
            attention: None,
        }
    );
    session.stop().expect("relay shutdown");
}

#[test]
fn relay_binary_forwards_spooled_hook_event() {
    let runtime_dir = tempfile::tempdir().expect("runtime dir");
    let spool_dir = runtime_dir.path().join("spool");
    std::fs::create_dir_all(&spool_dir).expect("spool dir");
    write_spool_frame(
        &spool_dir.join("00000000000000000001.frame"),
        RelayHookPayload {
            source: "cursor".into(),
            event: "lifecycle".into(),
            session_id: None,
            external_session_id: Some("cursor-remote-9".into()),
            summary: Some("Agent finished planning".into()),
            occurred_at_ms: Some(1_700_000_001_000),
            tool_name: None,
            attention: None,
        },
    );

    let transport = Box::new(DirectRelayTransport::with_event_spool_dir(
        env!("CARGO_BIN_EXE_llm-notch-relay"),
        runtime_dir.path().to_string_lossy(),
    ));
    let mut session = RelaySession::new(host_config(), transport);
    session.start().expect("relay handshake");

    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut frame = None;
    while std::time::Instant::now() < deadline {
        if let Ok(Some(received)) = session.receive() {
            if matches!(received.payload, RelayPayload::SessionEvent { .. }) {
                frame = Some(received);
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let frame = frame.expect("session event from spool");
    assert_eq!(
        frame.payload,
        RelayPayload::SessionEvent {
            session_id: "cursor-remote-9".into(),
            source: "cursor".into(),
            summary: "Agent finished planning".into(),
            occurred_at_ms: 1_700_000_001_000,
            kind: Some(SessionEventKind::Lifecycle),
            tool_name: None,
            attention: None,
        }
    );
    session.stop().expect("relay shutdown");
}

fn write_spool_frame(path: &std::path::Path, payload: RelayHookPayload) {
    let body = serde_json::json!({
        "type": "ingest",
        "v": 1,
        "requestId": "relay-test",
        "payload": payload,
    });
    let body_bytes = serde_json::to_vec(&body).expect("encode spool body");
    let mut frame = (body_bytes.len() as u32).to_be_bytes().to_vec();
    frame.extend_from_slice(&body_bytes);
    std::fs::write(path, frame).expect("write spool frame");
}

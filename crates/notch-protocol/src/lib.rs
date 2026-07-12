//! # notch-protocol
//!
//! **Owner:** Stage 0 foundation agent — frozen shared wire contracts.
//!
//! Parallel agents MUST NOT change field semantics, enum variants, or protocol
//! version without explicit coordination. Additive changes require a protocol
//! version bump.

pub mod connector;
pub mod constants;
pub mod decision;
pub mod health;
pub mod migration;
pub mod purge;
pub mod types;

pub use connector::*;
pub use constants::*;
pub use decision::*;
pub use health::*;
pub use migration::*;
pub use purge::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use serde_json::{Value, json};
    use ts_rs::{Config, TS};
    use uuid::Uuid;

    fn metric_sample() -> MetricSample {
        MetricSample {
            at_ms: 1_700_000_000_100,
            cpu_core_percent: 125.5,
            cpu_host_percent: 10.4,
            rss_bytes: 512 * 1024 * 1024,
            runtime_ms: 90_000,
            process_count: 3,
            read_bytes_per_sec: 4_096,
            write_bytes_per_sec: 8_192,
            quality: MetricQuality {
                attribution: AttributionQuality::Shared,
                cpu: MetricAvailability::WarmingUp,
                io: IoQuality::AllIo,
                reason: Some("process tree is shared by multiple sessions".into()),
            },
        }
    }

    fn session() -> AgentSession {
        AgentSession {
            id: "sess-1".into(),
            source: AgentSource::Cursor,
            external_session_id: "cursor-session-42".into(),
            label: "Implement native foundation".into(),
            workspace_label: Some("llm_notch".into()),
            status: SessionStatus::Running,
            attention: AttentionKind::Permission,
            started_at_ms: 1_700_000_000_000,
            last_event_at_ms: 1_700_000_000_100,
            ended_at_ms: None,
            process_root: Some(ProcessIdentity {
                pid: 42,
                started_at_ms: 1_700_000_000_000,
            }),
            verified_terminal: None,
            latest_metric: Some(metric_sample()),
        }
    }

    #[test]
    fn protocol_version_remains_unshipped_v1() {
        assert_eq!(PROTOCOL_VERSION, 1);
    }

    #[test]
    fn agent_session_round_trips_camel_case() {
        let session = session();
        let value: Value = serde_json::to_value(&session).expect("serialize");
        assert_eq!(value["externalSessionId"], "cursor-session-42");
        assert_eq!(value["attention"], "permission");
        assert_eq!(value["latestMetric"]["cpuCorePercent"], 125.5);
        assert!(value.get("endedAtMs").is_none());
        assert!(value.get("progress").is_none());
        assert!(value.get("tokenCount").is_none());
        assert!(value.get("costCents").is_none());

        let decoded: AgentSession = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded, session);
    }

    #[test]
    fn metric_frame_round_trips_quality_and_agent_map() {
        let sample = metric_sample();
        let aggregate = AgentAggregate {
            at_ms: sample.at_ms,
            cpu_core_percent: sample.cpu_core_percent,
            cpu_host_percent: sample.cpu_host_percent,
            rss_bytes: sample.rss_bytes,
            runtime_ms: sample.runtime_ms,
            process_count: sample.process_count,
            read_bytes_per_sec: sample.read_bytes_per_sec,
            write_bytes_per_sec: sample.write_bytes_per_sec,
            quality: sample.quality.clone(),
            active_sessions: 1,
            attention_sessions: 1,
        };
        let mut agents = BTreeMap::new();
        agents.insert("sess-1".into(), sample);
        let frame = MetricsFrame {
            host: HostMetricSample {
                at_ms: 1_700_000_000_100,
                cpu_host_percent: 37.5,
                used_memory_bytes: 8_000_000_000,
                total_memory_bytes: 16_000_000_000,
                visible_process_count: 320,
                disk_read_bytes_per_sec: 1_024,
                disk_write_bytes_per_sec: 2_048,
            },
            aggregate,
            agents,
        };

        let value = serde_json::to_value(&frame).expect("serialize");
        assert_eq!(value["agents"]["sess-1"]["quality"]["cpu"], "warmingUp");
        assert_eq!(value["agents"]["sess-1"]["quality"]["io"], "allIo");
        assert_eq!(value["aggregate"]["attentionSessions"], 1);

        let decoded: MetricsFrame = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn stream_payload_uses_named_fields_and_round_trips() {
        let frame = StreamFrame {
            sequence: 7,
            emitted_at_ms: 1_700_000_000_200,
            payload: StreamPayload::SessionEvent {
                event: SessionEvent {
                    id: Uuid::nil(),
                    session_id: "sess-1".into(),
                    sequence: 3,
                    occurred_at_ms: 1_700_000_000_150,
                    kind: SessionEventKind::Tool,
                    level: EventLevel::Warning,
                    summary: "Tool requested elevated permission".into(),
                    tool_name: Some("shell".into()),
                },
            },
        };

        let value = serde_json::to_value(&frame).expect("serialize");
        assert_eq!(value["payload"]["type"], "sessionEvent");
        assert_eq!(value["payload"]["event"]["kind"], "tool");
        assert!(value["payload"].get("data").is_none());

        let decoded: StreamFrame = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded, frame);

        let remove = serde_json::to_value(StreamPayload::SessionRemove {
            session_id: "sess-1".into(),
        })
        .expect("serialize removal");
        assert_eq!(remove["sessionId"], "sess-1");
        assert!(remove.get("session_id").is_none());
    }

    #[test]
    fn strict_contracts_reject_prototype_fields() {
        let mut value = serde_json::to_value(session()).expect("serialize");
        value
            .as_object_mut()
            .expect("session object")
            .insert("progress".into(), json!(0.5));

        assert!(serde_json::from_value::<AgentSession>(value).is_err());
    }

    #[test]
    fn adapter_capabilities_v2_fields_default_and_round_trip() {
        let caps = AdapterCapabilities::template(AgentSource::Cursor);
        let value = serde_json::to_value(&caps).expect("serialize");
        assert_eq!(value["failOpenHooks"], true);
        assert_eq!(value["contextOpenTier"], "none");

        let decoded: AdapterCapabilities = serde_json::from_value(value).expect("deserialize");
        assert_eq!(decoded.source, AgentSource::Cursor);
        assert!(decoded.fail_open_hooks);
    }

    #[test]
    fn agent_source_round_trips_and_accepts_legacy_aliases() {
        let canonical = serde_json::json!("qwen");
        let decoded: AgentSource = serde_json::from_value(canonical).expect("qwen");
        assert_eq!(decoded, AgentSource::Qwen);

        for alias in ["antigravity-cli", "copilot", "qwen-cli"] {
            let decoded: AgentSource =
                serde_json::from_value(serde_json::json!(alias)).expect(alias);
            assert_ne!(decoded, AgentSource::Generic);
        }
    }

    #[test]
    fn ts_rs_declarations_match_camel_case_wire_names() {
        let config = Config::default();
        let session_decl = AgentSession::decl(&config);
        assert!(session_decl.contains("externalSessionId"));
        assert!(session_decl.contains("latestMetric"));
        assert!(!session_decl.contains("tokenCount"));
        assert!(!session_decl.contains("costCents"));

        let metrics_decl = MetricsFrame::decl(&config);
        assert!(metrics_decl.contains("agents"));
        assert!(metrics_decl.contains("MetricSample"));

        let stream_decl = StreamPayload::decl(&config);
        assert!(stream_decl.contains("sessionEvent"));
        assert!(stream_decl.contains("resyncRequired"));
    }
}

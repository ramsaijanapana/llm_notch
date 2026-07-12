use std::collections::HashMap;

use notch_protocol::{MetricsFrame, StreamFrame, StreamPayload};

use crate::constants::STREAM_REPLAY_CAPACITY;

/// Coalesces high-frequency stream payloads before emission.
#[derive(Debug, Default)]
pub struct StreamCoalescer {
    pending_session_upserts: HashMap<String, StreamPayload>,
    pending_metrics: Option<MetricsFrame>,
    pending_other: Vec<StreamPayload>,
}

impl StreamCoalescer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, payload: StreamPayload) {
        match payload {
            StreamPayload::SessionUpsert { session } => {
                self.pending_session_upserts
                    .insert(session.id.clone(), StreamPayload::SessionUpsert { session });
            }
            StreamPayload::Metrics { metrics } => {
                self.pending_metrics = Some(metrics);
            }
            other => self.pending_other.push(other),
        }
    }

    pub fn drain(&mut self) -> Vec<StreamPayload> {
        let mut out = Vec::new();
        out.append(&mut self.pending_other);
        out.extend(self.pending_session_upserts.drain().map(|(_, p)| p));
        if let Some(metrics) = self.pending_metrics.take() {
            out.push(StreamPayload::Metrics { metrics });
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.pending_other.is_empty()
            && self.pending_session_upserts.is_empty()
            && self.pending_metrics.is_none()
    }
}

/// Bounded replay buffer for stream frames.
#[derive(Debug, Default)]
pub struct StreamReplayBuffer {
    frames: Vec<StreamFrame>,
}

impl StreamReplayBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, frame: StreamFrame) {
        self.frames.push(frame);
        if self.frames.len() > STREAM_REPLAY_CAPACITY {
            let overflow = self.frames.len() - STREAM_REPLAY_CAPACITY;
            self.frames.drain(0..overflow);
        }
    }

    pub fn since(&self, sequence: u64) -> Vec<StreamFrame> {
        self.frames
            .iter()
            .filter(|frame| frame.sequence > sequence)
            .cloned()
            .collect()
    }

    pub fn latest_sequence(&self) -> u64 {
        self.frames.last().map(|f| f.sequence).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::{
        AgentAggregate, AgentSession, AgentSource, AttentionKind, HostMetricSample, MetricSample,
        SessionStatus,
    };
    use std::collections::BTreeMap;

    #[test]
    fn coalescer_keeps_latest_session_and_metrics() {
        let mut c = StreamCoalescer::new();
        let session_a = AgentSession {
            id: "s1".into(),
            source: AgentSource::Cursor,
            external_session_id: "e1".into(),
            label: "a".into(),
            workspace_label: None,
            status: SessionStatus::Running,
            attention: AttentionKind::None,
            started_at_ms: 0,
            last_event_at_ms: 0,
            ended_at_ms: None,
            process_root: None,
            verified_terminal: None,
            latest_metric: None,
        };
        let session_b = session_a.clone();
        c.push(StreamPayload::SessionUpsert { session: session_a });
        c.push(StreamPayload::SessionUpsert {
            session: AgentSession {
                label: "b".into(),
                ..session_b
            },
        });

        let host = HostMetricSample {
            at_ms: 0,
            cpu_host_percent: 1.0,
            used_memory_bytes: 1,
            total_memory_bytes: 2,
            visible_process_count: 1,
            disk_read_bytes_per_sec: 0,
            disk_write_bytes_per_sec: 0,
        };
        let sample = MetricSample {
            at_ms: 0,
            cpu_core_percent: 0.0,
            cpu_host_percent: 0.0,
            rss_bytes: 0,
            runtime_ms: 0,
            process_count: 0,
            read_bytes_per_sec: 0,
            write_bytes_per_sec: 0,
            quality: notch_protocol::MetricQuality {
                attribution: notch_protocol::AttributionQuality::Exact,
                cpu: notch_protocol::MetricAvailability::Available,
                io: notch_protocol::IoQuality::Unavailable,
                reason: None,
            },
        };
        let frame = MetricsFrame {
            host: host.clone(),
            aggregate: AgentAggregate {
                at_ms: 0,
                cpu_core_percent: 0.0,
                cpu_host_percent: 0.0,
                rss_bytes: 0,
                runtime_ms: 0,
                process_count: 0,
                read_bytes_per_sec: 0,
                write_bytes_per_sec: 0,
                quality: sample.quality.clone(),
                active_sessions: 1,
                attention_sessions: 0,
            },
            agents: BTreeMap::new(),
        };
        c.push(StreamPayload::Metrics {
            metrics: frame.clone(),
        });
        c.push(StreamPayload::Metrics {
            metrics: MetricsFrame {
                host: HostMetricSample {
                    cpu_host_percent: 99.0,
                    ..host
                },
                ..frame
            },
        });

        let drained = c.drain();
        assert_eq!(drained.len(), 2);
        match &drained[0] {
            StreamPayload::SessionUpsert { session } => assert_eq!(session.label, "b"),
            _ => panic!("expected session upsert"),
        }
        match &drained[1] {
            StreamPayload::Metrics { metrics } => {
                assert_eq!(metrics.host.cpu_host_percent, 99.0)
            }
            _ => panic!("expected metrics"),
        }
    }
}

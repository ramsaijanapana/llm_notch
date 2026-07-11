use std::collections::BTreeMap;
use std::time::Instant;

use notch_protocol::{AttributionQuality, MetricsFrame, ProcessIdentity};

use crate::aggregate::{compute_aggregate, compute_session_metrics, tree_to_sample};
use crate::constants::{ACTIVE_REFRESH_INTERVAL_MS, IDLE_REFRESH_INTERVAL_MS, MAX_ACTIVE_ROOTS};
use crate::history::SessionHistory;
use crate::model::RegisteredSession;
use crate::sysinfo_adapter::SysinfoProbe;
use crate::{MetricsError, MetricsResult};

/// Runtime statistics for sampler overhead and retention.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SamplerStats {
    pub refresh_count: u64,
    pub last_refresh_ms: i64,
    pub last_refresh_duration_us: u64,
    pub active_roots: u32,
    pub registered_sessions: u32,
    pub history_samples_total: u64,
    pub warming_up: bool,
    pub refresh_interval_ms: u64,
}

/// Collects host and per-session metrics on a fixed cadence.
#[derive(Debug)]
pub struct MetricsSampler {
    probe: SysinfoProbe,
    sessions: Vec<RegisteredSession>,
    histories: BTreeMap<String, SessionHistory>,
    latest: Option<MetricsFrame>,
    last_refresh_ms: i64,
    refresh_count: u64,
    last_refresh_duration_us: u64,
    active_sessions: u32,
    attention_sessions: u32,
}

impl Default for MetricsSampler {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsSampler {
    pub fn new() -> Self {
        Self {
            probe: SysinfoProbe::new(),
            sessions: Vec::new(),
            histories: BTreeMap::new(),
            latest: None,
            last_refresh_ms: 0,
            refresh_count: 0,
            last_refresh_duration_us: 0,
            active_sessions: 0,
            attention_sessions: 0,
        }
    }

    pub fn register_session(
        &mut self,
        session_id: String,
        root: ProcessIdentity,
        attribution: AttributionQuality,
        registered_at_ms: i64,
    ) -> MetricsResult<()> {
        if self
            .sessions
            .iter()
            .any(|session| session.session_id == session_id)
        {
            self.unregister_session(&session_id);
        }

        if self.sessions.len() >= MAX_ACTIVE_ROOTS {
            return Err(MetricsError::TooManyRoots {
                max: MAX_ACTIVE_ROOTS,
            });
        }

        self.sessions.push(RegisteredSession {
            session_id: session_id.clone(),
            root,
            attribution,
            registered_at_ms,
        });
        self.histories.entry(session_id).or_default();
        Ok(())
    }

    pub fn unregister_session(&mut self, session_id: &str) {
        self.sessions
            .retain(|session| session.session_id != session_id);
        self.histories.remove(session_id);
        if let Some(frame) = self.latest.as_mut() {
            frame.agents.remove(session_id);
        }
    }

    pub fn registered_session_ids(&self) -> Vec<String> {
        self.sessions
            .iter()
            .map(|session| session.session_id.clone())
            .collect()
    }

    pub fn set_session_counts(&mut self, active_sessions: u32, attention_sessions: u32) {
        self.active_sessions = active_sessions;
        self.attention_sessions = attention_sessions;
    }

    pub fn refresh_interval_ms(&self) -> u64 {
        if self.sessions.is_empty() {
            IDLE_REFRESH_INTERVAL_MS
        } else {
            ACTIVE_REFRESH_INTERVAL_MS
        }
    }

    pub fn should_refresh(&self, at_ms: i64) -> bool {
        if self.last_refresh_ms == 0 {
            return true;
        }
        let elapsed = (at_ms - self.last_refresh_ms).max(0) as u64;
        elapsed >= self.refresh_interval_ms()
    }

    pub fn tick(&mut self, at_ms: i64) -> Option<MetricsFrame> {
        if self.should_refresh(at_ms) {
            Some(self.refresh(at_ms))
        } else {
            None
        }
    }

    pub fn refresh(&mut self, at_ms: i64) -> MetricsFrame {
        let started = Instant::now();
        let snapshot = self.probe.refresh(at_ms);
        let (tree_metrics, _) = compute_session_metrics(
            &self.sessions,
            &snapshot.processes,
            snapshot.logical_cores,
            snapshot.elapsed_ms,
            at_ms,
            snapshot.cpu_ready,
            snapshot.io_ready,
            snapshot.io_quality,
        );

        let mut agents = BTreeMap::new();
        for (session_id, tree) in &tree_metrics {
            let sample = tree_to_sample(at_ms, tree, snapshot.logical_cores);
            self.histories
                .entry(session_id.clone())
                .or_default()
                .push(sample.clone());
            agents.insert(session_id.clone(), sample);
        }

        let frame = MetricsFrame {
            host: snapshot.host_sample(),
            aggregate: compute_aggregate(
                &tree_metrics,
                at_ms,
                snapshot.logical_cores,
                self.active_sessions,
                self.attention_sessions,
                snapshot.io_quality,
            ),
            agents,
        };

        self.latest = Some(frame.clone());
        self.last_refresh_ms = at_ms;
        self.refresh_count = self.refresh_count.saturating_add(1);
        self.last_refresh_duration_us = started.elapsed().as_micros() as u64;
        frame
    }

    pub fn latest_frame(&self) -> Option<&MetricsFrame> {
        self.latest.as_ref()
    }

    pub fn session_history(&self, session_id: &str) -> Vec<notch_protocol::MetricSample> {
        self.histories
            .get(session_id)
            .map(|history| history.samples().cloned().collect())
            .unwrap_or_default()
    }

    pub fn session_latest(&self, session_id: &str) -> Option<notch_protocol::MetricSample> {
        self.histories
            .get(session_id)
            .and_then(|history| history.latest().cloned())
    }

    pub fn clear_history(&mut self) {
        for history in self.histories.values_mut() {
            *history = SessionHistory::default();
        }
        self.latest = None;
    }

    pub fn stats(&self) -> SamplerStats {
        let history_samples_total = self
            .histories
            .values()
            .map(|history| history.len() as u64)
            .sum();
        SamplerStats {
            refresh_count: self.refresh_count,
            last_refresh_ms: self.last_refresh_ms,
            last_refresh_duration_us: self.last_refresh_duration_us,
            active_roots: self.sessions.len() as u32,
            registered_sessions: self.sessions.len() as u32,
            history_samples_total,
            warming_up: self.probe.refresh_count() < 2,
            refresh_interval_ms: self.refresh_interval_ms(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::MAX_HISTORY_SAMPLES_PER_SESSION;

    #[test]
    fn registration_enforces_root_cap() {
        let mut sampler = MetricsSampler::new();
        for i in 0..MAX_ACTIVE_ROOTS {
            sampler
                .register_session(
                    format!("s{i}"),
                    ProcessIdentity {
                        pid: i as u32 + 1,
                        started_at_ms: 1,
                    },
                    AttributionQuality::Exact,
                    0,
                )
                .expect("register");
        }
        let err = sampler
            .register_session(
                "overflow".into(),
                ProcessIdentity {
                    pid: 999,
                    started_at_ms: 1,
                },
                AttributionQuality::Exact,
                0,
            )
            .expect_err("too many");
        assert!(matches!(err, MetricsError::TooManyRoots { .. }));
    }

    #[test]
    fn idle_refresh_interval_when_no_sessions() {
        let sampler = MetricsSampler::new();
        assert_eq!(sampler.refresh_interval_ms(), IDLE_REFRESH_INTERVAL_MS);
    }

    #[test]
    fn tick_respects_active_refresh_cadence() {
        let mut sampler = MetricsSampler::new();
        sampler
            .register_session(
                "cadence".into(),
                ProcessIdentity {
                    pid: 1,
                    started_at_ms: 1,
                },
                AttributionQuality::Exact,
                0,
            )
            .unwrap();
        sampler.last_refresh_ms = 1_000;
        assert!(!sampler.should_refresh(1_000 + ACTIVE_REFRESH_INTERVAL_MS as i64 - 1));
        assert!(sampler.should_refresh(1_000 + ACTIVE_REFRESH_INTERVAL_MS as i64));
    }

    #[test]
    fn history_retains_bounded_samples() {
        let mut sampler = MetricsSampler::new();
        let history = sampler.histories.entry("hist".into()).or_default();
        for i in 0..=MAX_HISTORY_SAMPLES_PER_SESSION {
            history.push(notch_protocol::MetricSample {
                at_ms: i as i64 * 1_000,
                cpu_core_percent: 0.0,
                cpu_host_percent: 0.0,
                rss_bytes: 0,
                runtime_ms: 0,
                process_count: 0,
                read_bytes_per_sec: 0,
                write_bytes_per_sec: 0,
                quality: notch_protocol::MetricQuality {
                    attribution: AttributionQuality::Heuristic,
                    cpu: notch_protocol::MetricAvailability::Available,
                    io: notch_protocol::IoQuality::Unavailable,
                    reason: None,
                },
            });
        }
        let history = sampler.session_history("hist");
        assert_eq!(history.len(), MAX_HISTORY_SAMPLES_PER_SESSION);
    }
}

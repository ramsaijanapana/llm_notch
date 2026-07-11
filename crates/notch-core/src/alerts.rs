use notch_protocol::{
    AttentionKind, HostMetricSample, MetricSample, ResourceAlert, ResourceAlertKind,
};

use crate::constants::{
    CPU_CRITICAL_DURATION, CPU_CRITICAL_THRESHOLD, CPU_WARN_DURATION, CPU_WARN_THRESHOLD,
    RSS_ALERT_BYTES, RSS_ALERT_DURATION, RSS_ALERT_HOST_FRACTION,
};

/// V1 alert kinds evaluated by the core (never focus windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlertKind {
    NewAttention,
    CpuWarn,
    CpuCritical,
    MemoryHigh,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveAlert {
    pub kind: AlertKind,
    pub session_id: Option<String>,
    pub message: String,
    pub raised_at_ms: i64,
}

#[derive(Debug, Clone, Default)]
struct ThresholdTracker {
    above_since_ms: Option<i64>,
}

impl ThresholdTracker {
    fn observe(&mut self, now_ms: i64, above: bool, required_ms: i64) -> bool {
        if above {
            let since = *self.above_since_ms.get_or_insert(now_ms);
            now_ms.saturating_sub(since) >= required_ms
        } else {
            self.above_since_ms = None;
            false
        }
    }
}

/// Sustained-threshold alert evaluator for attention and host metrics.
#[derive(Debug, Default)]
pub struct AlertEvaluator {
    cpu_warn: ThresholdTracker,
    cpu_critical: ThresholdTracker,
    rss: ThresholdTracker,
    known_attention: std::collections::HashSet<String>,
    active: Vec<ActiveAlert>,
    disk_alerts_enabled: bool,
}

impl AlertEvaluator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_disk_alerts(mut self, enabled: bool) -> Self {
        self.disk_alerts_enabled = enabled;
        self
    }

    pub fn active_alerts(&self) -> &[ActiveAlert] {
        &self.active
    }

    pub fn clear_local_acknowledgement(&mut self, session_id: &str) {
        self.known_attention.remove(session_id);
        self.active
            .retain(|alert| alert.session_id.as_deref() != Some(session_id));
    }

    pub fn evaluate_attention(
        &mut self,
        session_id: &str,
        attention: AttentionKind,
        now_ms: i64,
    ) -> Option<ActiveAlert> {
        if attention == AttentionKind::None {
            self.known_attention.remove(session_id);
            return None;
        }

        if self.known_attention.insert(session_id.to_string()) {
            let alert = ActiveAlert {
                kind: AlertKind::NewAttention,
                session_id: Some(session_id.to_string()),
                message: format!("Session requires attention: {attention:?}"),
                raised_at_ms: now_ms,
            };
            self.active.push(alert.clone());
            Some(alert)
        } else {
            None
        }
    }

    pub fn evaluate_host_metrics(
        &mut self,
        host: &HostMetricSample,
        aggregate: &MetricSample,
        now_ms: i64,
    ) -> Vec<ActiveAlert> {
        let mut raised = Vec::new();

        if self.cpu_critical.observe(
            now_ms,
            host.cpu_host_percent >= CPU_CRITICAL_THRESHOLD,
            CPU_CRITICAL_DURATION.as_millis() as i64,
        ) {
            let alert = ActiveAlert {
                kind: AlertKind::CpuCritical,
                session_id: None,
                message: format!("Host CPU sustained above {:.0}%", CPU_CRITICAL_THRESHOLD),
                raised_at_ms: now_ms,
            };
            self.upsert_global_alert(alert.clone());
            raised.push(alert);
        } else if self.cpu_warn.observe(
            now_ms,
            host.cpu_host_percent >= CPU_WARN_THRESHOLD,
            CPU_WARN_DURATION.as_millis() as i64,
        ) {
            let alert = ActiveAlert {
                kind: AlertKind::CpuWarn,
                session_id: None,
                message: format!("Host CPU sustained above {:.0}%", CPU_WARN_THRESHOLD),
                raised_at_ms: now_ms,
            };
            self.upsert_global_alert(alert.clone());
            raised.push(alert);
        }

        let rss_threshold =
            RSS_ALERT_BYTES.min((host.total_memory_bytes as f64 * RSS_ALERT_HOST_FRACTION) as u64);
        if self.rss.observe(
            now_ms,
            aggregate.rss_bytes >= rss_threshold,
            RSS_ALERT_DURATION.as_millis() as i64,
        ) {
            let alert = ActiveAlert {
                kind: AlertKind::MemoryHigh,
                session_id: None,
                message: format!("Aggregate RSS sustained above {} bytes", rss_threshold),
                raised_at_ms: now_ms,
            };
            self.upsert_global_alert(alert.clone());
            raised.push(alert);
        }

        if self.disk_alerts_enabled {
            // Disk alerts are intentionally disabled unless explicitly configured.
        }

        raised
    }

    fn upsert_global_alert(&mut self, alert: ActiveAlert) {
        if let Some(existing) = self
            .active
            .iter_mut()
            .find(|a| a.kind == alert.kind && a.session_id == alert.session_id)
        {
            *existing = alert;
        } else {
            self.active.push(alert);
        }
    }
}

/// Maps core alerts to wire resource alerts (host metrics only; attention stays separate).
pub fn resource_alerts_from_active(alerts: &[ActiveAlert]) -> Vec<ResourceAlert> {
    alerts
        .iter()
        .filter_map(|alert| match alert.kind {
            AlertKind::CpuWarn => Some(ResourceAlert {
                kind: ResourceAlertKind::CpuWarn,
                message: alert.message.clone(),
                session_id: alert.session_id.clone(),
                raised_at_ms: alert.raised_at_ms,
            }),
            AlertKind::CpuCritical => Some(ResourceAlert {
                kind: ResourceAlertKind::CpuCritical,
                message: alert.message.clone(),
                session_id: alert.session_id.clone(),
                raised_at_ms: alert.raised_at_ms,
            }),
            AlertKind::MemoryHigh => Some(ResourceAlert {
                kind: ResourceAlertKind::MemoryHigh,
                message: alert.message.clone(),
                session_id: alert.session_id.clone(),
                raised_at_ms: alert.raised_at_ms,
            }),
            AlertKind::NewAttention => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::{AttributionQuality, IoQuality, MetricAvailability, MetricQuality};

    fn host(cpu: f64, total_mem: u64, used_mem: u64) -> HostMetricSample {
        HostMetricSample {
            at_ms: 0,
            cpu_host_percent: cpu,
            used_memory_bytes: used_mem,
            total_memory_bytes: total_mem,
            visible_process_count: 1,
            disk_read_bytes_per_sec: 0,
            disk_write_bytes_per_sec: 0,
        }
    }

    fn aggregate(rss: u64) -> MetricSample {
        MetricSample {
            at_ms: 0,
            cpu_core_percent: 0.0,
            cpu_host_percent: 0.0,
            rss_bytes: rss,
            runtime_ms: 0,
            process_count: 0,
            read_bytes_per_sec: 0,
            write_bytes_per_sec: 0,
            quality: MetricQuality {
                attribution: AttributionQuality::Exact,
                cpu: MetricAvailability::Available,
                io: IoQuality::Unavailable,
                reason: None,
            },
        }
    }

    #[test]
    fn cpu_critical_requires_thirty_seconds() {
        let mut eval = AlertEvaluator::new();
        let agg = aggregate(0);
        let total = 16 * 1024 * 1024 * 1024;

        for t in (0..29_000).step_by(1_000) {
            let alerts = eval.evaluate_host_metrics(&host(95.0, total, 0), &agg, t);
            assert!(alerts.is_empty());
        }

        let alerts = eval.evaluate_host_metrics(&host(95.0, total, 0), &agg, 30_000);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].kind, AlertKind::CpuCritical);
    }

    #[test]
    fn cpu_warn_requires_sixty_seconds() {
        let mut eval = AlertEvaluator::new();
        let agg = aggregate(0);
        let total = 16 * 1024 * 1024 * 1024;

        for t in (0..59_000).step_by(1_000) {
            let alerts = eval.evaluate_host_metrics(&host(75.0, total, 0), &agg, t);
            assert!(alerts.is_empty());
        }

        let alerts = eval.evaluate_host_metrics(&host(75.0, total, 0), &agg, 60_000);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].kind, AlertKind::CpuWarn);
    }

    #[test]
    fn rss_uses_lower_of_absolute_and_fraction() {
        let mut eval = AlertEvaluator::new();
        // 16 GiB host => 25% = 4 GiB, same as absolute floor.
        let total = 16 * 1024 * 1024 * 1024;
        let threshold = 4 * 1024 * 1024 * 1024;

        for t in (0..59_000).step_by(1_000) {
            let alerts = eval.evaluate_host_metrics(&host(0.0, total, 0), &aggregate(threshold), t);
            assert!(alerts.is_empty());
        }

        let alerts =
            eval.evaluate_host_metrics(&host(0.0, total, 0), &aggregate(threshold), 60_000);
        assert_eq!(alerts[0].kind, AlertKind::MemoryHigh);
    }

    #[test]
    fn new_attention_fires_once_per_session() {
        let mut eval = AlertEvaluator::new();
        let first = eval.evaluate_attention("s1", AttentionKind::Permission, 1);
        assert!(first.is_some());
        let second = eval.evaluate_attention("s1", AttentionKind::Error, 2);
        assert!(second.is_none());
    }

    #[test]
    fn resource_alerts_exclude_attention() {
        let mut eval = AlertEvaluator::new();
        eval.evaluate_attention("s1", AttentionKind::Permission, 1);
        let agg = aggregate(0);
        let total = 16 * 1024 * 1024 * 1024;
        for t in (0..29_000).step_by(1_000) {
            eval.evaluate_host_metrics(&host(95.0, total, 0), &agg, t);
        }
        eval.evaluate_host_metrics(&host(95.0, total, 0), &agg, 30_000);
        let mapped = resource_alerts_from_active(eval.active_alerts());
        assert_eq!(mapped.len(), 1);
        assert_eq!(mapped[0].kind, ResourceAlertKind::CpuCritical);
    }
}

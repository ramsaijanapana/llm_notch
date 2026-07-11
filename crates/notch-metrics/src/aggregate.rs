use std::collections::{BTreeMap, BTreeSet};

use notch_protocol::{
    AgentAggregate, AttributionQuality, IoQuality, MAX_METRIC_REASON_LEN, MetricAvailability,
    MetricQuality, MetricSample,
};

use crate::graph::{
    assign_process_owners, children_index, tree_has_overlap, tree_has_parent_gaps, verify_root,
};
use crate::model::{CounterReadiness, ProcessNode, RegisteredSession, RootStatus};

/// Per-session tree metrics before protocol conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeMetrics {
    pub cpu_core_percent: f64,
    pub rss_bytes: u64,
    pub runtime_ms: u64,
    pub process_count: u32,
    pub read_bytes_per_sec: u64,
    pub write_bytes_per_sec: u64,
    pub quality: MetricQuality,
}

/// Converts a byte delta over `elapsed_ms` into bytes/sec.
pub fn bytes_per_sec(delta: u64, elapsed_ms: u64) -> u64 {
    if elapsed_ms == 0 {
        return 0;
    }
    ((delta as u128 * 1_000) / elapsed_ms as u128) as u64
}

/// Normalizes core-equivalent CPU usage to host percent.
pub fn cpu_host_percent(cpu_core_percent: f64, logical_cores: usize) -> f64 {
    let cores = logical_cores.max(1) as f64;
    (cpu_core_percent / cores).clamp(0.0, 100.0)
}

fn truncate_reason(reason: &str) -> String {
    if reason.len() <= MAX_METRIC_REASON_LEN {
        return reason.to_string();
    }
    reason.chars().take(MAX_METRIC_REASON_LEN).collect()
}

fn build_quality(
    session: &RegisteredSession,
    root_status: RootStatus,
    overlap: bool,
    parent_gaps: bool,
    cpu_ready: CounterReadiness,
    io_ready: CounterReadiness,
    io_quality: IoQuality,
    partial_io: bool,
) -> MetricQuality {
    let mut reason = None;

    let attribution = match root_status {
        RootStatus::Valid if overlap || session.attribution == AttributionQuality::Shared => {
            if overlap {
                reason = Some(truncate_reason(
                    "process tree overlaps a nearer registered ancestor",
                ));
            }
            AttributionQuality::Shared
        }
        RootStatus::Valid
            if parent_gaps || session.attribution == AttributionQuality::Heuristic =>
        {
            if parent_gaps {
                reason = Some(truncate_reason(
                    "parent chain has protected or missed processes",
                ));
            }
            AttributionQuality::Heuristic
        }
        RootStatus::Valid => session.attribution,
        RootStatus::Missing => {
            reason = Some(truncate_reason("registered root process is not visible"));
            AttributionQuality::Unknown
        }
        RootStatus::PidReused => {
            reason = Some(truncate_reason(
                "pid was reused; start time no longer matches registration",
            ));
            AttributionQuality::Unknown
        }
    };

    let cpu = if root_status != RootStatus::Valid {
        MetricAvailability::Unavailable
    } else if cpu_ready == CounterReadiness::WarmingUp {
        MetricAvailability::WarmingUp
    } else {
        MetricAvailability::Available
    };

    let io = if root_status != RootStatus::Valid {
        IoQuality::Unavailable
    } else if io_ready == CounterReadiness::WarmingUp {
        io_quality
    } else if partial_io {
        IoQuality::Partial
    } else {
        io_quality
    };

    MetricQuality {
        attribution,
        cpu,
        io,
        reason,
    }
}

/// Computes per-session tree metrics and exclusive ownership map.
pub fn compute_session_metrics(
    sessions: &[RegisteredSession],
    processes: &BTreeMap<u32, ProcessNode>,
    _logical_cores: usize,
    elapsed_ms: u64,
    at_ms: i64,
    cpu_ready: CounterReadiness,
    io_ready: CounterReadiness,
    io_quality: IoQuality,
) -> (BTreeMap<String, TreeMetrics>, BTreeMap<u32, usize>) {
    let children = children_index(processes);
    let owners = assign_process_owners(sessions, processes);
    let mut metrics = BTreeMap::new();

    for (session_idx, session) in sessions.iter().enumerate() {
        let root_status = verify_root(&session.root, processes);
        let owned: BTreeSet<u32> = owners
            .iter()
            .filter_map(|(pid, owner)| (*owner == session_idx).then_some(*pid))
            .collect();

        let overlap = root_status == RootStatus::Valid
            && tree_has_overlap(session_idx, session.root.pid, &owners, processes, &children);
        let parent_gaps = root_status == RootStatus::Valid
            && tree_has_parent_gaps(session.root.pid, &owned, processes);

        let mut cpu_core_percent = 0.0;
        let mut rss_bytes = 0_u64;
        let mut read_delta = 0_u64;
        let mut write_delta = 0_u64;
        let mut io_seen = 0_u32;
        let mut io_available_count = 0_u32;

        if root_status == RootStatus::Valid {
            for pid in &owned {
                if let Some(node) = processes.get(pid) {
                    cpu_core_percent += node.cpu_usage_percent;
                    rss_bytes += node.rss_bytes;
                    read_delta += node.read_bytes_delta;
                    write_delta += node.write_bytes_delta;
                    io_seen += 1;
                    if node.io_available {
                        io_available_count += 1;
                    }
                }
            }
        }

        let partial_io = io_seen > 0 && io_available_count < io_seen;
        let runtime_ms = processes
            .get(&session.root.pid)
            .map(|node| (at_ms - node.started_at_ms).max(0) as u64)
            .unwrap_or(0);

        let quality = build_quality(
            session,
            root_status,
            overlap,
            parent_gaps,
            cpu_ready,
            io_ready,
            io_quality,
            partial_io,
        );

        metrics.insert(
            session.session_id.clone(),
            TreeMetrics {
                cpu_core_percent: if quality.cpu == MetricAvailability::Unavailable {
                    0.0
                } else {
                    cpu_core_percent
                },
                rss_bytes: if quality.cpu == MetricAvailability::Unavailable {
                    0
                } else {
                    rss_bytes
                },
                runtime_ms,
                process_count: owned.len() as u32,
                read_bytes_per_sec: if matches!(quality.io, IoQuality::Unavailable) {
                    0
                } else {
                    bytes_per_sec(read_delta, elapsed_ms)
                },
                write_bytes_per_sec: if matches!(quality.io, IoQuality::Unavailable) {
                    0
                } else {
                    bytes_per_sec(write_delta, elapsed_ms)
                },
                quality,
            },
        );
    }

    (metrics, owners)
}

pub fn tree_to_sample(at_ms: i64, tree: &TreeMetrics, logical_cores: usize) -> MetricSample {
    MetricSample {
        at_ms,
        cpu_core_percent: tree.cpu_core_percent,
        cpu_host_percent: cpu_host_percent(tree.cpu_core_percent, logical_cores),
        rss_bytes: tree.rss_bytes,
        runtime_ms: tree.runtime_ms,
        process_count: tree.process_count,
        read_bytes_per_sec: tree.read_bytes_per_sec,
        write_bytes_per_sec: tree.write_bytes_per_sec,
        quality: tree.quality.clone(),
    }
}

/// Builds a deduplicated aggregate across attributed process trees.
pub fn compute_aggregate(
    session_metrics: &BTreeMap<String, TreeMetrics>,
    at_ms: i64,
    logical_cores: usize,
    active_sessions: u32,
    attention_sessions: u32,
    io_quality: IoQuality,
) -> AgentAggregate {
    let mut cpu_core_percent = 0.0;
    let mut rss_bytes = 0_u64;
    let mut runtime_ms = 0_u64;
    let mut process_count = 0_u32;
    let mut read_bytes_per_sec = 0_u64;
    let mut write_bytes_per_sec = 0_u64;

    let mut attribution = AttributionQuality::Exact;
    let mut cpu = MetricAvailability::Available;
    let mut io = io_quality;
    let mut reasons = Vec::new();
    let mut contributed = 0_u32;

    for tree in session_metrics.values() {
        if tree.quality.cpu == MetricAvailability::Unavailable {
            continue;
        }
        contributed += 1;
        cpu_core_percent += tree.cpu_core_percent;
        rss_bytes += tree.rss_bytes;
        runtime_ms = runtime_ms.max(tree.runtime_ms);
        process_count += tree.process_count;
        read_bytes_per_sec += tree.read_bytes_per_sec;
        write_bytes_per_sec += tree.write_bytes_per_sec;

        attribution = worsen_attribution(attribution, tree.quality.attribution);
        cpu = worsen_cpu(cpu, tree.quality.cpu);
        io = worsen_io(io, tree.quality.io);
        if let Some(reason) = &tree.quality.reason {
            reasons.push(reason.as_str());
        }
    }

    if session_metrics.is_empty() || contributed == 0 {
        cpu = MetricAvailability::Unavailable;
        io = IoQuality::Unavailable;
        attribution = AttributionQuality::Unknown;
    }

    let reason = if reasons.is_empty() {
        None
    } else {
        Some(truncate_reason(&reasons.join("; ")))
    };

    AgentAggregate {
        at_ms,
        cpu_core_percent,
        cpu_host_percent: cpu_host_percent(cpu_core_percent, logical_cores),
        rss_bytes,
        runtime_ms,
        process_count,
        read_bytes_per_sec,
        write_bytes_per_sec,
        quality: MetricQuality {
            attribution,
            cpu,
            io,
            reason,
        },
        active_sessions,
        attention_sessions,
    }
}

fn worsen_attribution(current: AttributionQuality, next: AttributionQuality) -> AttributionQuality {
    use AttributionQuality::*;
    match (current, next) {
        (Unknown, _) | (_, Unknown) => Unknown,
        (Heuristic, _) | (_, Heuristic) => Heuristic,
        (Shared, _) | (_, Shared) => Shared,
        _ => Exact,
    }
}

fn worsen_cpu(current: MetricAvailability, next: MetricAvailability) -> MetricAvailability {
    use MetricAvailability::*;
    match (current, next) {
        (Unavailable, _) | (_, Unavailable) => Unavailable,
        (WarmingUp, _) | (_, WarmingUp) => WarmingUp,
        _ => Available,
    }
}

fn worsen_io(current: IoQuality, next: IoQuality) -> IoQuality {
    use IoQuality::*;
    match (current, next) {
        (Unavailable, _) | (_, Unavailable) => Unavailable,
        (Partial, _) | (_, Partial) => Partial,
        (Disk, AllIo) | (AllIo, Disk) => Partial,
        _ => current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RegisteredSession;
    use notch_protocol::{AttributionQuality, ProcessIdentity};

    fn node(
        pid: u32,
        parent: Option<u32>,
        cpu: f64,
        rss: u64,
        read: u64,
        write: u64,
    ) -> ProcessNode {
        ProcessNode {
            pid,
            parent_pid: parent,
            started_at_ms: 1_000,
            cpu_usage_percent: cpu,
            rss_bytes: rss,
            read_bytes_total: read,
            write_bytes_total: write,
            read_bytes_delta: read,
            write_bytes_delta: write,
            io_available: true,
        }
    }

    #[test]
    fn aggregate_unavailable_when_no_trees_contribute() {
        let mut metrics = BTreeMap::new();
        metrics.insert(
            "sess-1".into(),
            TreeMetrics {
                cpu_core_percent: 0.0,
                rss_bytes: 0,
                runtime_ms: 0,
                process_count: 0,
                read_bytes_per_sec: 0,
                write_bytes_per_sec: 0,
                quality: MetricQuality {
                    attribution: AttributionQuality::Exact,
                    cpu: MetricAvailability::Unavailable,
                    io: IoQuality::Disk,
                    reason: Some("missing root".into()),
                },
            },
        );
        let agg = compute_aggregate(&metrics, 2_000, 4, 1, 0, IoQuality::Disk);
        assert_eq!(agg.quality.cpu, MetricAvailability::Unavailable);
    }

    #[test]
    fn aggregate_deduplicates_exclusive_trees() {
        let mut processes = BTreeMap::new();
        processes.insert(1, node(1, None, 10.0, 100, 1_000, 2_000));
        processes.insert(2, node(2, Some(1), 5.0, 50, 500, 700));
        let sessions = vec![RegisteredSession {
            session_id: "a".into(),
            root: ProcessIdentity {
                pid: 1,
                started_at_ms: 1_000,
            },
            attribution: AttributionQuality::Exact,
            registered_at_ms: 0,
        }];
        let (metrics, _) = compute_session_metrics(
            &sessions,
            &processes,
            4,
            1_000,
            2_000,
            CounterReadiness::Ready,
            CounterReadiness::Ready,
            IoQuality::Disk,
        );
        let agg = compute_aggregate(&metrics, 2_000, 4, 1, 0, IoQuality::Disk);
        assert_eq!(agg.cpu_core_percent, 15.0);
        assert_eq!(agg.rss_bytes, 150);
        assert_eq!(agg.process_count, 2);
        assert_eq!(agg.read_bytes_per_sec, 1_500);
    }

    #[test]
    fn aggregate_worsens_attribution_quality_honestly() {
        let mut processes = BTreeMap::new();
        processes.insert(1, node(1, None, 1.0, 10, 100, 100));
        processes.insert(2, node(2, None, 1.0, 10, 100, 100));
        let sessions = vec![
            RegisteredSession {
                session_id: "exact".into(),
                root: ProcessIdentity {
                    pid: 1,
                    started_at_ms: 1_000,
                },
                attribution: AttributionQuality::Exact,
                registered_at_ms: 0,
            },
            RegisteredSession {
                session_id: "shared".into(),
                root: ProcessIdentity {
                    pid: 2,
                    started_at_ms: 1_000,
                },
                attribution: AttributionQuality::Shared,
                registered_at_ms: 0,
            },
        ];
        let (metrics, _) = compute_session_metrics(
            &sessions,
            &processes,
            4,
            1_000,
            2_000,
            CounterReadiness::Ready,
            CounterReadiness::Ready,
            IoQuality::Disk,
        );
        let agg = compute_aggregate(&metrics, 2_000, 4, 2, 0, IoQuality::Disk);
        assert_eq!(agg.quality.attribution, AttributionQuality::Shared);
    }

    #[test]
    fn warming_up_reports_quality_not_values() {
        let mut processes = BTreeMap::new();
        processes.insert(9, node(9, None, 99.0, 999, 9_000, 9_000));
        let sessions = vec![RegisteredSession {
            session_id: "warm".into(),
            root: ProcessIdentity {
                pid: 9,
                started_at_ms: 1_000,
            },
            attribution: AttributionQuality::Exact,
            registered_at_ms: 0,
        }];
        let (metrics, _) = compute_session_metrics(
            &sessions,
            &processes,
            8,
            1_000,
            2_000,
            CounterReadiness::WarmingUp,
            CounterReadiness::WarmingUp,
            IoQuality::Disk,
        );
        let sample = tree_to_sample(2_000, metrics.get("warm").unwrap(), 8);
        assert_eq!(sample.quality.cpu, MetricAvailability::WarmingUp);
        assert_eq!(sample.cpu_core_percent, 99.0);
        assert_eq!(sample.read_bytes_per_sec, 9_000);
    }

    #[test]
    fn missing_root_marks_metrics_unavailable() {
        let processes = BTreeMap::new();
        let sessions = vec![RegisteredSession {
            session_id: "gone".into(),
            root: ProcessIdentity {
                pid: 404,
                started_at_ms: 1,
            },
            attribution: AttributionQuality::Exact,
            registered_at_ms: 0,
        }];
        let (metrics, _) = compute_session_metrics(
            &sessions,
            &processes,
            4,
            1_000,
            1_000,
            CounterReadiness::Ready,
            CounterReadiness::Ready,
            IoQuality::Disk,
        );
        let tree = metrics.get("gone").expect("tree");
        assert_eq!(tree.quality.cpu, MetricAvailability::Unavailable);
        assert_eq!(tree.quality.io, IoQuality::Unavailable);
        assert_eq!(tree.cpu_core_percent, 0.0);
        assert!(tree.quality.reason.is_some());
        assert_eq!(tree.quality.attribution, AttributionQuality::Unknown);
    }
}

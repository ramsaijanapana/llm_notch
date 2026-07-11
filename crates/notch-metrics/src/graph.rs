use std::collections::{BTreeMap, BTreeSet};

use notch_protocol::ProcessIdentity;

use crate::constants::MAX_PARENT_WALK_DEPTH;
use crate::model::{ProcessNode, RegisteredSession, RootStatus};

/// Returns whether an observed process start time matches the registered identity.
pub fn start_times_match(expected_ms: i64, observed_ms: i64) -> bool {
    (expected_ms - observed_ms).abs() < 1_000
}

/// Validates that `identity` still refers to the same live process.
pub fn verify_root(
    identity: &ProcessIdentity,
    processes: &BTreeMap<u32, ProcessNode>,
) -> RootStatus {
    let Some(node) = processes.get(&identity.pid) else {
        return RootStatus::Missing;
    };
    if start_times_match(identity.started_at_ms, node.started_at_ms) {
        RootStatus::Valid
    } else {
        RootStatus::PidReused
    }
}

/// Builds a parent→children adjacency index from the snapshot.
pub fn children_index(processes: &BTreeMap<u32, ProcessNode>) -> BTreeMap<u32, Vec<u32>> {
    let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for node in processes.values() {
        if let Some(parent) = node.parent_pid {
            children.entry(parent).or_default().push(node.pid);
        }
    }
    for kids in children.values_mut() {
        kids.sort_unstable();
    }
    children
}

/// Collects all descendant PIDs reachable below `root_pid` in the snapshot.
pub fn collect_descendants(
    root_pid: u32,
    processes: &BTreeMap<u32, ProcessNode>,
    children: &BTreeMap<u32, Vec<u32>>,
) -> BTreeSet<u32> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![root_pid];
    while let Some(pid) = stack.pop() {
        if !seen.insert(pid) {
            continue;
        }
        if let Some(kids) = children.get(&pid) {
            for kid in kids {
                if processes.contains_key(kid) {
                    stack.push(*kid);
                }
            }
        }
    }
    seen
}

/// Assigns each visible PID to the nearest registered ancestor session index.
pub fn assign_process_owners(
    sessions: &[RegisteredSession],
    processes: &BTreeMap<u32, ProcessNode>,
) -> BTreeMap<u32, usize> {
    let valid_roots: Vec<(usize, u32)> = sessions
        .iter()
        .enumerate()
        .filter_map(|(idx, session)| {
            if verify_root(&session.root, processes) == RootStatus::Valid {
                Some((idx, session.root.pid))
            } else {
                None
            }
        })
        .collect();

    let mut owners = BTreeMap::new();
    for &pid in processes.keys() {
        if let Some(owner) = nearest_registered_owner(pid, &valid_roots, processes) {
            owners.insert(pid, owner);
        }
    }
    owners
}

fn nearest_registered_owner(
    pid: u32,
    roots: &[(usize, u32)],
    processes: &BTreeMap<u32, ProcessNode>,
) -> Option<usize> {
    let mut current = pid;
    for _ in 0..MAX_PARENT_WALK_DEPTH {
        if let Some(&(idx, _)) = roots.iter().find(|(_, root_pid)| *root_pid == current) {
            return Some(idx);
        }
        let parent = processes.get(&current)?.parent_pid?;
        current = parent;
    }
    None
}

/// Returns `true` when any owned PID is attributed to a different session.
pub fn tree_has_overlap(
    session_idx: usize,
    root_pid: u32,
    owners: &BTreeMap<u32, usize>,
    processes: &BTreeMap<u32, ProcessNode>,
    children: &BTreeMap<u32, Vec<u32>>,
) -> bool {
    collect_descendants(root_pid, processes, children)
        .into_iter()
        .filter(|pid| *pid != root_pid)
        .any(|pid| owners.get(&pid).is_some_and(|owner| *owner != session_idx))
}

/// Returns `true` when a parent hop is missing between root and an owned descendant.
pub fn tree_has_parent_gaps(
    root_pid: u32,
    owned_pids: &BTreeSet<u32>,
    processes: &BTreeMap<u32, ProcessNode>,
) -> bool {
    for &pid in owned_pids {
        if pid == root_pid {
            continue;
        }
        let mut current = pid;
        for _ in 0..MAX_PARENT_WALK_DEPTH {
            let Some(node) = processes.get(&current) else {
                return true;
            };
            let Some(parent) = node.parent_pid else {
                return true;
            };
            if parent == root_pid {
                break;
            }
            current = parent;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use notch_protocol::AttributionQuality;

    fn node(pid: u32, parent: Option<u32>, started_ms: i64) -> ProcessNode {
        ProcessNode {
            pid,
            parent_pid: parent,
            started_at_ms: started_ms,
            cpu_usage_percent: 0.0,
            rss_bytes: 0,
            read_bytes_total: 0,
            write_bytes_total: 0,
            read_bytes_delta: 0,
            write_bytes_delta: 0,
            io_available: true,
        }
    }

    fn processes(edges: &[(u32, Option<u32>, i64)]) -> BTreeMap<u32, ProcessNode> {
        edges
            .iter()
            .map(|(pid, parent, started)| (*pid, node(*pid, *parent, *started)))
            .collect()
    }

    #[test]
    fn pid_reuse_is_detected() {
        let mut map = processes(&[(10, None, 1_000)]);
        assert_eq!(
            verify_root(
                &ProcessIdentity {
                    pid: 10,
                    started_at_ms: 1_000,
                },
                &map
            ),
            RootStatus::Valid
        );
        map.insert(10, node(10, None, 9_000));
        assert_eq!(
            verify_root(
                &ProcessIdentity {
                    pid: 10,
                    started_at_ms: 1_000,
                },
                &map
            ),
            RootStatus::PidReused
        );
    }

    #[test]
    fn nearest_ancestor_wins_overlap() {
        let map = processes(&[
            (100, None, 1_000),
            (110, Some(100), 1_100),
            (120, Some(110), 1_200),
        ]);
        let sessions = vec![
            RegisteredSession {
                session_id: "parent".into(),
                root: ProcessIdentity {
                    pid: 100,
                    started_at_ms: 1_000,
                },
                attribution: AttributionQuality::Exact,
                registered_at_ms: 0,
            },
            RegisteredSession {
                session_id: "child".into(),
                root: ProcessIdentity {
                    pid: 110,
                    started_at_ms: 1_100,
                },
                attribution: AttributionQuality::Exact,
                registered_at_ms: 0,
            },
        ];
        let owners = assign_process_owners(&sessions, &map);
        assert_eq!(owners.get(&100), Some(&0));
        assert_eq!(owners.get(&110), Some(&1));
        assert_eq!(owners.get(&120), Some(&1));
    }

    #[test]
    fn overlap_detection_flags_shared_trees() {
        let map = processes(&[(1, None, 1), (2, Some(1), 2), (3, Some(2), 3)]);
        let children = children_index(&map);
        let sessions = vec![
            RegisteredSession {
                session_id: "a".into(),
                root: ProcessIdentity {
                    pid: 1,
                    started_at_ms: 1,
                },
                attribution: AttributionQuality::Exact,
                registered_at_ms: 0,
            },
            RegisteredSession {
                session_id: "b".into(),
                root: ProcessIdentity {
                    pid: 2,
                    started_at_ms: 2,
                },
                attribution: AttributionQuality::Exact,
                registered_at_ms: 0,
            },
        ];
        let owners = assign_process_owners(&sessions, &map);
        assert!(tree_has_overlap(0, 1, &owners, &map, &children));
        assert!(!tree_has_overlap(1, 2, &owners, &map, &children));
    }
}

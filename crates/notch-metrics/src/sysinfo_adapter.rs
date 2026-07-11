use std::collections::BTreeMap;
use std::time::Instant;

use notch_protocol::{HostMetricSample, IoQuality, ProcessIdentity};
use sysinfo::{
    Disks, MemoryRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System,
};

use crate::aggregate::bytes_per_sec;
use crate::model::{CounterReadiness, ProcessNode};

/// Resolves the current OS start time for a PID before registering a root.
///
/// Hook timestamps describe the event, not necessarily process creation. The
/// host replaces them with this observed identity to preserve PID-reuse checks.
pub fn resolve_process_identity(pid: u32) -> Option<ProcessIdentity> {
    let system = System::new_all();
    system
        .process(Pid::from_u32(pid))
        .map(|process| ProcessIdentity {
            pid,
            started_at_ms: (process.start_time() as i64) * 1_000,
        })
}

/// Snapshot captured from a persistent `sysinfo` probe.
#[derive(Debug, Clone)]
pub struct ProcessSnapshot {
    pub at_ms: i64,
    pub elapsed_ms: u64,
    pub logical_cores: usize,
    pub host_cpu_percent: f64,
    pub used_memory_bytes: u64,
    pub total_memory_bytes: u64,
    pub visible_process_count: u32,
    pub disk_read_bytes_per_sec: u64,
    pub disk_write_bytes_per_sec: u64,
    pub processes: BTreeMap<u32, ProcessNode>,
    pub cpu_ready: CounterReadiness,
    pub io_ready: CounterReadiness,
    pub io_quality: IoQuality,
}

/// Persistent host/process probe backed by `sysinfo::System` and `sysinfo::Disks`.
#[derive(Debug)]
pub struct SysinfoProbe {
    system: System,
    disks: Disks,
    logical_cores: usize,
    io_quality: IoQuality,
    refresh_count: u64,
    last_refresh_ms: i64,
    last_disk_read_total: u64,
    last_disk_write_total: u64,
}

impl Default for SysinfoProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl SysinfoProbe {
    pub fn new() -> Self {
        let mut system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_memory(MemoryRefreshKind::everything())
                .with_processes(
                    ProcessRefreshKind::nothing()
                        .with_memory()
                        .with_disk_usage(),
                ),
        );
        system.refresh_cpu_usage();
        system.refresh_memory();

        let mut disks = Disks::new_with_refreshed_list();
        for disk in disks.list_mut() {
            disk.refresh();
        }

        let (read_total, write_total) = disk_totals(&disks);

        let logical_cores = system.cpus().len().max(1);

        Self {
            logical_cores,
            system,
            disks,
            io_quality: platform_io_quality(),
            refresh_count: 0,
            last_refresh_ms: 0,
            last_disk_read_total: read_total,
            last_disk_write_total: write_total,
        }
    }

    pub fn logical_cores(&self) -> usize {
        self.logical_cores
    }

    pub fn io_quality(&self) -> IoQuality {
        self.io_quality
    }

    pub fn refresh_count(&self) -> u64 {
        self.refresh_count
    }

    pub fn refresh(&mut self, at_ms: i64) -> ProcessSnapshot {
        let started = Instant::now();
        let elapsed_ms = if self.last_refresh_ms > 0 {
            (at_ms - self.last_refresh_ms).max(1) as u64
        } else {
            1_000
        };

        self.system.refresh_cpu_usage();
        self.system.refresh_memory();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage(),
        );

        for disk in self.disks.list_mut() {
            disk.refresh();
        }

        let (disk_read_total, disk_write_total) = disk_totals(&self.disks);
        let read_delta = disk_read_total.saturating_sub(self.last_disk_read_total);
        let write_delta = disk_write_total.saturating_sub(self.last_disk_write_total);
        self.last_disk_read_total = disk_read_total;
        self.last_disk_write_total = disk_write_total;

        self.refresh_count = self.refresh_count.saturating_add(1);
        self.last_refresh_ms = at_ms;

        let cpu_ready = if self.refresh_count >= 2 {
            CounterReadiness::Ready
        } else {
            CounterReadiness::WarmingUp
        };
        let io_ready = cpu_ready;

        let processes = self.collect_processes();
        let snapshot = ProcessSnapshot {
            at_ms,
            elapsed_ms,
            logical_cores: self.logical_cores,
            host_cpu_percent: self.system.global_cpu_usage() as f64,
            used_memory_bytes: self.system.used_memory(),
            total_memory_bytes: self.system.total_memory(),
            visible_process_count: self.system.processes().len() as u32,
            disk_read_bytes_per_sec: bytes_per_sec(read_delta, elapsed_ms),
            disk_write_bytes_per_sec: bytes_per_sec(write_delta, elapsed_ms),
            processes,
            cpu_ready,
            io_ready,
            io_quality: self.io_quality,
        };

        tracing::trace!(
            elapsed_ms,
            refresh_count = self.refresh_count,
            overhead_us = started.elapsed().as_micros(),
            visible = snapshot.visible_process_count,
            "sysinfo refresh completed"
        );

        snapshot
    }

    fn collect_processes(&self) -> BTreeMap<u32, ProcessNode> {
        let mut processes = BTreeMap::new();
        for (pid, process) in self.system.processes() {
            let disk = process.disk_usage();
            let io_available = disk.total_read_bytes > 0
                || disk.total_written_bytes > 0
                || disk.read_bytes > 0
                || disk.written_bytes > 0;
            processes.insert(
                pid.as_u32(),
                ProcessNode {
                    pid: pid.as_u32(),
                    parent_pid: process.parent().map(|p| p.as_u32()),
                    started_at_ms: (process.start_time() as i64) * 1_000,
                    cpu_usage_percent: process.cpu_usage() as f64,
                    rss_bytes: process.memory(),
                    read_bytes_total: disk.total_read_bytes,
                    write_bytes_total: disk.total_written_bytes,
                    read_bytes_delta: disk.read_bytes,
                    write_bytes_delta: disk.written_bytes,
                    io_available,
                },
            );
        }
        processes
    }
}

impl ProcessSnapshot {
    pub fn host_sample(&self) -> HostMetricSample {
        HostMetricSample {
            at_ms: self.at_ms,
            cpu_host_percent: self.host_cpu_percent,
            used_memory_bytes: self.used_memory_bytes,
            total_memory_bytes: self.total_memory_bytes,
            visible_process_count: self.visible_process_count,
            disk_read_bytes_per_sec: if self.io_ready.is_ready() {
                self.disk_read_bytes_per_sec
            } else {
                0
            },
            disk_write_bytes_per_sec: if self.io_ready.is_ready() {
                self.disk_write_bytes_per_sec
            } else {
                0
            },
        }
    }
}

fn disk_totals(disks: &Disks) -> (u64, u64) {
    disks
        .list()
        .iter()
        .map(|disk| {
            let usage = disk.usage();
            (usage.total_read_bytes, usage.total_written_bytes)
        })
        .fold((0, 0), |(read, write), (r, w)| (read + r, write + w))
}

pub fn platform_io_quality() -> IoQuality {
    if cfg!(target_os = "windows") {
        IoQuality::AllIo
    } else {
        IoQuality::Disk
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_io_quality_matches_os() {
        if cfg!(target_os = "windows") {
            assert_eq!(platform_io_quality(), IoQuality::AllIo);
        } else {
            assert_eq!(platform_io_quality(), IoQuality::Disk);
        }
    }
}

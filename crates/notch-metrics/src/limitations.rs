//! Platform and data-source limitations for host/process metrics.
//!
//! # RSS and shared memory
//!
//! Per-process RSS comes from the OS resident-set size. Shared pages (libraries,
//! mmap'd files, copy-on-write) may be counted in multiple process trees, so
//! summed agent RSS can modestly exceed host used memory.
//!
//! # Process I/O counters
//!
//! - **Linux / macOS**: `sysinfo` reads `/proc/<pid>/io` or the platform
//!   equivalent. Counters generally reflect block-device I/O, not sockets or
//!   GPU traffic. Cached reads may not increment counters.
//! - **Windows**: `Process::disk_usage` reports **all** I/O bytes (disk, pipe,
//!   socket, etc.), so agent I/O totals are not directly comparable to Linux/macOS.
//!
//! # Protected and missed processes
//!
//! Processes owned by other users, sandboxed helpers, or short-lived children may
//! be absent from the snapshot. Parent-chain gaps force heuristic attribution
//! and can hide part of a tree until the next refresh.
//!
//! # Pooled Cursor / IDE trees
//!
//! Multiple agent sessions that share a long-lived IDE or terminal parent should
//! register the nearest distinct root practical for each session. Overlapping
//! trees are deduplicated to the nearest registered ancestor, which often marks
//! parent sessions as [`notch_protocol::AttributionQuality::Shared`].
//!
//! # Deliberately excluded
//!
//! This crate does not attempt elevation, shell probing, network attribution,
//! GPU, or energy metrics.

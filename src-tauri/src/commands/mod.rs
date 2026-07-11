//! Narrow Tauri commands. No command accepts arbitrary paths, command lines,
//! file bodies, or network destinations.

pub mod bootstrap;
pub mod context;
pub mod error;
pub mod integration;
pub mod overlay;
pub mod settings;
pub mod types;
pub mod validation;

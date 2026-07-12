//! Narrow Tauri commands. No command accepts arbitrary paths, command lines,
//! file bodies, or network destinations.

pub mod bootstrap;
pub mod catalog;
pub mod context;
pub mod decision;
pub mod error;
pub mod integration;
pub mod overlay;
pub mod remote;
pub mod services;
pub mod settings;
pub mod types;
pub mod validation;

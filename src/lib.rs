//! Conflux - OpenSpec Orchestrator library
//!
//! This library provides the core functionality for the Conflux orchestrator,
//! including web monitoring APIs and event handling.
//!
//! This library crate exposes only the modules needed for the OpenAPI generator binary.
//! The main application logic is in the binary crate (main.rs).

// Allow dead code for internal modules that are only used by the binary crate
#![allow(dead_code)]

// Public modules for OpenAPI generator
pub mod events;
pub mod tui;

#[cfg(feature = "web-monitoring")]
pub mod web;

// Internal modules required by public modules
mod acceptance;
mod agent;
mod ai_command_runner;
mod analyzer;
mod cli;
mod command_queue;
mod config;
mod error;
mod error_history;
mod execution;
mod history;
mod hooks;
mod merge_stall_monitor;
mod openspec;
mod orchestration;
mod orchestrator;
mod parallel;
mod parallel_run_service;
mod permission;
mod process_manager;
mod progress;
mod remote;
mod serial_run_service;
mod spec_delta;
mod stall;
mod stream_json_textifier;
mod task_parser;
mod templates;
mod vcs;
pub mod worktree_ops;

#[cfg(test)]
mod test_support;

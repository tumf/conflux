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
pub mod embedded_skills;
pub mod events;
pub mod install_skills;
pub mod lifecycle_integration;
pub mod log_viewer;
pub mod logging;
pub mod tui;

#[cfg(feature = "web-monitoring")]
pub mod web;

// Internal modules required by public modules
mod acceptance;
mod agent;
pub mod ai_command_runner;
mod analyzer;
mod archive_layout;
mod bounded_git;
pub mod cli;
pub mod client;
mod command_queue;
pub mod completion;
pub mod config;
mod dependency_targets;
mod error;
mod error_history;
mod execution;
mod history;
mod hooks;
// Public so `tests/client_cli_tests.rs` can build the same monitoring snapshot a
// real owner publishes when it wires a production coordinator.
pub mod openspec;
pub mod openspec_cmd;
pub mod orchestration;
mod orchestrator;
mod parallel;
mod parallel_run_service;
mod permission;
// Public so `tests/process_cleanup_test.rs` can drive real process-group
// cleanup against a real managed worktree.
pub mod process_manager;
pub mod repo_lock;
pub mod runtime;
mod shell_command;
mod spec_delta;
mod stall;
mod stream_json_textifier;
mod task_parser;
mod templates;
pub mod upstream;
mod vcs;
pub mod worktree_ops;

#[cfg(test)]
mod test_support;

//! Conflux - OpenSpec Orchestrator library
//!
//! This library provides the core functionality for the Conflux orchestrator,
//! including web monitoring APIs and event handling.

// Only expose modules needed for the OpenAPI generator
#[cfg(feature = "web-monitoring")]
pub mod web;

pub mod events;
pub mod tui;

// Re-export modules required by web module but not directly part of public API
mod acceptance;
mod agent;
mod ai_command_runner;
mod analyzer;
mod approval;
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
mod process_manager;
mod progress;
mod serial_run_service;
mod spec_delta;
#[cfg(test)]
mod spec_test_annotations;
mod stall;
mod task_parser;
mod templates;
mod vcs;

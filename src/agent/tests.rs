//! Tests for agent module

use super::*;
use crate::agent::prompt::APPLY_SYSTEM_PROMPT;
use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
use crate::command_queue::CommandQueueConfig;
use crate::config::defaults::*;
use crate::config::OrchestratorConfig;
use std::sync::Arc;
use tokio::sync::Mutex;

fn build_test_ai_runner() -> AiCommandRunner {
    let shared_stagger_state: SharedStaggerState = Arc::new(Mutex::new(None));
    let queue_config = CommandQueueConfig {
        stagger_delay_ms: 0,
        max_retries: DEFAULT_MAX_RETRIES,
        retry_delay_ms: DEFAULT_RETRY_DELAY_MS,
        retry_error_patterns: vec![],
        retry_if_duration_under_secs: DEFAULT_RETRY_IF_DURATION_UNDER_SECS,
        inactivity_timeout_secs: 0,
        inactivity_kill_grace_secs: 1,
        inactivity_timeout_max_retries: 0,
        strict_process_cleanup: true,
    };

    AiCommandRunner::new(queue_config, shared_stagger_state)
}

#[test]
fn test_agent_runner_creation() {
    let config = OrchestratorConfig {
        apply_command: Some("test apply {change_id}".to_string()),
        ..Default::default()
    };
    let runner = AgentRunner::new(config);
    assert_eq!(
        runner.config().get_apply_command().unwrap(),
        "test apply {change_id}"
    );
}

#[test]
fn test_agent_runner_with_custom_config() {
    let config = OrchestratorConfig {
        apply_command: Some("custom-agent apply {change_id}".to_string()),
        archive_command: Some("custom-agent archive {change_id}".to_string()),
        analyze_command: Some("custom-agent analyze '{prompt}'".to_string()),
        ..Default::default()
    };
    let runner = AgentRunner::new(config);
    assert_eq!(
        runner.config().get_apply_command().unwrap(),
        "custom-agent apply {change_id}"
    );
    assert_eq!(
        runner.config().get_archive_command().unwrap(),
        "custom-agent archive {change_id}"
    );
}

#[tokio::test]
async fn test_run_apply_echo_command() {
    let config = OrchestratorConfig {
        apply_command: Some("echo {change_id}".to_string()),
        ..Default::default()
    };
    let mut runner = AgentRunner::new(config);
    let result = runner.run_apply("test-change").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_run_archive_echo_command() {
    let config = OrchestratorConfig {
        archive_command: Some("echo {change_id}".to_string()),
        ..Default::default()
    };
    let runner = AgentRunner::new(config);
    let result = runner.run_archive("test-change").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_analyze_dependencies_echo_command() {
    let config = OrchestratorConfig {
        analyze_command: Some("echo '{prompt}'".to_string()),
        ..Default::default()
    };
    let runner = AgentRunner::new(config);
    let result = runner.analyze_dependencies("test prompt").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().trim(), "test prompt");
}

#[tokio::test]
async fn test_run_apply_with_prompt_expansion() {
    let config = OrchestratorConfig {
        apply_command: Some("echo {change_id} {prompt}".to_string()),
        apply_prompt: Some("prompt-marker".to_string()),
        ..Default::default()
    };
    let mut runner = AgentRunner::new(config);
    let result = runner.run_apply_streaming("my-change", None).await;
    assert!(result.is_ok());
    let (mut child, mut rx, _start) = result.unwrap();

    // Collect output
    let mut lines = Vec::new();
    while let Some(line) = rx.recv().await {
        lines.push(line);
    }

    // Wait for child to complete
    let status = child.wait().await.unwrap();
    assert!(status.success());
    // Verify the output contains expanded change_id
    let output: String = lines
        .iter()
        .map(|l| match l {
            OutputLine::Stdout(s) => s.clone(),
            OutputLine::Stderr(s) => s.clone(),
        })
        .collect();
    assert!(output.contains("my-change"));
    assert!(output.contains("prompt-marker"));
}

#[tokio::test]
async fn test_run_apply_with_default_prompt() {
    let config = OrchestratorConfig {
        apply_command: Some("echo {prompt}".to_string()),
        apply_prompt: None, // Use default empty prompt
        ..Default::default()
    };
    let mut runner = AgentRunner::new(config);
    let result = runner.run_apply_streaming("my-change", None).await;
    assert!(result.is_ok());
    let (mut child, mut rx, _start) = result.unwrap();

    // Collect output
    let mut lines = Vec::new();
    while let Some(line) = rx.recv().await {
        lines.push(line);
    }

    // Wait for child to complete
    let status = child.wait().await.unwrap();
    assert!(status.success());
}

#[tokio::test]
async fn test_run_archive_with_empty_default_prompt() {
    let config = OrchestratorConfig {
        archive_command: Some("echo {prompt}".to_string()),
        archive_prompt: None, // Default empty prompt
        ..Default::default()
    };
    let runner = AgentRunner::new(config);
    let result = runner.run_archive_streaming("my-change", None).await;
    assert!(result.is_ok());
    let (mut child, mut rx, _start) = result.unwrap();

    // Collect output
    let mut lines = Vec::new();
    while let Some(line) = rx.recv().await {
        lines.push(line);
    }

    // Wait for child to complete
    let status = child.wait().await.unwrap();
    assert!(status.success());
}

#[tokio::test]
async fn test_run_apply_streaming_with_prompt() {
    let config = OrchestratorConfig {
        apply_command: Some("echo {change_id} {prompt}".to_string()),
        apply_prompt: Some("prompt-marker".to_string()),
        ..Default::default()
    };
    let mut runner = AgentRunner::new(config);
    let result = runner.run_apply_streaming("my-change", None).await;
    assert!(result.is_ok());
    let (mut child, mut rx, _start) = result.unwrap();

    // Collect output
    let mut lines = Vec::new();
    while let Some(line) = rx.recv().await {
        lines.push(line);
    }

    // Wait for child to complete
    let status = child.wait().await.unwrap();
    assert!(status.success());
    // Verify the output contains expanded change_id
    let output: String = lines
        .iter()
        .map(|l| match l {
            OutputLine::Stdout(s) => s.clone(),
            OutputLine::Stderr(s) => s.clone(),
        })
        .collect();
    assert!(output.contains("my-change"));
    assert!(output.contains("prompt-marker"));
}

#[tokio::test]
async fn test_with_runner_paths_preserve_prompt_and_output() {
    let config = OrchestratorConfig {
        apply_command: Some("echo apply:{change_id}:{prompt}".to_string()),
        apply_prompt: Some("apply-marker".to_string()),
        archive_command: Some("echo archive:{change_id}:{prompt}".to_string()),
        archive_prompt: Some("archive-marker".to_string()),
        acceptance_command: Some("echo acceptance:{change_id}:{prompt}".to_string()),
        acceptance_prompt: Some("acceptance-marker".to_string()),
        analyze_command: Some("echo analyze:{prompt}".to_string()),
        resolve_command: Some("echo resolve:{prompt}".to_string()),
        ..Default::default()
    };
    let mut runner = AgentRunner::new(config);
    let ai_runner = build_test_ai_runner();

    let (mut apply_child, mut apply_rx, _apply_start, apply_command) = runner
        .run_apply_streaming_with_runner("change-1", &ai_runner, None)
        .await
        .unwrap();
    let mut apply_output = String::new();
    while let Some(line) = apply_rx.recv().await {
        match line {
            OutputLine::Stdout(s) | OutputLine::Stderr(s) => apply_output.push_str(&s),
        }
    }
    let apply_status = apply_child.wait().await.unwrap();
    assert!(apply_status.success());
    assert!(apply_command.contains("change-1"));
    assert!(apply_command.contains("apply-marker"));
    assert!(apply_output.contains("apply:change-1"));

    let (mut archive_child, mut archive_rx, _archive_start, archive_command) = runner
        .run_archive_streaming_with_runner("change-1", &ai_runner, None)
        .await
        .unwrap();
    let mut archive_output = String::new();
    while let Some(line) = archive_rx.recv().await {
        match line {
            OutputLine::Stdout(s) | OutputLine::Stderr(s) => archive_output.push_str(&s),
        }
    }
    let archive_status = archive_child.wait().await.unwrap();
    assert!(archive_status.success());
    assert!(archive_command.contains("change-1"));
    assert!(archive_command.contains("archive-marker"));
    assert!(archive_output.contains("archive:change-1"));

    let (mut acceptance_child, mut acceptance_rx, _acceptance_start, acceptance_command) = runner
        .run_acceptance_streaming_with_runner("change-1", &ai_runner, None, None)
        .await
        .unwrap();
    let mut acceptance_output = String::new();
    while let Some(line) = acceptance_rx.recv().await {
        match line {
            OutputLine::Stdout(s) | OutputLine::Stderr(s) => acceptance_output.push_str(&s),
        }
    }
    let acceptance_status = acceptance_child.wait().await.unwrap();
    assert!(acceptance_status.success());
    assert!(acceptance_command.contains("change-1"));
    assert!(acceptance_command.contains("acceptance-marker"));
    assert!(acceptance_output.contains("acceptance:change-1"));

    let analyze = runner
        .analyze_dependencies_with_runner("analyze-marker", &ai_runner)
        .await
        .unwrap();
    assert!(analyze.contains("analyze:analyze-marker"));

    let (mut resolve_child, mut resolve_rx) = runner
        .run_resolve_streaming_in_dir_with_runner("resolve-marker", std::path::Path::new("."), &ai_runner)
        .await
        .unwrap();
    let mut resolve_output = String::new();
    while let Some(line) = resolve_rx.recv().await {
        match line {
            OutputLine::Stdout(s) | OutputLine::Stderr(s) => resolve_output.push_str(&s),
        }
    }
    let resolve_status = resolve_child.wait().await.unwrap();
    assert!(resolve_status.success());
    assert!(resolve_output.contains("resolve:resolve-marker"));
}

// Tests for build_apply_prompt function and prompt construction order

#[test]
fn test_build_apply_prompt_with_all_parts() {
    let user_prompt = "Focus on implementation.";
    let history_context = "Previous attempt failed.";
    let acceptance_tail = "";
    let result = build_apply_prompt("my-change", user_prompt, history_context, acceptance_tail);

    assert!(result.contains("Focus on implementation."));
    assert!(result.contains("Previous attempt failed."));
}

#[test]
fn test_build_apply_prompt_with_empty_user_prompt() {
    let user_prompt = "";
    let history_context = "Previous attempt failed.";
    let acceptance_tail = "";
    let result = build_apply_prompt("my-change", user_prompt, history_context, acceptance_tail);

    assert!(result.contains("Previous attempt failed."));
}

#[test]
fn test_build_apply_prompt_with_empty_history() {
    let user_prompt = "Focus on implementation.";
    let history_context = "";
    let acceptance_tail = "";
    let result = build_apply_prompt("my-change", user_prompt, history_context, acceptance_tail);

    assert!(result.contains("Focus on implementation."));
}

#[test]
fn test_build_apply_prompt_with_only_system_prompt() {
    let user_prompt = "";
    let history_context = "";
    let acceptance_tail = "";
    let result = build_apply_prompt("my-change", user_prompt, history_context, acceptance_tail);

    assert!(result.contains("load skills: cflx-apply"));
    assert!(result.contains("Apply change id: my-change"));
    assert!(result.contains(APPLY_SYSTEM_PROMPT));
}

#[test]
fn test_build_apply_prompt_with_acceptance_tail() {
    let user_prompt = "Focus on implementation.";
    let history_context = "<last_apply attempt=\"1\">\nstatus: failed\n</last_apply>";
    let acceptance_tail =
        "<last_acceptance_output>\nTest failure detected\n</last_acceptance_output>";
    let result = build_apply_prompt("my-change", user_prompt, history_context, acceptance_tail);

    // Check all parts are present
    assert!(result.contains("Focus on implementation."));
    assert!(result.contains("<last_acceptance_output>"));
    assert!(result.contains("Test failure detected"));
    assert!(result.contains("<last_apply attempt=\"1\">"));

    // Check order: user_prompt, then system, then acceptance_tail, then history
    let user_pos = result.find("Focus on implementation.").unwrap();
    let acceptance_pos = result.find("<last_acceptance_output>").unwrap();
    let history_pos = result.find("<last_apply attempt=\"1\">").unwrap();

    assert!(
        user_pos < acceptance_pos,
        "User prompt should come before acceptance tail"
    );
    assert!(
        acceptance_pos < history_pos,
        "Acceptance tail should come before history"
    );
}

#[test]
fn test_build_apply_prompt_with_acceptance_tail_priority() {
    use super::build_last_acceptance_output_context;

    // Test stdout priority
    let stdout_tail = Some("stdout content");
    let stderr_tail = Some("stderr content");
    let context = build_last_acceptance_output_context(stdout_tail, stderr_tail);
    assert!(context.contains("stdout content"));
    assert!(context.contains("stderr content"));

    // Test stderr fallback when stdout is empty
    let context = build_last_acceptance_output_context(None, stderr_tail);
    assert!(context.contains("stderr content"));
    assert!(!context.contains("stdout"));

    // Test both empty
    let context = build_last_acceptance_output_context(None, None);
    assert!(context.is_empty());
}

#[test]
fn test_apply_system_prompt_content() {
    assert_eq!(APPLY_SYSTEM_PROMPT, "");
}

#[test]
fn test_build_archive_prompt_with_all_parts() {
    let user_prompt = "Please archive this change";
    let history_context = "<last_archive attempt=\"1\">\nstatus: failed\n</last_archive>";
    let result = build_archive_prompt("my-change", user_prompt, history_context);

    assert!(result.contains("load skills: cflx-archive"));
    assert!(result.contains("Archive change id: my-change"));
    assert!(result.contains("Please archive this change"));
    assert!(result.contains("<last_archive attempt=\"1\">"));
    assert!(result.contains("status: failed"));
}

#[test]
fn test_build_archive_prompt_with_empty_user_prompt() {
    let user_prompt = "";
    let history_context = "<last_archive attempt=\"1\">\nstatus: failed\n</last_archive>";
    let result = build_archive_prompt("my-change", user_prompt, history_context);

    // Should only contain history
    assert!(result.contains("<last_archive attempt=\"1\">"));
    assert!(!result.contains("\n\n\n")); // No triple newlines
}

#[test]
fn test_build_archive_prompt_with_empty_history() {
    let user_prompt = "Please archive this change";
    let history_context = "";
    let result = build_archive_prompt("my-change", user_prompt, history_context);

    assert!(result.contains("load skills: cflx-archive"));
    assert!(result.contains("Archive change id: my-change"));
    assert!(result.contains("Please archive this change"));
}

#[test]
fn test_build_archive_prompt_both_empty() {
    let user_prompt = "";
    let history_context = "";
    let result = build_archive_prompt("my-change", user_prompt, history_context);

    assert!(result.contains("load skills: cflx-archive"));
    assert!(result.contains("Archive change id: my-change"));
}

//! Dedicated upstream repair entrypoint.
//!
//! This reuses the existing bounded `resolve_command` runner, command queue
//! policy, streaming, and retry budget, but deliberately does **not** reuse the
//! existing merge/conflict success predicates: those cannot establish semantic
//! repair success. Convergence is decided by
//! [`crate::upstream::coordinator::UpstreamCoordinator`] from repository state
//! after every attempt.

use std::path::PathBuf;

use async_trait::async_trait;
use tracing::info;

use super::ports::{
    PortResult, RepairAttemptResult, RepairCause, RepairRequest, UpstreamPortError,
    UpstreamRepairAgent,
};
use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
use crate::config::OrchestratorConfig;

/// Operation identifier passed to the `cflx-resolve` skill's upstream mode.
pub const UPSTREAM_RESOLVE_OPERATION: &str = "upstream-integration";

fn cause_label(cause: RepairCause) -> &'static str {
    match cause {
        RepairCause::TextualConflict => "textual conflict",
        RepairCause::SemanticVerification => "failed verification command",
        RepairCause::PushRepository => "repository state blocking the cumulative-base push",
    }
}

/// Build the variable context for an upstream repair invocation.
///
/// Fixed guidance (including the prohibition on amend/rebase/reset/push) lives
/// in the `cflx-resolve` skill's upstream-integration mode; this prompt supplies
/// only the invocation-specific facts plus a restatement of the hard bounds so
/// the agent cannot miss them when the skill is unavailable.
pub fn build_upstream_repair_prompt(resolve_skill: &str, request: &RepairRequest) -> String {
    let mut prompt = format!(
        "{}\n\n\
         Operation: {}\n\n\
         Cause: {}\n\
         Selected remote: {}\n\
         Cumulative base branch: {}\n\
         Local cumulative revision before integration: {}\n",
        crate::agent::prompt::skill_prelude(resolve_skill),
        UPSTREAM_RESOLVE_OPERATION,
        cause_label(request.cause),
        request.remote,
        request.branch,
        request.local_revision_before,
    );

    if !request.fetched_sha.is_empty() {
        prompt.push_str(&format!("Fetched remote SHA: {}\n", request.fetched_sha));
    }
    if !request.conflict_files.is_empty() {
        prompt.push_str(&format!(
            "Unmerged files: {}\n",
            request.conflict_files.join(", ")
        ));
    }
    prompt.push_str(&format!(
        "\nCurrent repository status (porcelain v2):\n{}\n",
        request.status
    ));

    match request.cause {
        RepairCause::SemanticVerification => {
            prompt.push_str(&format!(
                "\nComplete verification command: {}\n\nVerification failure output:\n{}\n",
                request.verify_command, request.verify_output_tail
            ));
        }
        RepairCause::PushRepository => {
            prompt.push_str(&format!(
                "\nPush diagnostics (sanitized):\n{}\n",
                request.push_diagnostics
            ));
        }
        RepairCause::TextualConflict => {}
    }

    prompt.push_str(
        "\nPreserve both the accepted local cumulative intent and the upstream intent.\n\
         Hard bounds: do NOT amend, rebase, reset, cherry-pick over, force-push, or otherwise \
         rewrite cumulative history; do NOT run `git push`; do NOT alter credentials or bypass \
         hooks. Conflux owns retries, verification, and the push, and validates repository state \
         after your attempt — narrative claims of success are ignored.\n",
    );

    prompt
}

/// Bounded `resolve_command` adapter for upstream repair.
pub struct ResolveCommandRepairAgent {
    config: OrchestratorConfig,
    repo_root: PathBuf,
    max_attempts: u32,
    shared_stagger_state: SharedStaggerState,
}

impl ResolveCommandRepairAgent {
    pub fn new(
        config: OrchestratorConfig,
        repo_root: impl Into<PathBuf>,
        max_attempts: u32,
        shared_stagger_state: SharedStaggerState,
    ) -> Self {
        Self {
            config,
            repo_root: repo_root.into(),
            max_attempts,
            shared_stagger_state,
        }
    }
}

#[async_trait]
impl UpstreamRepairAgent for ResolveCommandRepairAgent {
    fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    async fn repair(&self, request: &RepairRequest) -> PortResult<RepairAttemptResult> {
        let prompt = crate::agent::append_optional_prompt(
            build_upstream_repair_prompt(self.config.get_resolve_skill(), request),
            self.config.get_resolve_append_prompt(),
        );
        let template = self
            .config
            .get_resolve_command()
            .map_err(|e| UpstreamPortError::new("resolve_command", e.to_string()))?;
        let command = OrchestratorConfig::expand_prompt(template, &prompt);

        info!(
            cause = cause_label(request.cause),
            remote = %request.remote,
            branch = %request.branch,
            "Invoking bounded resolve_command for upstream repair"
        );

        let runner = AiCommandRunner::from_orchestrator_config(
            &self.config,
            self.shared_stagger_state.clone(),
        );
        let (mut child, mut rx) = runner
            .execute_streaming_with_retry(&command, Some(&self.repo_root), Some("resolve"), None)
            .await
            .map_err(|e| UpstreamPortError::new("resolve_command", e.to_string()))?;

        // Drain output so the child is not blocked on a full pipe. The content is
        // intentionally not used for routing.
        while rx.recv().await.is_some() {}

        let status = child
            .wait()
            .await
            .map_err(|e| UpstreamPortError::new("resolve_command", e.to_string()))?;

        Ok(RepairAttemptResult {
            command_success: status.success(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(cause: RepairCause) -> RepairRequest {
        RepairRequest {
            cause,
            remote: "origin".into(),
            branch: "main".into(),
            local_revision_before: "aaa".into(),
            fetched_sha: "bbb".into(),
            conflict_files: vec!["src/lib.rs".into()],
            status: "u UU N... src/lib.rs".into(),
            verify_command: "cargo test".into(),
            verify_output_tail: "test failed".into(),
            push_diagnostics: "non-force push blocked".into(),
        }
    }

    #[test]
    fn upstream_integration_prompt_identifies_upstream_mode_and_revisions() {
        let prompt =
            build_upstream_repair_prompt("cflx-resolve", &request(RepairCause::TextualConflict));
        assert!(prompt.contains(UPSTREAM_RESOLVE_OPERATION));
        assert!(prompt.contains("Selected remote: origin"));
        assert!(prompt.contains("Cumulative base branch: main"));
        assert!(prompt.contains("Local cumulative revision before integration: aaa"));
        assert!(prompt.contains("Fetched remote SHA: bbb"));
        assert!(prompt.contains("Unmerged files: src/lib.rs"));
        assert!(prompt.contains("textual conflict"));
    }

    #[test]
    fn upstream_integration_prompt_carries_verification_context_when_semantic() {
        let prompt = build_upstream_repair_prompt(
            "cflx-resolve",
            &request(RepairCause::SemanticVerification),
        );
        assert!(prompt.contains("Complete verification command: cargo test"));
        assert!(prompt.contains("test failed"));
    }

    #[test]
    fn upstream_integration_prompt_omits_verification_context_when_textual() {
        let prompt =
            build_upstream_repair_prompt("cflx-resolve", &request(RepairCause::TextualConflict));
        assert!(!prompt.contains("Complete verification command"));
    }

    #[test]
    fn upstream_integration_prompt_forbids_history_rewriting_and_push() {
        for cause in [
            RepairCause::TextualConflict,
            RepairCause::SemanticVerification,
            RepairCause::PushRepository,
        ] {
            let prompt = build_upstream_repair_prompt("cflx-resolve", &request(cause));
            assert!(prompt.contains("do NOT amend, rebase, reset"));
            assert!(prompt.contains("do NOT run `git push`"));
        }
    }

    #[test]
    fn upstream_integration_prompt_carries_sanitized_push_diagnostics() {
        let prompt =
            build_upstream_repair_prompt("cflx-resolve", &request(RepairCause::PushRepository));
        assert!(prompt.contains("Push diagnostics (sanitized)"));
        assert!(prompt.contains("non-force push blocked"));
    }
}

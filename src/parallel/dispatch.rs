//! Change dispatch logic for parallel execution.
//!
//! This module handles spawning individual change execution tasks into worktrees:
//! - Pre-flight checks (stopped changes, duplicate dispatch prevention)
//! - Workspace acquisition (semaphore-gated)
//! - Apply + Acceptance + Archive pipeline execution
//! - Per-change cancellation monitoring

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::agent::AgentRunner;
use crate::error::{OrchestratorError, Result};
use crate::events::LogEntry;
use crate::execution::state::{detect_workspace_state, is_merged_to_base, WorkspaceState};
use crate::orchestration::acceptance::{
    decide_acceptance_blocker, decide_acceptance_retry, normalize_findings,
    semantic_progress_fingerprint, AcceptanceBlockerDecision, AcceptanceProtocolDriver,
    AcceptanceRetryDecision, MissingVerdictRetryStep, MAX_ACCEPTANCE_RETRY_CYCLES,
};
use crate::orchestration::{
    execute_rejection_flow, handle_blocked_from_rejecting, handle_resume_apply_from_rejecting,
    run_rejection_review, RejectionReviewVerdict,
};
use crate::task_parser;
use crate::vcs::WorkspaceStatus;

use super::acceptance_state::{
    parse_blocked_marker, AcceptanceRetryContext, BlockedMarker, BlockedMarkerOrigin,
};
use super::cleanup::WorkspaceCleanupGuard;
use super::events::send_event;
use super::executor::{
    execute_acceptance_in_workspace, execute_apply_in_workspace,
    execute_archive_finalization_in_workspace, execute_archive_in_workspace,
};
use super::types::WorkspaceResult;
use super::workspace;
use super::ParallelEvent;
use super::ParallelExecutor;

/// Record a stalled observation directly in the reducer that owns dispatch
/// suppression.
///
/// The event is also published on the event channel for frontends, but only
/// some frontends feed it back into the reducer. Applying it here is what makes
/// the in-memory hold — the sole replacement for the removed out-of-worktree
/// stall record — effective in every mode, including headless CLI runs.
async fn record_stall_in_shared_state(
    shared_orchestrator_state: &Option<
        Arc<tokio::sync::RwLock<crate::orchestration::state::OrchestratorState>>,
    >,
    event: &crate::events::ExecutionEvent,
) {
    if let Some(shared) = shared_orchestrator_state {
        shared.write().await.apply_execution_event(event);
    }
}

fn stalled_blocker_from_marker(marker: &BlockedMarker) -> crate::events::StalledBlocker {
    crate::events::StalledBlocker {
        category: "acceptance_marker".to_string(),
        phase: marker.phase.clone(),
        gate: marker.reason.clone(),
        error_summary: marker.reason.clone(),
        evidence: marker.evidence.clone(),
        // A legacy marker carries no verifiable unblock condition and no owner,
        // so it can never be promoted to an external `blocked` wait; it keeps
        // its conservative stalled handling.
        unblock_condition: None,
        prerequisite_owner: None,
        next_action: marker.next_action.clone(),
        resumable: marker.resumable,
        worktree_preserved: marker.worktree_preserved,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        archived_dirty_repair_candidate_from_unmerged_workspace, decide_resume_action,
        resume_cycle_flags, should_run_apply, ResumeAction,
    };
    use crate::execution::state::WorkspaceState;
    use crate::parallel::acceptance_state::{
        AcceptanceRetryContext, BlockedMarker, BlockedMarkerOrigin,
    };
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    fn init_git_workspace(path: &std::path::Path) {
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(path)
            .output()
            .unwrap();
        std::fs::write(path.join("README.md"), "resume test").unwrap();
        Command::new("git")
            .args(["add", "README.md"])
            .current_dir(path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(path)
            .output()
            .unwrap();
    }

    #[test]
    fn resumed_marker_restores_structured_stalled_metadata() {
        let marker = BlockedMarker {
            origin: BlockedMarkerOrigin::Acceptance,
            reason: "acceptance_gated".to_string(),
            phase: "acceptance".to_string(),
            evidence: vec!["verification output".to_string()],
            finding_identities: vec!["finding-a".to_string()],
            retry_count: 2,
            semantic_fingerprint: Some("finding-a".to_string()),
            semantic_progress: "no_semantic_progress".to_string(),
            external_blockers: vec!["verification unavailable".to_string()],
            resumable: false,
            next_action: "inspect evidence".to_string(),
            worktree_preserved: true,
        };

        let blocker = super::stalled_blocker_from_marker(&marker);

        assert_eq!(blocker.phase, "acceptance");
        assert_eq!(blocker.gate, "acceptance_gated");
        assert_eq!(blocker.evidence, ["verification output"]);
        assert!(!blocker.resumable);
        assert_eq!(blocker.next_action, "inspect evidence");
    }

    #[test]
    fn acceptance_retry_context_defaults_to_a_fresh_sequence() {
        // A restarted dispatch starts from the default context, so the next
        // acceptance failure is judged as the first failure of a new sequence
        // rather than resuming a reconstructed baseline.
        let fresh = AcceptanceRetryContext::default();

        assert!(fresh.previous_identities().is_empty());
        assert_eq!(fresh.previous_fingerprint(), None);
        assert_eq!(fresh.cycle_count, 0);
        assert!(matches!(
            crate::orchestration::acceptance::decide_acceptance_retry(
                fresh.previous_identities(),
                fresh.previous_fingerprint(),
                &crate::orchestration::acceptance::normalize_findings(&[
                    "src/lib.rs:1 missing regression coverage"
                        .to_string()
                        .into(),
                ]),
                "fingerprint",
                1,
            ),
            crate::orchestration::acceptance::AcceptanceRetryDecision::Retry {
                reason: "first_acceptance_failure"
            }
        ));
    }

    #[test]
    fn acceptance_retry_context_stalls_only_on_repeated_in_run_findings() {
        let findings = crate::orchestration::acceptance::normalize_findings(&[
            "src/lib.rs:1 missing regression coverage"
                .to_string()
                .into(),
        ]);
        let previous = AcceptanceRetryContext {
            finding_identities: findings
                .iter()
                .map(|finding| finding.identity.clone())
                .collect(),
            semantic_fingerprint: Some("fingerprint".to_string()),
            cycle_count: 1,
            ..AcceptanceRetryContext::default()
        };

        assert!(matches!(
            crate::orchestration::acceptance::decide_acceptance_retry(
                previous.previous_identities(),
                previous.previous_fingerprint(),
                &findings,
                "fingerprint",
                2,
            ),
            crate::orchestration::acceptance::AcceptanceRetryDecision::Stall {
                reason: "repeated_acceptance_findings",
                ..
            }
        ));
    }

    #[test]
    fn decide_resume_action_routes_applied_to_acceptance_without_state_file() {
        let tmp = TempDir::new().unwrap();
        init_git_workspace(tmp.path());
        let change_dir = tmp.path().join("openspec/changes/change-incomplete");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(
            change_dir.join("proposal.md"),
            "---\nchange_type: implementation\n---\n# Change\n",
        )
        .unwrap();
        fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] done\n",
        )
        .unwrap();

        let action =
            decide_resume_action("change-incomplete", tmp.path(), &WorkspaceState::Applied);
        assert_eq!(action, ResumeAction::Acceptance);
    }

    #[test]
    fn decide_resume_action_routes_applied_to_acceptance_even_with_external_durable_state() {
        let tmp = TempDir::new().unwrap();
        init_git_workspace(tmp.path());
        let change_dir = tmp.path().join("openspec/changes/change-complete");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(
            change_dir.join("proposal.md"),
            "---\nchange_type: implementation\n---\n# Change\n",
        )
        .unwrap();
        fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] done\n",
        )
        .unwrap();

        // A leftover checkpoint from an older Conflux version must not be
        // consulted: routing is decided from workspace-local evidence only.
        let stale_checkpoint = tmp.path().join(".cflx/acceptance-state.json");
        fs::create_dir_all(stale_checkpoint.parent().unwrap()).unwrap();
        fs::write(&stale_checkpoint, "{\"state\":\"passed\"}\n").unwrap();

        let action = decide_resume_action("change-complete", tmp.path(), &WorkspaceState::Applied);
        assert_eq!(action, ResumeAction::Acceptance);
        assert_eq!(
            fs::read_to_string(&stale_checkpoint).unwrap(),
            "{\"state\":\"passed\"}\n",
            "resume routing must neither consume nor rewrite generated acceptance state"
        );
    }

    #[test]
    fn decide_resume_action_routes_applied_to_apply_when_implementation_tasks_incomplete() {
        let tmp = TempDir::new().unwrap();
        init_git_workspace(tmp.path());
        let change_dir = tmp.path().join("openspec/changes/change-incomplete");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(
            change_dir.join("proposal.md"),
            "---\nchange_type: implementation\n---\n# Change\n",
        )
        .unwrap();
        fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] done\n- [ ] todo\n\n## Future Work\n- 補足メモのみ\n",
        )
        .unwrap();

        let action =
            decide_resume_action("change-incomplete", tmp.path(), &WorkspaceState::Applied);
        assert_eq!(action, ResumeAction::Apply);
    }

    #[test]
    fn decide_resume_action_routes_applied_to_apply_when_follow_up_tasks_incomplete() {
        let tmp = TempDir::new().unwrap();
        init_git_workspace(tmp.path());
        let change_dir = tmp.path().join("openspec/changes/change-follow-up");
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(
            change_dir.join("proposal.md"),
            "---\nchange_type: implementation\n---\n# Change\n",
        )
        .unwrap();
        fs::write(
            change_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] done\n\n## Acceptance #1 Failure Follow-up\n- [ ] fix regression\n",
        )
        .unwrap();

        let action = decide_resume_action("change-follow-up", tmp.path(), &WorkspaceState::Applied);
        assert_eq!(action, ResumeAction::Apply);
    }

    #[test]
    fn parallel_latest_fail_reconciles_completed_findings_for_apply_resume() {
        let tmp = TempDir::new().unwrap();
        init_git_workspace(tmp.path());
        let change_id = "change-follow-up";
        let change_dir = tmp.path().join("openspec/changes").join(change_id);
        fs::create_dir_all(&change_dir).unwrap();
        fs::write(
            change_dir.join("proposal.md"),
            "---\nchange_type: implementation\n---\n# Change\n",
        )
        .unwrap();
        let tasks_path = change_dir.join("tasks.md");
        fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- attempt: 1\n- [x] [SAME_FINDING] fixed wording\n- [x] [RETIRED_FINDING] fixed and not reported again\n- [x] [DIFFERENT_FINDING] unrelated completed defect\n",
        )
        .unwrap();

        crate::task_parser::replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            2,
            &[
                "[SAME_FINDING] defect still present with new evidence"
                    .to_string()
                    .into(),
                "[NEW_FINDING] distinct newly reported defect"
                    .to_string()
                    .into(),
            ],
        )
        .unwrap();

        let content = fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("- [ ] [SAME_FINDING] defect still present with new evidence"));
        assert!(content.contains("- [ ] [NEW_FINDING] distinct newly reported defect"));
        assert!(!content.contains("RETIRED_FINDING"));
        assert!(!content.contains("DIFFERENT_FINDING"));
        assert_eq!(
            crate::task_parser::parse_file(&tasks_path, None).unwrap(),
            crate::task_parser::TaskProgress::with_counts(1, 3)
        );
        assert_eq!(
            decide_resume_action(change_id, tmp.path(), &WorkspaceState::Applied),
            ResumeAction::Apply
        );
    }

    #[test]
    fn decide_resume_action_keeps_archived_as_terminal() {
        let tmp = TempDir::new().unwrap();
        let action = decide_resume_action("change-archived", tmp.path(), &WorkspaceState::Archived);
        assert_eq!(action, ResumeAction::Terminal);
    }

    #[test]
    fn should_run_apply_consumes_skip_flag_after_first_cycle() {
        let mut skip_apply_once = true;

        assert!(!should_run_apply(&mut skip_apply_once));
        assert!(!skip_apply_once);
        assert!(should_run_apply(&mut skip_apply_once));
    }

    #[test]
    fn resume_cycle_flags_for_acceptance_resume_skip_only_apply_once() {
        let (skip_apply_once, skip_acceptance_once) = resume_cycle_flags(ResumeAction::Acceptance);

        assert!(skip_apply_once);
        assert!(!skip_acceptance_once);
    }

    #[test]
    fn resume_cycle_flags_for_archive_resume_skip_apply_and_acceptance_once() {
        let (skip_apply_once, skip_acceptance_once) = resume_cycle_flags(ResumeAction::Archive);

        assert!(skip_apply_once);
        assert!(skip_acceptance_once);
    }

    #[test]
    fn archived_dirty_repair_candidate_reads_archived_tasks_without_active_change_dir() {
        let tmp = TempDir::new().unwrap();
        init_git_workspace(tmp.path());
        let archive_dir = tmp
            .path()
            .join("openspec/changes/archive/2026-05-08-fix-dependency-target-handling");
        fs::create_dir_all(&archive_dir).unwrap();
        fs::write(
            archive_dir.join("proposal.md"),
            "---\nchange_type: implementation\ndependencies: []\n---\n# Change\n",
        )
        .unwrap();
        fs::write(
            archive_dir.join("tasks.md"),
            "## Implementation Tasks\n- [x] done\n",
        )
        .unwrap();
        fs::write(archive_dir.join("report.md"), "# Report\n").unwrap();

        let candidate = archived_dirty_repair_candidate_from_unmerged_workspace(
            "fix-dependency-target-handling",
            tmp.path(),
        )
        .expect("archived dirty candidate should be reconstructed from archive entry");

        assert_eq!(candidate.id, "fix-dependency-target-handling");
        assert_eq!(candidate.completed_tasks, 1);
        assert_eq!(candidate.total_tasks, 1);
        assert!(candidate.metadata.change_type.as_deref() == Some("implementation"));
    }

    // --- Actionable finding contract: parallel wiring ---

    fn structured_finding(
        id: &str,
        implementation: &str,
        verification: &str,
    ) -> crate::acceptance::AcceptanceFinding {
        crate::acceptance::AcceptanceFinding::structured(crate::acceptance::RepositoryFinding {
            id: id.to_string(),
            severity: crate::acceptance::FindingSeverity::Minor,
            summary: "Challenge and proof leakage is not tested by value".to_string(),
            evidence: vec!["relay exposes counts but not issued values".to_string()],
            required_changes: vec![crate::acceptance::FindingFileExpectation {
                file: implementation.to_string(),
                description: "Expose issued challenge and presented proof values".to_string(),
            }],
            verification: vec![crate::acceptance::FindingFileExpectation {
                file: verification.to_string(),
                description: "Assert recorded values are absent from audit output".to_string(),
            }],
        })
    }

    fn secret_value_finding() -> crate::acceptance::AcceptanceFinding {
        structured_finding(
            "acceptance-secret-value-scan",
            "tests/support/relay.ts",
            "runtime/recovery.integration.test.ts",
        )
    }

    #[test]
    fn parallel_retry_checkpoint_preserves_the_payload_for_the_next_apply() {
        // Mirrors the ordering dispatch uses: the executor records the complete
        // payload, then dispatch records comparison state. The payload must
        // survive, so the next apply prompt still carries the required change.
        let mut shared_history = crate::history::AcceptanceHistory::new();
        shared_history.set_follow_up_findings("change-a", 1, vec![secret_value_finding()]);

        let retry = AcceptanceRetryContext {
            finding_identities: vec!["repository|id|acceptance-secret-value-scan".to_string()],
            semantic_fingerprint: Some("fingerprint".to_string()),
            cycle_count: 1,
            findings: vec![secret_value_finding()],
            ..AcceptanceRetryContext::default()
        };
        shared_history.set_retry_checkpoint(
            "change-a",
            retry.cycle_count,
            retry.finding_identities.clone(),
            retry.semantic_fingerprint.clone(),
        );

        let mut agent =
            crate::agent::AgentRunner::new(crate::config::OrchestratorConfig::default());
        agent.seed_acceptance_history(shared_history);
        let prompt_context = agent.get_acceptance_tail_context_for_apply("change-a");

        assert!(
            prompt_context.contains("Expose issued challenge and presented proof values"),
            "{prompt_context}"
        );
        assert!(
            prompt_context.contains("runtime/recovery.integration.test.ts"),
            "{prompt_context}"
        );
        assert!(
            !prompt_context.contains("repository|id|acceptance-secret-value-scan"),
            "{prompt_context}"
        );
    }

    #[test]
    fn parallel_repeated_structured_id_stops_before_a_second_repair_apply() {
        let mut retry = AcceptanceRetryContext::default();
        let findings = [secret_value_finding()];
        let normalized = crate::orchestration::acceptance::normalize_findings(&findings);

        let crate::orchestration::acceptance::FindingRepairDecision::Repair { identities } =
            retry.repair_ledger.observe_fail(&normalized)
        else {
            panic!("first observation must allow one repair");
        };
        retry.repair_ledger.record_repair_dispatched(&identities);

        // Unrelated semantic progress plus rewritten prose for the same defect.
        let restated = crate::acceptance::AcceptanceFinding::structured(
            crate::acceptance::RepositoryFinding {
                id: "acceptance-secret-value-scan".to_string(),
                severity: crate::acceptance::FindingSeverity::Major,
                summary: "Rewritten summary".to_string(),
                evidence: vec!["different evidence at src/other.rs:99".to_string()],
                required_changes: vec![crate::acceptance::FindingFileExpectation {
                    file: "src/other.rs".to_string(),
                    description: "different change".to_string(),
                }],
                verification: vec![crate::acceptance::FindingFileExpectation {
                    file: "tests/other.rs".to_string(),
                    description: "different proof".to_string(),
                }],
            },
        );
        let crate::orchestration::acceptance::FindingRepairDecision::Stop {
            reason,
            repeated_identities,
        } = retry.repair_ledger.observe_fail(
            &crate::orchestration::acceptance::normalize_findings(std::slice::from_ref(&restated)),
        )
        else {
            panic!("a repeated ID must stop automatic repair in parallel too");
        };

        assert_eq!(
            reason,
            crate::orchestration::acceptance::REPEATED_FINDING_REASON
        );
        let stop = crate::orchestration::acceptance::repeated_finding_stop(
            "change-a",
            std::slice::from_ref(&restated),
            &retry.repair_ledger,
            repeated_identities,
            Some("fail-rev"),
            Some("apply-rev"),
            &["src/other.rs".to_string(), "tests/other.rs".to_string()],
            &[],
        );
        let error = stop.summary();
        assert!(error.contains("repeated_acceptance_finding"), "{error}");
        assert!(error.contains("acceptance-secret-value-scan"), "{error}");
        assert!(error.contains("\"resumable\":true"), "{error}");
    }

    #[test]
    fn parallel_calibration_only_repair_holds_before_acceptance() {
        let retry = AcceptanceRetryContext {
            findings: vec![secret_value_finding()],
            fail_revision: Some("fail-rev".to_string()),
            ..AcceptanceRetryContext::default()
        };

        let crate::orchestration::acceptance::RepairGateDecision::Stop(stop) =
            crate::orchestration::acceptance::decide_repair_gate(
                "change-a",
                &retry.findings,
                &retry.repair_ledger,
                retry.fail_revision.as_deref(),
                Some("apply-rev"),
                &["tests/calibration.test.ts".to_string()],
                &["adjusted calibration threshold".to_string()],
            )
        else {
            panic!("calibration-only repair must hold before acceptance");
        };

        assert_eq!(
            stop.reason,
            crate::orchestration::acceptance::REMEDIATION_MISMATCH_REASON
        );
        assert_eq!(stop.coverage.unrelated_files, ["tests/calibration.test.ts"]);
        assert_eq!(
            stop.coverage.uncovered(),
            [
                "acceptance-secret-value-scan: required_changes tests/support/relay.ts",
                "acceptance-secret-value-scan: verification runtime/recovery.integration.test.ts",
            ]
        );
    }

    #[test]
    fn parallel_new_finding_id_receives_its_own_repair_opportunity() {
        let mut retry = AcceptanceRetryContext::default();
        let first = crate::orchestration::acceptance::normalize_findings(&[secret_value_finding()]);
        let crate::orchestration::acceptance::FindingRepairDecision::Repair { identities } =
            retry.repair_ledger.observe_fail(&first)
        else {
            panic!("first observation must allow one repair");
        };
        retry.repair_ledger.record_repair_dispatched(&identities);

        let second = crate::orchestration::acceptance::normalize_findings(&[structured_finding(
            "acceptance-new-defect",
            "src/new.rs",
            "tests/new.rs",
        )]);
        assert!(matches!(
            retry.repair_ledger.observe_fail(&second),
            crate::orchestration::acceptance::FindingRepairDecision::Repair { .. }
        ));
    }

    #[test]
    fn parallel_malformed_structured_finding_is_a_bounded_protocol_retry() {
        // The malformed-finding contract must not consume the missing-verdict or
        // bare-blocker budgets, and must never dispatch repair work.
        let mut driver = crate::orchestration::acceptance::AcceptanceProtocolDriver::default();
        let rejection = crate::acceptance::FindingRejection::MissingId;

        for attempt in 1..=crate::orchestration::acceptance::MAX_ACCEPTANCE_PROTOCOL_RETRIES {
            let step = driver.observe_malformed_finding(&rejection);
            let crate::orchestration::acceptance::MissingVerdictRetryStep::Retry { retry, .. } =
                step
            else {
                panic!("retry budget must remain at attempt {attempt}");
            };
            assert_eq!(
                retry.kind,
                crate::orchestration::acceptance::AcceptanceProtocolError::MalformedFinding
            );
        }
        assert!(matches!(
            driver.observe_malformed_finding(&rejection),
            crate::orchestration::acceptance::MissingVerdictRetryStep::Exhausted { .. }
        ));
        assert_eq!(driver.consecutive_missing_verdicts(), 0);
        assert_eq!(driver.consecutive_bare_blockers(), 0);

        // Any canonical verdict resets the malformed-finding sequence too.
        driver.observe_canonical_verdict();
        assert_eq!(driver.consecutive_malformed_findings(), 0);
    }

    /// An explicitly retried dispatch must start with the automatic per-finding
    /// repair budget released, and the release must keep occurrence evidence.
    ///
    /// Parallel builds one retry context per dispatch, so the budget is empty by
    /// construction; the shared release API is still what production calls, so
    /// both modes grant exactly one more repair opportunity per explicit retry.
    #[test]
    fn parallel_explicit_retry_starts_with_a_released_repair_budget() {
        let mut retry = AcceptanceRetryContext::default();
        let findings =
            crate::orchestration::acceptance::normalize_findings(&[secret_value_finding()]);
        let crate::orchestration::acceptance::FindingRepairDecision::Repair { identities } =
            retry.repair_ledger.observe_fail(&findings)
        else {
            panic!("first observation must allow one repair");
        };
        retry.repair_ledger.record_repair_dispatched(&identities);
        assert!(retry
            .repair_ledger
            .has_consumed_repair("repository|id|acceptance-secret-value-scan"));

        retry.repair_ledger.reset_for_explicit_retry();

        assert!(
            !retry
                .repair_ledger
                .has_consumed_repair("repository|id|acceptance-secret-value-scan"),
            "an explicit retry releases the automatic repair budget"
        );
        assert_eq!(
            retry
                .repair_ledger
                .occurrences("repository|id|acceptance-secret-value-scan"),
            1,
            "occurrence evidence must remain inspectable after the retry"
        );
        assert!(matches!(
            retry.repair_ledger.observe_fail(&findings),
            crate::orchestration::acceptance::FindingRepairDecision::Repair { .. }
        ));

        // The production explicit-retry branch is what calls the release.
        let dispatch_source = include_str!("dispatch.rs");
        assert!(
            dispatch_source.contains("reset_for_explicit_retry"),
            "the explicit-retry branch must call the shared release API"
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResumeAction {
    Terminal,
    Apply,
    Acceptance,
    Archive,
    Blocked,
    Rejecting,
}

pub(super) fn decide_resume_action(
    change_id: &str,
    workspace_path: &Path,
    state: &WorkspaceState,
) -> ResumeAction {
    match state {
        WorkspaceState::Merged => ResumeAction::Terminal,
        WorkspaceState::Archived => ResumeAction::Terminal,
        WorkspaceState::Archiving => ResumeAction::Archive,
        WorkspaceState::Applied => {
            if should_route_to_apply_for_incomplete_implementation_tasks(change_id, workspace_path)
            {
                info!(
                    "Resume route for '{}' forcing apply because implementation tasks are incomplete",
                    change_id
                );
                return ResumeAction::Apply;
            }

            info!(
                "Resume route forcing acceptance for '{}' in Applied state based on workspace-local evidence",
                change_id
            );
            ResumeAction::Acceptance
        }
        WorkspaceState::Blocked => ResumeAction::Blocked,
        WorkspaceState::Rejecting => ResumeAction::Rejecting,
        WorkspaceState::Created | WorkspaceState::Applying { .. } => ResumeAction::Apply,
    }
}

fn should_route_to_apply_for_incomplete_implementation_tasks(
    change_id: &str,
    workspace_path: &Path,
) -> bool {
    if !is_implementation_change(change_id, workspace_path) {
        return false;
    }

    match read_implementation_task_progress(change_id, workspace_path) {
        Ok(Some((completed, total))) => {
            let has_incomplete = completed < total;
            if has_incomplete {
                info!(
                    "Resume routing check for '{}' detected incomplete implementation tasks ({}/{})",
                    change_id, completed, total
                );
            }
            has_incomplete
        }
        Ok(None) => false,
        Err(err) => {
            warn!(
                "Failed to read implementation task progress for '{}' in '{}': {}",
                change_id,
                workspace_path.display(),
                err
            );
            false
        }
    }
}

fn is_implementation_change(change_id: &str, workspace_path: &Path) -> bool {
    let proposal_path = workspace_path
        .join("openspec/changes")
        .join(change_id)
        .join("proposal.md");

    if !proposal_path.exists() {
        return false;
    }

    let metadata = crate::openspec::parse_proposal_metadata_from_file(&proposal_path);
    metadata
        .change_type
        .as_deref()
        .is_some_and(|change_type| change_type.eq_ignore_ascii_case("implementation"))
}

fn read_implementation_task_progress(
    change_id: &str,
    workspace_path: &Path,
) -> Result<Option<(u32, u32)>> {
    let tasks_path = workspace_path
        .join("openspec/changes")
        .join(change_id)
        .join("tasks.md");

    if !tasks_path.exists() {
        return Ok(None);
    }

    let progress = parse_tasks_progress(&tasks_path, change_id)?;

    if progress.total == 0 {
        Ok(None)
    } else {
        Ok(Some((progress.completed, progress.total)))
    }
}

fn parse_tasks_progress(
    tasks_path: &Path,
    change_id: &str,
) -> Result<crate::task_parser::TaskProgress> {
    task_parser::parse_file(tasks_path, Some(change_id)).map_err(|e| {
        OrchestratorError::ConfigLoad(format!(
            "Failed to parse tasks file '{}' for resume routing: {}",
            tasks_path.display(),
            e
        ))
    })
}

pub(super) async fn archived_dirty_repair_candidate_from_workspace(
    change_id: &str,
    workspace_path: &Path,
    base_branch: &str,
) -> Option<crate::openspec::Change> {
    match is_merged_to_base(change_id, workspace_path, base_branch).await {
        Ok(true) => {
            info!(
                change_id = %change_id,
                base_branch = %base_branch,
                workspace_path = %workspace_path.display(),
                "Skipping archived dirty repair candidate because workspace evidence is already merged to base"
            );
            return None;
        }
        Ok(false) => {}
        Err(error) => {
            warn!(
                change_id = %change_id,
                base_branch = %base_branch,
                workspace_path = %workspace_path.display(),
                "Failed to check merged state before archived dirty repair discovery: {}",
                error
            );
            return None;
        }
    }

    archived_dirty_repair_candidate_from_unmerged_workspace(change_id, workspace_path)
}

fn archived_dirty_repair_candidate_from_unmerged_workspace(
    change_id: &str,
    workspace_path: &Path,
) -> Option<crate::openspec::Change> {
    let active_change_dir = workspace_path.join("openspec/changes").join(change_id);
    if active_change_dir.exists() {
        return None;
    }

    let archive_dir = workspace_path.join("openspec/changes/archive");
    let archive_entry = find_archive_entry_path(change_id, &archive_dir)?;
    if !archive_entry.join("proposal.md").exists() {
        return None;
    }

    let tasks_path = archive_entry.join("tasks.md");
    let (completed_tasks, total_tasks) = parse_tasks_progress(&tasks_path, change_id)
        .map(|progress| (progress.completed, progress.total))
        .unwrap_or((0, 0));
    let metadata =
        crate::openspec::parse_proposal_metadata_from_file(&archive_entry.join("proposal.md"));
    let dependencies = metadata.dependencies.clone();

    Some(crate::openspec::Change {
        id: change_id.to_string(),
        completed_tasks,
        total_tasks,
        last_modified: String::new(),
        dependencies,
        metadata,
    })
}

fn find_archive_entry_path(change_id: &str, archive_dir: &Path) -> Option<std::path::PathBuf> {
    if !archive_dir.exists() {
        return None;
    }

    std::fs::read_dir(archive_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .find(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            (name == change_id || name.ends_with(&format!("-{change_id}"))) && entry.path().is_dir()
        })
        .map(|entry| entry.path())
}

fn should_run_apply(skip_apply_once: &mut bool) -> bool {
    if *skip_apply_once {
        *skip_apply_once = false;
        false
    } else {
        true
    }
}

fn resume_cycle_flags(resume_action: ResumeAction) -> (bool, bool) {
    (
        matches!(
            resume_action,
            ResumeAction::Acceptance | ResumeAction::Archive
        ),
        matches!(resume_action, ResumeAction::Archive),
    )
}

impl ParallelExecutor {
    /// Dispatch a single change to a workspace for apply + acceptance + archive.
    ///
    /// This method:
    /// - Checks if the change has been stopped or is already in-flight
    /// - Acquires a semaphore permit (to enforce concurrency limits)
    /// - Creates or resumes a workspace
    /// - Spawns an async task for apply + acceptance + archive pipeline
    ///
    /// The spawned task will:
    /// - Execute apply command
    /// - Execute acceptance test (with retry loop)
    /// - Execute archive command (only if acceptance passes)
    /// - Return WorkspaceResult
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn dispatch_change_to_workspace(
        &mut self,
        change_id: String,
        base_revision: String,
        semaphore: Arc<Semaphore>,
        join_set: &mut JoinSet<WorkspaceResult>,
        in_flight: &mut HashSet<String>,
        cleanup_guard: &mut WorkspaceCleanupGuard,
    ) -> Result<()> {
        if self.is_cancelled() {
            info!(
                "Change '{}' skipped before dispatch because parallel execution is cancelled",
                change_id
            );
            return Ok(());
        }

        // Check if this change has been stopped (single-change stop)
        if let Some(ref queue) = self.dynamic_queue {
            if queue.is_stopped(&change_id).await {
                queue.clear_stopped(&change_id).await;
                info!("Change '{}' stopped before dispatch", change_id);
                send_event(
                    &self.event_tx,
                    ParallelEvent::ChangeDequeued {
                        change_id: change_id.clone(),
                    },
                )
                .await;
                send_event(
                    &self.event_tx,
                    ParallelEvent::Log(LogEntry::info(format!("Change stopped: {}", change_id))),
                )
                .await;
                return Ok(());
            }
        }

        if let Some(shared) = &self.shared_orchestrator_state {
            let terminal_gate = shared.try_read().ok().and_then(|guard| {
                if guard.is_final_terminal_dispatch_stop(&change_id) {
                    Some((
                        "final_terminal",
                        format!(
                            "Change {} is already in a final terminal state; skipping dispatch",
                            change_id
                        ),
                    ))
                } else if guard.is_terminal_error_change(&change_id) {
                    Some((
                        "terminal_error",
                        format!(
                            "Change {} remains error until explicitly retried",
                            change_id
                        ),
                    ))
                } else {
                    None
                }
            });

            if let Some((gate, message)) = terminal_gate {
                info!(
                    change_id = %change_id,
                    gate,
                    "Skipping workspace dispatch because reducer terminal state blocks ordinary dispatch"
                );
                send_event(&self.event_tx, ParallelEvent::Log(LogEntry::info(message))).await;
                return Ok(());
            }
        }

        // Check if already in-flight (avoid duplicate dispatch)
        if in_flight.contains(&change_id) {
            warn!(
                "Change '{}' already in-flight, skipping dispatch",
                change_id
            );
            return Ok(());
        }

        // Acquire semaphore permit, but wake promptly if global cancellation arrives while waiting.
        let permit = if let Some(token) = &self.cancel_token {
            tokio::select! {
                _ = token.cancelled() => {
                    info!("Change '{}' skipped while waiting for parallel slot because execution was cancelled", change_id);
                    return Ok(());
                }
                permit = semaphore.clone().acquire_owned() => permit.map_err(|e| {
                    OrchestratorError::AgentCommand(format!("Failed to acquire semaphore: {}", e))
                })?,
            }
        } else {
            semaphore.clone().acquire_owned().await.map_err(|e| {
                OrchestratorError::AgentCommand(format!("Failed to acquire semaphore: {}", e))
            })?
        };

        let force_recreate = self.force_recreate_worktree.remove(&change_id);
        if force_recreate {
            info!(
                "Dispatching '{}' with forced fresh workspace recreation after dependency resolution",
                change_id
            );
            send_event(
                &self.event_tx,
                ParallelEvent::Log(LogEntry::info(format!(
                    "Dependency resolved: forcing fresh workspace for {}",
                    change_id
                ))),
            )
            .await;
        }

        // Create or reuse workspace; was_resumed=true means an existing workspace was reused.
        let mut force_recreate_set = HashSet::new();
        if force_recreate {
            force_recreate_set.insert(change_id.clone());
        }
        let (workspace_val, was_resumed) = workspace::get_or_create_workspace(
            self.workspace_manager.as_mut(),
            &change_id,
            &base_revision,
            self.no_resume,
            &force_recreate_set,
            &self.event_tx,
        )
        .await?;

        let base_branch = self
            .workspace_manager
            .ensure_original_branch_initialized()
            .await
            .map_err(OrchestratorError::from_vcs_error)?;

        // An Acceptance stall is in-memory reducer state for the lifetime of one
        // process (constitutional law 1: no out-of-worktree durable workflow
        // state). Nothing is reloaded here: the change was kept out of ordinary
        // dispatch by `classify_queued_work`, and once the hold is gone — by
        // explicit retry or by restart — routing is recomputed from the
        // workspace's own git and file evidence below.

        // Track workspace for cleanup
        cleanup_guard.track(workspace_val.name.clone(), workspace_val.path.clone());

        // Add to in-flight set
        in_flight.insert(change_id.clone());

        let shared_orchestrator_state = self.shared_orchestrator_state.clone();
        let explicit_retry = self.explicit_retry;
        let dynamic_queue_for_requeue = self.dynamic_queue.clone();

        // Prepare context for spawned task
        let apply_command = self.apply_command.clone();
        let archive_command = self.archive_command.clone();
        let repo_root = self.repo_root.clone();
        let config = self.config.clone();
        let event_tx = self.event_tx.clone();
        let vcs_backend = self.workspace_manager.backend_type();
        let ai_runner = self.ai_runner.clone();
        let apply_history = self.apply_history.clone();
        let archive_history = self.archive_history.clone();
        let acceptance_history = self.acceptance_history.clone();
        let acceptance_tail_injected = self.acceptance_tail_injected.clone();
        let cancel_token = self.cancel_token.clone();
        let shared_stagger_state = self.shared_stagger_state.clone();
        let dynamic_queue = self.dynamic_queue.clone();
        let workspace = workspace_val;

        // Spawn apply + acceptance + archive task
        join_set.spawn(async move {
            let _permit = permit; // Hold permit until task completes

            // Detect workspace state for resumed workspaces and route accordingly.
            // A new workspace always starts fresh (Created state).
            // A resumed workspace may be in any state; we must not blindly run the full
            // pipeline for terminal states (Archived, Merged) or already-applied states.
            let effective_state = if was_resumed {
                match detect_workspace_state(&change_id, &workspace.path, &base_branch).await {
                    Ok(state) => {
                        let state_label = format!("{:?}", state);
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::Log(
                                    LogEntry::info(format!(
                                        "Resuming existing workspace for {} (detected state: {})",
                                        change_id, state_label
                                    ))
                                    .with_change_id(&change_id),
                                ))
                                .await;
                        }
                        state
                    }
                    Err(e) => {
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some(format!("Failed to detect workspace state: {e}")),
                            rejected: None,
                        };
                    }
                }
            } else {
                WorkspaceState::Created
            };

            // Routing comes from workspace evidence alone. An explicit retry of
            // a previously stalled change lands here with a complete unarchived
            // Apply revision, so `decide_resume_action` selects Acceptance
            // without rerunning Apply — the same route a restart takes.
            let resume_action = if was_resumed {
                decide_resume_action(&change_id, &workspace.path, &effective_state)
            } else {
                ResumeAction::Apply
            };

            if was_resumed && matches!(effective_state, WorkspaceState::Archiving) {
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::ArchiveResumed {
                            change_id: change_id.clone(),
                            reason: Some("archive_commit_incomplete".to_string()),
                            summary: Some(
                                "Resuming archived dirty workspace; archive move is complete and commit finalization is incomplete"
                                    .to_string(),
                            ),
                        })
                        .await;
                }
            }

            if was_resumed {
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::Log(
                            LogEntry::info(format!(
                                "Resume routing for {}: state={:?} -> {:?}",
                                change_id, effective_state, resume_action
                            ))
                            .with_change_id(&change_id),
                        ))
                        .await;
                }
            }

            // Early return for terminal states: Archived and Merged workspaces must not
            // re-enter the apply/acceptance/archive pipeline.  Doing so silently creates
            // duplicate apply commits or masks already-complete work as a fresh start.
            if matches!(resume_action, ResumeAction::Blocked) {
                if let Some(ref tx) = event_tx {
                    if let Ok(Some(marker)) = parse_blocked_marker(&workspace.path, &change_id) {
                        let blocker = stalled_blocker_from_marker(&marker);
                        let event = if matches!(marker.origin, BlockedMarkerOrigin::Acceptance) {
                            ParallelEvent::AcceptanceGated {
                                change_id: change_id.clone(),
                                blocker,
                            }
                        } else {
                            ParallelEvent::ExecutionBlocked {
                                change_id: change_id.clone(),
                                blocker,
                            }
                        };
                        let _ = tx.send(event).await;
                    }
                    let _ = tx
                        .send(ParallelEvent::WorkspaceStatusUpdated {
                            change_id: change_id.clone(),
                            workspace_name: workspace.name.clone(),
                            status: WorkspaceStatus::Blocked,
                        })
                        .await;
                }
                return WorkspaceResult {
                    change_id,
                    workspace_name: workspace.name,
                    final_revision: None,
                    error: None,
                    rejected: None,
                };
            }

            if matches!(resume_action, ResumeAction::Terminal) {
                match &effective_state {
                    WorkspaceState::Merged => {
                    info!(
                        "Change '{}' workspace already merged to base, skipping all processing",
                        change_id
                    );
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::Log(
                                LogEntry::info(format!(
                                    "Change {} skipped: workspace already merged to base branch",
                                    change_id
                                ))
                                .with_change_id(&change_id),
                            ))
                            .await;
                    }
                    // cancel_monitor has not been spawned yet at this point,
                    // so we return without aborting it.
                    return WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: None,
                        error: None,
                        rejected: None,
                    };
                }
                WorkspaceState::Archived => {
                    // The workspace is already past the archive step.  We must hand it
                    // off to merge handling rather than silently returning a no-op result
                    // with final_revision=None (which would cause the change to disappear
                    // from the queue lifecycle and never reach MergeWait).
                    info!(
                        "Change '{}' workspace already archived on resume, handing off to merge",
                        change_id
                    );
                    // Get the current HEAD revision of the worktree — this is the
                    // archive commit that the merge step needs.
                    let resume_revision =
                        crate::vcs::git::commands::get_current_commit(&workspace.path).await;
                    match resume_revision {
                        Ok(rev) => {
                            if let Some(shared) = &shared_orchestrator_state {
                                let mut guard = shared.write().await;
                                guard.apply_execution_event(&ParallelEvent::ChangeArchived(
                                    change_id.clone(),
                                ));
                            }
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::info(format!(
                                            "Change {} resumed: workspace already archived, entering merge handling",
                                            change_id
                                        ))
                                        .with_change_id(&change_id),
                                    ))
                                    .await;
                                // Emit the same ChangeArchived event as the normal archive
                                // success path so that downstream state machines (TUI,
                                // output bridge) treat this resume identically.
                                let _ = tx
                                    .send(ParallelEvent::ChangeArchived(change_id.clone()))
                                    .await;
                            }
                            // cancel_monitor has not been spawned yet at this point,
                            // so we return without aborting it.
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: Some(rev),
                                error: None,
                                rejected: None,
                            };
                        }
                        Err(e) => {
                            // Could not read the revision — treat as a transient error so
                            // the orchestrator can surface it rather than silently dropping
                            // the change from the queue.
                            warn!(
                                "Change '{}' archived on resume but revision read failed: {}",
                                change_id, e
                            );
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: None,
                                error: Some(format!(
                                    "Archived resume: failed to read workspace revision: {}",
                                    e
                                )),
                                rejected: None,
                            };
                        }
                    }
                }
                    _ => {}
                }
            }

            // Create agent for acceptance testing.
            let mut agent =
                AgentRunner::new_with_shared_state(config.clone(), shared_stagger_state.clone());

            // Track apply+acceptance cycles to prevent infinite loops.
            // Retry context lives in memory for this dispatch only: a restarted
            // process re-runs acceptance instead of resuming a prior verdict.
            let mut acceptance_retry = AcceptanceRetryContext::default();
            if explicit_retry {
                // Explicit operator retry: release the automatic per-finding
                // repair budget through the same shared API serial calls, so both
                // modes grant exactly one more repair opportunity per retry.
                // Parallel's ledger is per-dispatch, so this starts from an empty
                // budget by construction; calling the release keeps the two
                // explicit-retry contracts identical instead of implicit.
                acceptance_retry
                    .repair_ledger
                    .reset_for_explicit_retry();
            }
            // Consecutive missing-verdict accounting for this active run only,
            // shared with serial orchestration. It is independent from the
            // configured explicit-CONTINUE budget and is never persisted, so a
            // restart re-runs acceptance from workspace state instead of
            // resuming a protocol-retry sequence.
            let mut protocol = AcceptanceProtocolDriver::default();
            let mut cycle_count = 0u32;
            let mut cumulative_iteration = 0u32; // Track total apply iterations across all cycles

            // Create a per-change cancel token that monitors both global cancel and single-change stop
            let per_change_cancel = CancellationToken::new();

            // Register the kill token for immediate force-kill from TUI/WebUI
            if let Some(ref queue) = dynamic_queue {
                queue
                    .register_kill_token(change_id.clone(), per_change_cancel.clone())
                    .await;
            }

            let monitor_cancel = per_change_cancel.clone();
            let monitor_global = cancel_token.clone();
            let monitor_queue = dynamic_queue.clone();
            let monitor_change_id = change_id.clone();

            // Spawn a background task to monitor both cancellation sources
            let cancel_monitor = tokio::spawn(async move {
                loop {
                    // Check global cancellation
                    if let Some(ref token) = monitor_global {
                        if token.is_cancelled() {
                            monitor_cancel.cancel();
                            break;
                        }
                    }

                    // Check single-change stop
                    if let Some(ref queue) = monitor_queue {
                        if queue.is_stopped(&monitor_change_id).await {
                            monitor_cancel.cancel();
                            break;
                        }
                    }

                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            });

            // Apply+Acceptance loop: retry apply when acceptance fails.
            // Resume routing determines whether we start from apply or acceptance.
            // The acceptance-only shortcut is consumed after one cycle so that any
            // acceptance FAIL/command error path re-enters apply on the next cycle.
            // Rejecting resumes are handled as an immediate rejection review branch.
            if matches!(resume_action, ResumeAction::Rejecting) {
                let rejection_review_deferred = if let Some(shared) = &shared_orchestrator_state {
                    let mut guard = shared.write().await;
                    guard.mark_reject_wait(&change_id);
                    guard
                        .reject_wait_change_ids()
                        .iter()
                        .any(|id| id == &change_id)
                } else {
                    false
                };

                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::WorkspaceStatusUpdated {
                            change_id: change_id.clone(),
                            workspace_name: workspace.name.clone(),
                            status: WorkspaceStatus::Rejecting,
                        })
                        .await;
                }

                if rejection_review_deferred {
                    if let Some(queue) = &dynamic_queue_for_requeue {
                        queue.push(change_id.clone()).await;
                    }
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::Log(
                                LogEntry::info(
                                    "Rejecting resume deferred because the base-mutating lane is occupied; status=reject pending",
                                )
                                .with_change_id(&change_id)
                                .with_operation("rejecting"),
                            ))
                            .await;
                    }
                    cancel_monitor.abort();
                    return WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: None,
                        error: None,
                        rejected: None,
                    };
                }

                match run_rejection_review(&change_id, &workspace.path, &config, &ai_runner).await {
                    Ok(RejectionReviewVerdict::Confirm) => {
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::RejectionReviewCompleted {
                                    change_id: change_id.clone(),
                                    outcome: crate::events::RejectionOutcome::Confirm,
                                })
                                .await;
                        }
                        let rejected_path = workspace
                            .path
                            .join("openspec")
                            .join("changes")
                            .join(&change_id)
                            .join("REJECTED.md");
                        let reason = format!(
                            "Rejecting review confirmed rejection (proposal: {})",
                            rejected_path.display()
                        );
                        let resolved_base = base_branch.clone();
                        match execute_rejection_flow(
                            &change_id,
                            &reason,
                            &workspace.path,
                            &resolved_base,
                            &repo_root,
                        )
                        .await
                        {
                            Ok(()) => {
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::ChangeRejected {
                                            change_id: change_id.clone(),
                                            reason: reason.clone(),
                                        })
                                        .await;
                                    let _ = tx
                                        .send(ParallelEvent::ChangeDequeued {
                                            change_id: change_id.clone(),
                                        })
                                        .await;
                                }
                                cancel_monitor.abort();
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: None,
                                    rejected: Some(reason),
                                };
                            }
                            Err(e) => {
                                cancel_monitor.abort();
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: Some(format!(
                                        "Rejected flow failed after rejecting CONFIRM verdict: {}",
                                        e
                                    )),
                                    rejected: None,
                                };
                            }
                        }
                    }
                    Ok(RejectionReviewVerdict::Resume) => {
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::RejectionReviewCompleted {
                                    change_id: change_id.clone(),
                                    outcome: crate::events::RejectionOutcome::Resume,
                                })
                                .await;
                        }
                        if let Err(e) = handle_resume_apply_from_rejecting(&change_id, &workspace.path).await {
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::RejectionReviewFailed {
                                        change_id: change_id.clone(),
                                        error: e.to_string(),
                                    })
                                    .await;
                            }
                            cancel_monitor.abort();
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: None,
                                error: Some(format!(
                                    "Failed to resume apply from rejecting verdict: {}",
                                    e
                                )),
                                rejected: None,
                            };
                        }
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::Log(
                                    LogEntry::warn("Rejecting review returned RESUME; returning to apply loop")
                                        .with_change_id(&change_id)
                                        .with_operation("rejecting"),
                                ))
                                .await;
                        }
                    }
                    Ok(RejectionReviewVerdict::Block) => {
                        if let Err(e) = handle_blocked_from_rejecting(&change_id, &workspace.path).await {
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::RejectionReviewFailed {
                                        change_id: change_id.clone(),
                                        error: e.to_string(),
                                    })
                                    .await;
                            }
                            cancel_monitor.abort();
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: None,
                                error: Some(format!(
                                    "Failed to transition rejecting verdict BLOCK into blocked state: {}",
                                    e
                                )),
                                rejected: None,
                            };
                        }

                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::RejectionReviewCompleted {
                                    change_id: change_id.clone(),
                                    outcome: crate::events::RejectionOutcome::Block,
                                })
                                .await;
                            let _ = tx
                                .send(ParallelEvent::Log(
                                    LogEntry::warn("Rejecting review returned BLOCK; cleared rejection proposal and preserved stalled workspace")
                                        .with_change_id(&change_id)
                                        .with_operation("rejecting"),
                                ))
                                .await;
                        }
                        cancel_monitor.abort();
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: None,
                            rejected: None,
                        };
                    }
                    Err(e) => {
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::RejectionReviewFailed {
                                    change_id: change_id.clone(),
                                    error: e.to_string(),
                                })
                                .await;
                        }
                        cancel_monitor.abort();
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some(format!(
                                "Rejecting review failed while resuming rejecting stage: {}",
                                e
                            )),
                            rejected: None,
                        };
                    }
                }
            }
            let (mut skip_apply_once, mut skip_acceptance_once) =
                resume_cycle_flags(resume_action);

            let _apply_revision = loop {
                cycle_count += 1;
                if cycle_count > MAX_ACCEPTANCE_RETRY_CYCLES {
                    // Cycle exhaustion is a runtime safety ceiling, not a
                    // reviewer-validated external blocker. It carries no
                    // explicit category or evidence, so it must not fabricate
                    // one or create a durable stalled hold; it stops and waits
                    // for an explicit operator retry.
                    let error = format!(
                        "Acceptance retry ceiling reached for {change_id}: {MAX_ACCEPTANCE_RETRY_CYCLES} \
                         apply+acceptance cycles produced no resolution. Explicit retry is required."
                    );
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::Log(
                                LogEntry::error(error.clone())
                                    .with_change_id(&change_id)
                                    .with_operation("acceptance"),
                            ))
                            .await;
                    }
                    cancel_monitor.abort();
                    return WorkspaceResult { change_id, workspace_name: workspace.name, final_revision: None, error: Some(error), rejected: None };
                }

                // Skip apply only for the first cycle when resuming from an already-applied state.
                // Even when apply is skipped, this cycle must still execute acceptance unless
                // resume_action explicitly allows archive continuation.
                let (revision, final_iteration, blocked_handoff, rejected_handoff) = if !should_run_apply(&mut skip_apply_once) {
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::Log(
                                LogEntry::info(format!(
                                    "Skipping apply for {} (workspace already in {:?} state); continuing with acceptance/archive routing",
                                    change_id, effective_state
                                ))
                                .with_change_id(&change_id),
                            ))
                            .await;
                    }

                    match crate::vcs::git::commands::get_current_commit(&workspace.path).await {
                        Ok(revision) => (revision, cumulative_iteration, None, None),
                        Err(e) => {
                            cancel_monitor.abort();
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: None,
                                error: Some(format!(
                                    "Failed to resolve current revision while resuming without apply: {}",
                                    e
                                )),
                                rejected: None,
                            };
                        }
                    }
                } else {

                // Check if this change has been stopped (single-change stop)
                if let Some(ref queue) = dynamic_queue {
                    if queue.is_stopped(&change_id).await {
                        queue.clear_stopped(&change_id).await;
                        info!("Change '{}' stopped during execution", change_id);
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::ChangeDequeued {
                                    change_id: change_id.clone(),
                                })
                                .await;
                            let _ = tx
                                .send(ParallelEvent::Log(LogEntry::info(format!(
                                    "Change stopped: {}",
                                    change_id
                                ))))
                                .await;
                        }
                        cancel_monitor.abort();
                                    return WorkspaceResult {
                                        change_id,
                                        workspace_name: workspace.name,
                                        final_revision: None,
                                        error: None, // No error - intentionally stopped
                                        rejected: None,
                                    };



                    }
                }

                // Step 1: Execute apply with cumulative iteration count
                // Use per-change cancel token that monitors both global and single-change stop
                let apply_result = execute_apply_in_workspace(
                    &change_id,
                    &workspace.path,
                    &apply_command,
                    &config,
                    event_tx.clone(),
                    vcs_backend,
                    None, // hooks
                    None, // parallel_ctx
                    Some(&per_change_cancel),
                    &ai_runner,
                    &repo_root,
                    &apply_history,
                    &acceptance_history,
                    &acceptance_tail_injected,
                    cumulative_iteration, // Pass current iteration count
                )
                .await;

                match apply_result {
                    Ok((rev, iter, blocked_handoff, rejected_handoff)) => {
                        (rev, iter, blocked_handoff, rejected_handoff)
                    },
                    Err(e) => {
                        // Check if this was a single-change stop
                        let error_str = e.to_string();
                        if error_str.contains("Cancelled") {
                            if let Some(ref queue) = dynamic_queue {
                                if queue.is_stopped(&change_id).await {
                                    queue.clear_stopped(&change_id).await;
                                    info!("Change '{}' stopped during apply", change_id);
                                    if let Some(ref tx) = event_tx {
                                        let _ = tx
                                            .send(ParallelEvent::ChangeDequeued {
                                                change_id: change_id.clone(),
                                            })
                                            .await;
                                        let _ = tx
                                            .send(ParallelEvent::Log(LogEntry::info(format!(
                                                "Change stopped: {}",
                                                change_id
                                            ))))
                                            .await;
                                    }
                                    cancel_monitor.abort();
                                    return WorkspaceResult {
                                        change_id,
                                        workspace_name: workspace.name,
                                        final_revision: None,
                                        error: None, // No error - intentionally stopped
                                        rejected: None,
                                    };
                                }
                            }
                        }
                        if matches!(e, OrchestratorError::PermissionStalled { .. }) {
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::warn(format!(
                                            "Apply stalled on repeated unresolved permission/tool policy denial: {}",
                                            e
                                        ))
                                        .with_change_id(&change_id)
                                        .with_operation("apply"),
                                    ))
                                    .await;
                            }
                            cancel_monitor.abort();
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: None,
                                error: None,
                                rejected: None,
                            };
                        }

                        // Apply failed - return error immediately
                        cancel_monitor.abort();
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some(format!("Apply failed: {}", e)),
                            rejected: None,
                        };
                    }
                }
                };

                // Update cumulative iteration count
                cumulative_iteration = final_iteration;

                if let Some(handoff) = &rejected_handoff {
                    info!(
                        change_id = %change_id,
                        rejected_path = %handoff.rejected_path.display(),
                        "Apply emitted rejection proposal; entering rejecting review flow"
                    );
                    let rejection_review_deferred = if let Some(shared) = &shared_orchestrator_state {
                        let mut guard = shared.write().await;
                        guard.mark_reject_wait(&change_id);
                        guard
                            .reject_wait_change_ids()
                            .iter()
                            .any(|id| id == &change_id)
                    } else {
                        false
                    };

                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::WorkspaceStatusUpdated {
                                change_id: change_id.clone(),
                                workspace_name: workspace.name.clone(),
                                status: WorkspaceStatus::Rejecting,
                            })
                            .await;
                        let _ = tx
                            .send(ParallelEvent::Log(
                                LogEntry::warn(format!(
                                    "Apply rejection proposal detected via {}; entering rejecting review",
                                    handoff.rejected_path.display()
                                ))
                                .with_change_id(&change_id)
                                .with_operation("apply"),
                            ))
                            .await;
                    }

                    if rejection_review_deferred {
                        if let Some(queue) = &dynamic_queue_for_requeue {
                            queue.push(change_id.clone()).await;
                        }
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::Log(
                                    LogEntry::info(
                                        "Rejection review handoff deferred because the base-mutating lane is occupied; status=reject pending",
                                    )
                                    .with_change_id(&change_id)
                                    .with_operation("apply"),
                                ))
                                .await;
                        }
                        cancel_monitor.abort();
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: None,
                            rejected: None,
                        };
                    }

                    match run_rejection_review(&change_id, &workspace.path, &config, &ai_runner).await {
                        Ok(verdict) => match verdict {
                            RejectionReviewVerdict::Confirm => {
                                let rejected_path = workspace
                                    .path
                                    .join("openspec")
                                    .join("changes")
                                    .join(&change_id)
                                    .join("REJECTED.md");
                                let reason = format!(
                                    "Rejecting review confirmed rejection (proposal: {})",
                                    rejected_path.display()
                                );
                                let resolved_base = base_branch.clone();
                                match execute_rejection_flow(
                                    &change_id,
                                    &reason,
                                    &workspace.path,
                                    &resolved_base,
                                    &repo_root,
                                )
                                .await
                                {
                                    Ok(()) => {
                                        if let Some(ref tx) = event_tx {
                                            let _ = tx
                                                .send(ParallelEvent::ChangeRejected {
                                                    change_id: change_id.clone(),
                                                    reason: reason.clone(),
                                                })
                                                .await;
                                            let _ = tx
                                                .send(ParallelEvent::ChangeDequeued {
                                                    change_id: change_id.clone(),
                                                })
                                                .await;
                                        }
                                        cancel_monitor.abort();
                                        return WorkspaceResult {
                                            change_id,
                                            workspace_name: workspace.name,
                                            final_revision: None,
                                            error: None,
                                            rejected: Some(reason),
                                        };
                                    }
                                    Err(e) => {
                                        cancel_monitor.abort();
                                        return WorkspaceResult {
                                            change_id,
                                            workspace_name: workspace.name,
                                            final_revision: None,
                                            error: Some(format!(
                                                "Rejected flow failed after apply-time rejecting CONFIRM verdict: {}",
                                                e
                                            )),
                                            rejected: None,
                                        };
                                    }
                                }
                            }
                            RejectionReviewVerdict::Resume => {
                                if let Err(e) = handle_resume_apply_from_rejecting(&change_id, &workspace.path).await {
                                    if let Some(ref tx) = event_tx {
                                        let _ = tx
                                            .send(ParallelEvent::RejectionReviewFailed {
                                                change_id: change_id.clone(),
                                                error: e.to_string(),
                                            })
                                            .await;
                                    }
                                    cancel_monitor.abort();
                                    return WorkspaceResult {
                                        change_id,
                                        workspace_name: workspace.name,
                                        final_revision: None,
                                        error: Some(format!(
                                            "Failed to resume apply from apply-time rejecting verdict: {}",
                                            e
                                        )),
                                        rejected: None,
                                    };
                                }
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::Log(
                                            LogEntry::warn("Rejecting review returned RESUME; returning to apply loop")
                                                .with_change_id(&change_id)
                                                .with_operation("rejecting"),
                                        ))
                                        .await;
                                }
                                continue;
                            }
                            RejectionReviewVerdict::Block => {
                                if let Err(e) = handle_blocked_from_rejecting(&change_id, &workspace.path).await {
                                    if let Some(ref tx) = event_tx {
                                        let _ = tx
                                            .send(ParallelEvent::RejectionReviewFailed {
                                                change_id: change_id.clone(),
                                                error: e.to_string(),
                                            })
                                            .await;
                                    }
                                    cancel_monitor.abort();
                                    return WorkspaceResult {
                                        change_id,
                                        workspace_name: workspace.name,
                                        final_revision: None,
                                        error: Some(format!(
                                            "Failed to transition apply-time rejecting verdict BLOCK into blocked state: {}",
                                            e
                                        )),
                                        rejected: None,
                                    };
                                }
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::WorkspaceStatusUpdated {
                                            change_id: change_id.clone(),
                                            workspace_name: workspace.name.clone(),
                                            status: WorkspaceStatus::Blocked,
                                        })
                                        .await;
                                    let _ = tx
                                        .send(ParallelEvent::Log(
                                            LogEntry::warn("Rejecting review returned BLOCK; cleared rejection proposal and preserved stalled workspace")
                                                .with_change_id(&change_id)
                                                .with_operation("rejecting"),
                                        ))
                                        .await;
                                }
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: None,
                                    rejected: None,
                                };
                            }
                        },
                        Err(e) => {
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: None,
                                error: Some(format!("Rejecting review failed after apply handoff: {}", e)),
                                rejected: None,
                            };
                        }
                    }
                }

                if let Some(handoff) = &blocked_handoff {
                    info!(
                        change_id = %change_id,
                        blocker_path = %handoff.blocker_path.display(),
                        "Apply emitted stalled handoff marker; staying stalled without rejecting flow"
                    );
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::WorkspaceStatusUpdated {
                                change_id: change_id.clone(),
                                workspace_name: workspace.name.clone(),
                                status: WorkspaceStatus::Blocked,
                            })
                            .await;
                        let _ = tx
                            .send(ParallelEvent::Log(
                                LogEntry::warn(format!(
                                    "Apply stalled handoff detected via {}; workspace remains stalled",
                                    handoff.blocker_path.display()
                                ))
                                .with_change_id(&change_id)
                                .with_operation("apply"),
                            ))
                            .await;
                    }

                    return WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: None,
                        error: None,
                        rejected: None,
                    };
                }

                // Send ApplyCompleted event
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(ParallelEvent::ApplyCompleted {
                            change_id: change_id.clone(),
                            revision: revision.clone(),
                        })
                        .await;
                }

                // Step 2: Execute acceptance test after apply succeeds unless this cycle is
                // resuming directly into archive from workspace-local Archiving state.
                if !skip_acceptance_once {
                    // Update status to Accepting
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::WorkspaceStatusUpdated {
                                change_id: change_id.clone(),
                                workspace_name: workspace.name.clone(),
                                status: WorkspaceStatus::Accepting,
                            })
                            .await;
                    }

                    info!(
                        "Running acceptance test for {} after apply completion (cycle {})",
                        change_id, cycle_count
                    );
                }
                let acceptance_result = if skip_acceptance_once {
                    skip_acceptance_once = false;
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::Log(
                                LogEntry::info(
                                    "Skipping acceptance on resume because workspace is already in Archiving state",
                                )
                                .with_change_id(&change_id)
                                .with_operation("acceptance"),
                            ))
                            .await;
                    }
                    Ok((crate::orchestration::AcceptanceResult::Pass, 0))
                } else {
                    // Validate the repair delta before spending another
                    // acceptance invocation. Passing authorizes only the next
                    // review; it never claims the finding is resolved.
                    if !acceptance_retry.latest_findings().is_empty() {
                        let (changed_files, apply_revision, remediation_evidence) =
                            crate::orchestration::acceptance::collect_repair_gate_inputs(
                                &workspace.path,
                                &change_id,
                                acceptance_retry.fail_revision.as_deref(),
                            )
                            .await;
                        if let crate::orchestration::acceptance::RepairGateDecision::Stop(stop) =
                            crate::orchestration::acceptance::decide_repair_gate(
                                &change_id,
                                acceptance_retry.latest_findings(),
                                &acceptance_retry.repair_ledger,
                                acceptance_retry.fail_revision.as_deref(),
                                apply_revision.as_deref(),
                                &changed_files,
                                &remediation_evidence,
                            )
                        {
                            let error = stop.summary();
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::error(error.clone())
                                            .with_change_id(&change_id)
                                            .with_operation("acceptance"),
                                    ))
                                    .await;
                            }
                            cancel_monitor.abort();
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: None,
                                error: Some(error),
                                rejected: None,
                            };
                        }
                    }
                    agent.seed_acceptance_history(acceptance_history.lock().await.clone());
                    execute_acceptance_in_workspace(
                        &change_id,
                        &workspace.path,
                        &mut agent,
                        event_tx.clone(),
                        Some(&per_change_cancel),
                        &ai_runner,
                        &config,
                        &acceptance_tail_injected,
                        &acceptance_history,
                        Some(base_branch.as_str()),
                        protocol.take_protocol_retry(),
                    )
                    .await
                };

                // Any canonical verdict ends the consecutive missing-verdict
                // sequence; its own routing below is unchanged. Command failure
                // and cancellation are not verdicts and are never reclassified
                // as a missing-verdict retry.
                if acceptance_result
                    .as_ref()
                    .is_ok_and(|(result, _)| result.is_canonical_verdict())
                {
                    protocol.observe_canonical_verdict();
                }

                match acceptance_result {
                    Ok((crate::orchestration::AcceptanceResult::Pass, _acceptance_iteration)) => {
                        // PASS hands off to archive through this active run's
                        // control flow only; no verdict is written to disk.
                        info!("Acceptance passed for {}, proceeding to archive", change_id);
                        match task_parser::resolve_acceptance_follow_up_tasks_path_for_cleanup(
                            &change_id,
                            workspace.path.as_path(),
                        ) {
                            Ok(Some(tasks_path)) => {
                                match task_parser::clear_acceptance_follow_up(&tasks_path) {
                                    Ok(recovery) => {
                                        if let Some(warning) = recovery.warning() {
                                            warn!(
                                                "Acceptance follow-up recovery for {} at {}: {}",
                                                change_id,
                                                tasks_path.display(),
                                                warning
                                            );
                                        }
                                    }
                                    Err(err) => {
                                        return WorkspaceResult {
                                            change_id,
                                            workspace_name: workspace.name,
                                            final_revision: None,
                                            error: Some(format!(
                                                "Acceptance passed but follow-up cleanup failed at {}: {}",
                                                tasks_path.display(),
                                                err
                                            )),
                                            rejected: None,
                                        };
                                    }
                                }
                            }
                            Ok(None) => {
                                debug!("No acceptance follow-up to clear for {}", change_id)
                            }
                            Err(err) => {
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: Some(format!(
                                        "Acceptance passed but follow-up path resolution failed: {}",
                                        err
                                    )),
                                    rejected: None,
                                };
                            }
                        }
                        // Break out of loop, proceed to archive
                        break revision;
                    }
                    Ok((
                        crate::orchestration::AcceptanceResult::Continue,
                        acceptance_iteration,
                    )) => {
                        let continue_count =
                            agent.count_consecutive_acceptance_continues(&change_id);
                        let max_continues = config.get_acceptance_max_continues();

                        if continue_count >= max_continues {
                            warn!(
                                "Acceptance CONTINUE limit ({}) exceeded for {} (cycle {}), treating as FAIL",
                                max_continues, change_id, cycle_count
                            );
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::warn(format!(
                                            "Acceptance CONTINUE limit exceeded (cycle {}), change will not be archived",
                                            cycle_count
                                        ))
                                        .with_change_id(&change_id)
                                        .with_operation("acceptance")
                                        .with_iteration(acceptance_iteration),
                                    ))
                                    .await;
                            }
                            return WorkspaceResult {
                                change_id,
                                workspace_name: workspace.name,
                                final_revision: None,
                                error: Some(format!(
                                    "Acceptance CONTINUE limit ({}) exceeded",
                                    max_continues
                                )),
                                rejected: None,
                            };
                        } else {
                            info!(
                                "Acceptance requires continuation for {} (attempt {}/{}, cycle {}), retrying acceptance",
                                change_id,
                                continue_count,
                                max_continues,
                                cycle_count
                            );
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::info(format!(
                                            "Acceptance requires continuation (attempt {}/{}, cycle {}), retrying",
                                            continue_count,
                                            max_continues,
                                            cycle_count
                                        ))
                                        .with_change_id(&change_id)
                                        .with_operation("acceptance")
                                        .with_iteration(acceptance_iteration),
                                    ))
                                    .await;
                            }
                            // Continue the acceptance loop - retry acceptance without re-applying
                            continue;
                        }
                    }
                    Ok((
                        crate::orchestration::AcceptanceResult::Fail { findings },
                        acceptance_iteration,
                    )) => {
                        let blocking_gate_context = findings
                            .first()
                            .map(|finding| finding.text().to_string())
                            .unwrap_or_else(|| "no acceptance findings captured".to_string());
                        warn!(
                            "Acceptance failed for {} ({} findings) (cycle {}), blocking gate context: {}; returning to apply loop",
                            change_id,
                            findings.len(),
                            cycle_count,
                            blocking_gate_context
                        );
                        let previous = acceptance_retry.clone();
                        let fingerprint = match semantic_progress_fingerprint(&workspace.path) {
                            Ok(fingerprint) => fingerprint,
                            Err(error) => {
                                cancel_monitor.abort();
                                return WorkspaceResult { change_id, workspace_name: workspace.name, final_revision: None, error: Some(format!("Failed to fingerprint acceptance progress: {error}")), rejected: None };
                            }
                        };
                        let normalized = normalize_findings(&findings);
                        let identities = normalized.iter().map(|finding| finding.identity.clone()).collect::<Vec<_>>();
                        let decision = decide_acceptance_retry(
                            previous.previous_identities(),
                            previous.previous_fingerprint(),
                            &normalized, &fingerprint, cycle_count,
                        );
                        // Per-finding accounting runs before the broad semantic
                        // comparison so a repeated ID stops automatic repair even
                        // when unrelated files changed. Serial makes the same call.
                        let mut repair_ledger = acceptance_retry.repair_ledger.clone();
                        let repair = repair_ledger.observe_fail(&normalized);
                        let previous_fail_revision = acceptance_retry.fail_revision.clone();
                        let fail_revision =
                            crate::vcs::git::commands::get_current_commit(&workspace.path)
                                .await
                                .ok();
                        if let crate::orchestration::acceptance::FindingRepairDecision::Stop {
                            repeated_identities,
                            ..
                        } = repair
                        {
                            let (changed_files, apply_revision, remediation_evidence) =
                                crate::orchestration::acceptance::collect_repair_gate_inputs(
                                    &workspace.path,
                                    &change_id,
                                    previous_fail_revision.as_deref(),
                                )
                                .await;
                            let stop = crate::orchestration::acceptance::repeated_finding_stop(
                                &change_id,
                                &findings,
                                &repair_ledger,
                                repeated_identities,
                                previous_fail_revision.as_deref(),
                                apply_revision.as_deref(),
                                &changed_files,
                                &remediation_evidence,
                            );
                            let error = stop.summary();
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::error(error.clone())
                                            .with_change_id(&change_id)
                                            .with_operation("acceptance")
                                            .with_iteration(acceptance_iteration),
                                    ))
                                    .await;
                            }
                            cancel_monitor.abort();
                            return WorkspaceResult { change_id, workspace_name: workspace.name, final_revision: None, error: Some(error), rejected: None };
                        }
                        // The repair opportunity for these identities is consumed
                        // now, so the next FAIL reporting one of them stops.
                        repair_ledger.record_repair_dispatched(&identities);
                        acceptance_retry = AcceptanceRetryContext {
                            finding_identities: identities.clone(),
                            semantic_fingerprint: Some(fingerprint),
                            cycle_count,
                            findings: findings.clone(),
                            repair_ledger,
                            fail_revision,
                        };
                        {
                            let mut shared_history = acceptance_history.lock().await;
                            // Identities and the semantic baseline are comparison
                            // state; the complete payload is written separately so
                            // the checkpoint can never overwrite actionable detail.
                            shared_history.set_retry_checkpoint(
                                &change_id,
                                acceptance_retry.cycle_count,
                                acceptance_retry.finding_identities.clone(),
                                acceptance_retry.semantic_fingerprint.clone(),
                            );
                        }
                        if let AcceptanceRetryDecision::Stall { reason, external_blockers } = decision {
                            // Repeated findings and cycle exhaustion are runtime
                            // retry judgements, not reviewer-validated external
                            // prerequisites: no category is invented, no unblock
                            // condition is fabricated, and nothing durable is
                            // written. They classify as execution `stalled`, the
                            // same outcome serial mode reaches, so both modes
                            // agree and neither presents this as a wait on a
                            // named prerequisite.
                            let error = format!(
                                "Acceptance stopped retrying {change_id} ({reason}). External blocker \
                                 context: {}. Explicit retry is required.",
                                if external_blockers.is_empty() {
                                    "none reported".to_string()
                                } else {
                                    external_blockers.join(" | ")
                                }
                            );
                            // The classifier owns the decision even here, so an
                            // execution stop cannot pick up an external category
                            // by taking a different code path.
                            let stop_reason =
                                crate::orchestration::blocker_classification::execution_stop_reason_for(reason);
                            let stalled_category = match crate::orchestration::blocker_classification::classify_execution_stop(
                                stop_reason,
                                error.clone(),
                            ) {
                                crate::orchestration::blocker_classification::LifecycleClassification::Stalled { reason, .. } => reason,
                                other => unreachable!(
                                    "an execution stop must classify as stalled, got {other:?}"
                                ),
                            };
                            let stalled_blocker = crate::events::StalledBlocker {
                                category: stalled_category,
                                phase: "acceptance".to_string(),
                                gate: "acceptance_retry_policy".to_string(),
                                error_summary: error.clone(),
                                evidence: if external_blockers.is_empty() {
                                    vec![error.clone()]
                                } else {
                                    external_blockers.clone()
                                },
                                // Deliberately absent: an exhausted retry budget
                                // names no verifiable external condition, which
                                // is what keeps it out of external `blocked`.
                                unblock_condition: None,
                                prerequisite_owner: None,
                                next_action: "inspect the repeated findings and request an explicit retry"
                                    .to_string(),
                                resumable: true,
                                worktree_preserved: true,
                            };
                            record_stall_in_shared_state(
                                &shared_orchestrator_state,
                                &crate::events::ExecutionEvent::ExecutionBlocked {
                                    change_id: change_id.clone(),
                                    blocker: stalled_blocker.clone(),
                                },
                            )
                            .await;
                            if let Some(ref tx) = event_tx {
                                let _ = tx
                                    .send(ParallelEvent::ExecutionBlocked {
                                        change_id: change_id.clone(),
                                        blocker: stalled_blocker,
                                    })
                                    .await;
                                let _ = tx
                                    .send(ParallelEvent::Log(
                                        LogEntry::error(error.clone())
                                            .with_change_id(&change_id)
                                            .with_operation("acceptance")
                                            .with_iteration(acceptance_iteration),
                                    ))
                                    .await;
                            }
                            cancel_monitor.abort();
                            // No `error`: the change is held, not failed, so the
                            // worktree stays preserved for the explicit retry.
                            return WorkspaceResult { change_id, workspace_name: workspace.name, final_revision: None, error: None, rejected: None };
                        }
                        if !findings.is_empty() {
                            if let Ok(tasks_path) = task_parser::resolve_acceptance_follow_up_tasks_path(&change_id, workspace.path.as_path()) {
                                match task_parser::replace_acceptance_follow_up_from_latest_fail(&tasks_path, acceptance_iteration, &findings) {
                                    Ok(recovery) => {
                                        if let Some(warning) = recovery.warning() {
                                            warn!("Acceptance follow-up recovery for {} at {}: {}", change_id, tasks_path.display(), warning);
                                        }
                                    }
                                    // Acceptance FAIL remains the primary diagnosis; persistence
                                    // degradation is supplemental context only.
                                    Err(err) => warn!("Acceptance follow-up persistence degraded for {} at {}: {}", change_id, tasks_path.display(), err),
                                }
                            }
                        }
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::Log(
                                    LogEntry::warn(format!(
                                        "Acceptance failed ({} findings), blocking gate context: {}; returning to apply loop (cycle {})",
                                        findings.len(),
                                        blocking_gate_context,
                                        cycle_count
                                    ))
                                    .with_change_id(&change_id)
                                    .with_operation("acceptance")
                                    .with_iteration(acceptance_iteration),
                                ))
                                .await;
                        }
                        continue;
                    }
                    Ok((
                        crate::orchestration::AcceptanceResult::MalformedFinding { rejection },
                        acceptance_iteration,
                    )) => {
                        // Bounded protocol error: acceptance is re-invoked for a
                        // corrected verdict. No repair work is dispatched and no
                        // path-only follow-up is written.
                        match protocol.observe_malformed_finding(&rejection) {
                            crate::orchestration::acceptance::MissingVerdictRetryStep::Retry {
                                progress,
                                ..
                            } => {
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::Log(
                                            LogEntry::info(progress)
                                                .with_change_id(&change_id)
                                                .with_operation("acceptance")
                                                .with_iteration(acceptance_iteration),
                                        ))
                                        .await;
                                }
                                continue;
                            }
                            crate::orchestration::acceptance::MissingVerdictRetryStep::Exhausted {
                                error,
                            } => {
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::Log(
                                            LogEntry::error(error.clone())
                                                .with_change_id(&change_id)
                                                .with_operation("acceptance")
                                                .with_iteration(acceptance_iteration),
                                        ))
                                        .await;
                                }
                                cancel_monitor.abort();
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: Some(error),
                                    rejected: None,
                                };
                            }
                        }
                    }
                    Ok((
                        crate::orchestration::AcceptanceResult::PermissionStalled { blocker },
                        acceptance_iteration,
                    )) => {
                        // A repeated unresolved permission/tool-policy denial is a
                        // concrete repository-external prerequisite with an
                        // explicit category supplied by the denial classifier —
                        // never inferred from narrative prose. The hold is
                        // recorded in the in-memory reducer only; nothing is
                        // written outside the managed worktree.
                        warn!(
                            "Acceptance stalled for {} on repeated unresolved permission/tool policy denial (cycle {}): {}",
                            change_id,
                            cycle_count,
                            blocker.summary()
                        );
                        record_stall_in_shared_state(
                            &shared_orchestrator_state,
                            &crate::events::ExecutionEvent::ExecutionBlocked {
                                change_id: change_id.clone(),
                                blocker: blocker.clone(),
                            },
                        )
                        .await;
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::ExecutionBlocked {
                                    change_id: change_id.clone(),
                                    blocker: blocker.clone(),
                                })
                                .await;
                            let _ = tx
                                .send(ParallelEvent::Log(
                                    LogEntry::warn(format!(
                                        "Acceptance stalled on repeated unresolved permission/tool policy denial (cycle {}): {}",
                                        cycle_count,
                                        blocker.summary()
                                    ))
                                    .with_change_id(&change_id)
                                    .with_operation("acceptance")
                                    .with_iteration(acceptance_iteration),
                                ))
                                .await;
                        }
                        cancel_monitor.abort();
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: None,
                            rejected: None,
                        };
                    }
                    Ok((
                        crate::orchestration::AcceptanceResult::CommandFailed {
                            error,
                            findings: _,
                        },
                        acceptance_iteration,
                    )) => {
                        error!(
                            "Acceptance command failed for {} (cycle {}): {}",
                            change_id, cycle_count, error
                        );
                        // Canonical owner note: runtime appends follow-up tasks for FAIL verdicts,
                        // while command-level failures are surfaced without forcing local tasks.md updates.
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(ParallelEvent::Log(
                                    LogEntry::error(format!(
                                        "Acceptance command failed (cycle {}): {}",
                                        cycle_count, error
                                    ))
                                    .with_change_id(&change_id)
                                    .with_operation("acceptance")
                                    .with_iteration(acceptance_iteration),
                                ))
                                .await;
                        }
                        // Command failed - this is a critical error, don't retry
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some(format!("Acceptance command failed: {}", error)),
                            rejected: None,
                        };
                    }
                    Ok((
                        crate::orchestration::AcceptanceResult::MissingVerdict { findings },
                        acceptance_iteration,
                    )) => {
                        // The acceptance command completed without a canonical
                        // verdict. This is a protocol failure distinct from an
                        // explicit CONTINUE: it uses its own bounded retry budget
                        // and never touches the CONTINUE retry counter.
                        match protocol.observe_missing_verdict(&findings) {
                            MissingVerdictRetryStep::Retry { progress, .. } => {
                                warn!(
                                    "Missing acceptance verdict for {} (cycle {}): {}",
                                    change_id, cycle_count, progress
                                );
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::Log(
                                            LogEntry::info(format!(
                                                "{} (cycle {})",
                                                progress, cycle_count
                                            ))
                                            .with_change_id(&change_id)
                                            .with_operation("acceptance")
                                            .with_iteration(acceptance_iteration),
                                        ))
                                        .await;
                                }
                                // Re-run the normal configured acceptance command
                                // with continuation context; apply is not repeated
                                // because the implementation itself did not fail.
                                skip_apply_once = true;
                                continue;
                            }
                            MissingVerdictRetryStep::Exhausted { error } => {
                                error!(
                                    "Missing acceptance verdict for {} (cycle {}): {}",
                                    change_id, cycle_count, error
                                );
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::Log(
                                            LogEntry::error(format!(
                                                "Missing acceptance verdict (cycle {}): {}",
                                                cycle_count, error
                                            ))
                                            .with_change_id(&change_id)
                                            .with_operation("acceptance")
                                            .with_iteration(acceptance_iteration),
                                        ))
                                        .await;
                                }
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: Some(error),
                                    rejected: None,
                                };
                            }
                        }
                    }
                    Ok((
                        result @ (crate::orchestration::AcceptanceResult::BareBlocker { .. }
                            | crate::orchestration::AcceptanceResult::Stalled { .. }),
                        acceptance_iteration,
                    )) => {
                        // One shared decision API drives both modes: bare or
                        // invalid compatibility input gets bounded acceptance-only
                        // retry and sets no lifecycle state, while a validated
                        // structured payload goes to the orchestrator's
                        // classifier.
                        match decide_acceptance_blocker(&mut protocol, &result) {
                            Some(AcceptanceBlockerDecision::ProtocolRetry { progress, .. }) => {
                                let diagnostic = crate::orchestration::blocker_classification::bare_compatibility_diagnostic(
                                    "gated",
                                    progress.clone(),
                                );
                                warn!(
                                    "Bare acceptance blocker for {} (cycle {}): {}",
                                    change_id, cycle_count, diagnostic
                                );
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::Log(
                                            LogEntry::info(format!(
                                                "{} (cycle {})",
                                                diagnostic, cycle_count
                                            ))
                                            .with_change_id(&change_id)
                                            .with_operation("acceptance")
                                            .with_iteration(acceptance_iteration),
                                        ))
                                        .await;
                                }
                                // Acceptance only: the implementation did not
                                // fail, so apply is never repeated and no stalled
                                // lifecycle transition is emitted.
                                skip_apply_once = true;
                                continue;
                            }
                            Some(AcceptanceBlockerDecision::ProtocolExhausted { error }) => {
                                error!(
                                    "Bare acceptance blocker for {} (cycle {}): {}",
                                    change_id, cycle_count, error
                                );
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::Log(
                                            LogEntry::error(format!(
                                                "Bare acceptance blocker (cycle {}): {}",
                                                cycle_count, error
                                            ))
                                            .with_change_id(&change_id)
                                            .with_operation("acceptance")
                                            .with_iteration(acceptance_iteration),
                                        ))
                                        .await;
                                }
                                cancel_monitor.abort();
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: Some(error),
                                    rejected: None,
                                };
                            }
                            Some(AcceptanceBlockerDecision::ExternalBlocker { blocker }) => {
                                // The reported facts become an in-memory reducer
                                // hold bound to this process only; the reducer's
                                // classifier decides `blocked` versus `stalled`.
                                // The worktree is left exactly as acceptance
                                // found it, and no record is written outside it.
                                let stalled_blocker = blocker.to_stalled_blocker();
                                warn!(
                                    "Acceptance reported a validated external blocker for {} ({})",
                                    change_id, blocker.category
                                );
                                record_stall_in_shared_state(
                                    &shared_orchestrator_state,
                                    &crate::events::ExecutionEvent::AcceptanceGated {
                                        change_id: change_id.clone(),
                                        blocker: stalled_blocker.clone(),
                                    },
                                )
                                .await;
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::AcceptanceGated {
                                            change_id: change_id.clone(),
                                            blocker: stalled_blocker,
                                        })
                                        .await;
                                    let _ = tx
                                        .send(ParallelEvent::WorkspaceStatusUpdated {
                                            change_id: change_id.clone(),
                                            workspace_name: workspace.name.clone(),
                                            status: WorkspaceStatus::Blocked,
                                        })
                                        .await;
                                    let _ = tx
                                        .send(ParallelEvent::Log(
                                            LogEntry::warn(format!(
                                                "Acceptance blocked ({}) on a validated external prerequisite; \
                                                 worktree preserved and apply revision {} unchanged",
                                                blocker.category, revision
                                            ))
                                            .with_change_id(&change_id)
                                            .with_operation("acceptance")
                                            .with_iteration(acceptance_iteration),
                                        ))
                                        .await;
                                }
                                cancel_monitor.abort();
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: None,
                                    rejected: None,
                                };
                            }
                            None => unreachable!(
                                "decide_acceptance_blocker owns every blocker-bearing result"
                            ),
                        }
                    }
                    Ok((
                        crate::orchestration::AcceptanceResult::Cancelled,
                        _acceptance_iteration,
                    )) => {
                        // Check if this was a single-change stop
                        if let Some(ref queue) = dynamic_queue {
                            if queue.is_stopped(&change_id).await {
                                queue.clear_stopped(&change_id).await;
                                info!("Change '{}' stopped during acceptance", change_id);
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::ChangeDequeued {
                                            change_id: change_id.clone(),
                                        })
                                        .await;
                                    let _ = tx
                                        .send(ParallelEvent::Log(LogEntry::info(format!(
                                            "Change stopped: {}",
                                            change_id
                                        ))))
                                        .await;
                                }
                                cancel_monitor.abort();
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: None, // No error - intentionally stopped
                                    rejected: None,
                                };
                            }
                        }
                        // Global cancellation
                        info!("Acceptance cancelled for {}", change_id);
                        cancel_monitor.abort();
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some("Acceptance cancelled".to_string()),
                            rejected: None,
                        };
                    }
                    Err(e) => {
                        // Check if this was a single-change stop (error contains "Cancelled")
                        let error_str = e.to_string();
                        if error_str.contains("Cancelled") {
                            if let Some(ref queue) = dynamic_queue {
                                if queue.is_stopped(&change_id).await {
                                    queue.clear_stopped(&change_id).await;
                                    info!("Change '{}' stopped during acceptance", change_id);
                                    if let Some(ref tx) = event_tx {
                                        let _ = tx
                                            .send(ParallelEvent::ChangeDequeued {
                                                change_id: change_id.clone(),
                                            })
                                            .await;
                                        let _ = tx
                                            .send(ParallelEvent::Log(LogEntry::info(format!(
                                                "Change stopped: {}",
                                                change_id
                                            ))))
                                            .await;
                                    }
                                    cancel_monitor.abort();
                                    return WorkspaceResult {
                                        change_id,
                                        workspace_name: workspace.name,
                                        final_revision: None,
                                        error: None, // No error - intentionally stopped
                                        rejected: None,
                                    };
                                }
                            }
                        }
                        error!("Acceptance error for {}: {}", change_id, e);
                        cancel_monitor.abort();
                        return WorkspaceResult {
                            change_id,
                            workspace_name: workspace.name,
                            final_revision: None,
                            error: Some(format!("Acceptance error: {}", e)),
                            rejected: None,
                        };
                    }
                }
            };

            // Step 3: Execute archive after acceptance passes
            // Update status to Archiving
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(ParallelEvent::WorkspaceStatusUpdated {
                        change_id: change_id.clone(),
                        workspace_name: workspace.name.clone(),
                        status: WorkspaceStatus::Archiving,
                    })
                    .await;
            }

            let archive_result = if matches!(resume_action, ResumeAction::Archive)
                && matches!(effective_state, WorkspaceState::Archiving)
            {
                execute_archive_finalization_in_workspace(
                    &change_id,
                    &workspace.path,
                    &config,
                    event_tx.clone(),
                    vcs_backend,
                    &ai_runner,
                    &shared_stagger_state,
                )
                .await
            } else {
                // ArchiveStarted event is sent inside execute_archive_in_workspace with command string
                execute_archive_in_workspace(
                    &change_id,
                    &workspace.path,
                    &archive_command,
                    &config,
                    event_tx.clone(),
                    vcs_backend,
                    None, // hooks
                    None, // parallel_ctx
                    Some(&per_change_cancel),
                    &ai_runner,
                    &archive_history,
                    &apply_history,
                    &shared_stagger_state,
                )
                .await
            };

            match archive_result {
                Ok(archive_revision) => {
                    // Archive succeeded
                    agent.clear_acceptance_history(&change_id);
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::ChangeArchived(change_id.clone()))
                            .await;
                    }
                    cancel_monitor.abort();
                    WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: Some(archive_revision),
                        error: None,
                        rejected: None,
                    }
                }
                Err(e) => {
                    // Check if this was a single-change stop
                    if e.to_string().contains("Cancelled") {
                        if let Some(ref queue) = dynamic_queue {
                            if queue.is_stopped(&change_id).await {
                                queue.clear_stopped(&change_id).await;
                                info!("Change '{}' stopped during archive", change_id);
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(ParallelEvent::ChangeDequeued {
                                            change_id: change_id.clone(),
                                        })
                                        .await;
                                    let _ = tx
                                        .send(ParallelEvent::Log(LogEntry::info(format!(
                                            "Change stopped: {}",
                                            change_id
                                        ))))
                                        .await;
                                }
                                cancel_monitor.abort();
                                return WorkspaceResult {
                                    change_id,
                                    workspace_name: workspace.name,
                                    final_revision: None,
                                    error: None, // No error - intentionally stopped
                                    rejected: None,
                                };
                            }
                        }
                    }
                    warn!("Archive failed for {}: {}", change_id, e);
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(ParallelEvent::ArchiveFailed {
                                change_id: change_id.clone(),
                                error: e.to_string(),
                                reason: None,
                                summary: Some(
                                    "Archive failed; external resume state is non-authoritative"
                                        .to_string(),
                                ),
                            })
                            .await;
                    }

                    cancel_monitor.abort();
                    // Archive failed - do not merge unarchived changes
                    WorkspaceResult {
                        change_id,
                        workspace_name: workspace.name,
                        final_revision: None,
                        error: Some(format!("Archive failed: {}", e)),
                        rejected: None,
                    }
                }
            }
            // _permit is dropped here, releasing semaphore
        });

        Ok(())
    }
}

//! Invalid queued proposal metadata is a visible change-local error.
//!
//! The regression: the strict proposal/dependency reader refused a queued
//! candidate, the ID went only into a local `dependency_blocked` vector, and
//! the blocked-only short circuit returned before anything published a reason.
//! The row stayed plain `queued` with no blocker and no work, forever.
//!
//! Every test here drives the production scheduler steps (retry-edge Step 0,
//! reconciliation, `evaluate_queued_reanalysis_and_dispatch`) over a real
//! reducer and the production `RunControlService` retry path. Events are
//! adopted into the reducer explicitly, so reducer-adoption lag is a state the
//! tests choose rather than a race they hope for. Paused Tokio time throughout.

use crate::analyzer::{AnalysisOutcome, AnalysisResult};
use crate::config::OrchestratorConfig;
use crate::events::ExecutionEvent;
use crate::openspec::{Change, ProposalMetadata};
use crate::orchestration::operator_command::{
    ExecutionMarkStore, NoopQueueHooks, OperatorCommandService, OperatorMode,
};
use crate::orchestration::run_control::testing::RecordingScheduler;
use crate::orchestration::run_control::{ResolveReservations, RunControlService, StartEligibility};
use crate::orchestration::state::{OrchestratorState, ReducerCommand};
use crate::parallel::cleanup::WorkspaceCleanupGuard;
use crate::parallel::dynamic_queue::ReanalysisReason;
use crate::parallel::queue_state::{QueuedWorkClass, ReanalysisDispatchContext};
use crate::parallel::{ParallelExecutor, WorkspaceResult};
use crate::tui::queue::DynamicQueue;
use crate::vcs::VcsBackend;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinSet;

const ALPHA: &str = "alpha";
const BETA: &str = "beta";
const GAMMA: &str = "gamma";

/// A value that must never travel from proposal YAML into any diagnostic.
const SENTINEL: &str = "SENTINEL-SECRET-7f3a";

const VALID: &str = "---\nchange_type: implementation\n---\n# Valid\n";

type AnalysisFuture<'a> = Pin<Box<dyn Future<Output = AnalysisOutcome> + Send + 'a>>;

/// Invalid proposal fixtures: name, content (`None` = proposal file missing).
fn invalid_fixtures() -> Vec<(&'static str, Option<String>)> {
    let wrong_typed = format!("---\ndependencies:\n  token: {SENTINEL}\n---\n# Wrong type\n");
    let unclosed = format!("---\ndependencies: [{SENTINEL}]\n# never closed\n");
    vec![
        ("wrong-typed-lf", Some(wrong_typed.clone())),
        ("wrong-typed-crlf", Some(wrong_typed.replace('\n', "\r\n"))),
        ("unclosed-lf", Some(unclosed.clone())),
        ("unclosed-crlf", Some(unclosed.replace('\n', "\r\n"))),
        ("missing", None),
    ]
}

/// Analyzer double that records every target it is handed and orders them.
fn recording_analyzer(
    targets: Arc<Mutex<Vec<String>>>,
) -> impl for<'a> Fn(&'a [Change], &'a [String], u32) -> AnalysisFuture<'a> + Send + Sync {
    move |changes: &[Change], _in_flight: &[String], _iteration: u32| -> AnalysisFuture<'_> {
        let order: Vec<String> = changes.iter().map(|change| change.id.clone()).collect();
        targets
            .lock()
            .expect("analyzer targets")
            .extend(order.iter().cloned());
        Box::pin(async move {
            AnalysisResult {
                order,
                dependencies: HashMap::new(),
                groups: None,
            }
            .into()
        })
    }
}

fn test_config(workspace_base: &std::path::Path) -> OrchestratorConfig {
    OrchestratorConfig {
        apply_command: Some("echo apply {change_id}".to_string()),
        archive_command: Some("echo archive {change_id}".to_string()),
        analyze_command: Some("echo analyze".to_string()),
        acceptance_command: Some("echo acceptance".to_string()),
        resolve_command: Some("echo resolve".to_string()),
        workspace_base_dir: Some(workspace_base.to_string_lossy().to_string()),
        ..Default::default()
    }
}

fn test_change(id: &str) -> Change {
    Change {
        id: id.to_string(),
        completed_tasks: 0,
        total_tasks: 1,
        last_modified: String::new(),
        dependencies: Vec::new(),
        metadata: ProposalMetadata::default(),
    }
}

fn git(repo_root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .expect("run git command");
    assert!(output.status.success(), "git {args:?} failed");
}

fn write_proposal(repo_root: &std::path::Path, change_id: &str, proposal: Option<&str>) {
    let dir = repo_root.join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&dir).expect("create change dir");
    std::fs::write(dir.join("tasks.md"), "- [ ] work\n").expect("write tasks");
    let path = dir.join("proposal.md");
    match proposal {
        Some(content) => std::fs::write(&path, content).expect("write proposal"),
        None => {
            let _ = std::fs::remove_file(&path);
        }
    }
}

fn commit_all(repo_root: &std::path::Path, message: &str) {
    git(repo_root, &["add", "-A"]);
    git(repo_root, &["commit", "--allow-empty", "-m", message]);
}

/// Everything one drained batch of scheduler events said.
#[derive(Debug, Default)]
struct Observed {
    processing_errors: Vec<(String, String)>,
    process_errors: Vec<String>,
    dependency_blocked: Vec<(String, Vec<String>)>,
    dispatch_started: Vec<String>,
}

impl Observed {
    fn errors_for(&self, change_id: &str) -> Vec<&str> {
        self.processing_errors
            .iter()
            .filter(|(id, _)| id == change_id)
            .map(|(_, error)| error.as_str())
            .collect()
    }

    fn dispatches_of(&self, change_id: &str) -> usize {
        self.dispatch_started
            .iter()
            .filter(|id| id.as_str() == change_id)
            .count()
    }
}

struct Harness {
    executor: ParallelExecutor,
    state: Arc<RwLock<OrchestratorState>>,
    run_control: RunControlService,
    marks: Arc<ExecutionMarkStore>,
    queued: Vec<Change>,
    in_flight: HashSet<String>,
    join_set: JoinSet<WorkspaceResult>,
    cleanup_guard: WorkspaceCleanupGuard,
    reanalysis_reason: ReanalysisReason,
    iteration: u32,
    /// Every candidate ID the analyzer was ever handed.
    analyzer_targets: Arc<Mutex<Vec<String>>>,
    events: mpsc::Receiver<ExecutionEvent>,
    repo_root: std::path::PathBuf,
    _repo_dir: TempDir,
    _workspace_base: TempDir,
}

impl Harness {
    /// A committed repository holding `proposals`, with each ID admitted
    /// through reducer queue intent and the scheduler-local candidate list.
    fn new(proposals: &[(&str, Option<&str>)]) -> Self {
        let repo_dir = TempDir::new().expect("create repo dir");
        let workspace_base = TempDir::new().expect("create workspace base");
        let root = repo_dir.path();
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.email", "test@example.com"]);
        git(root, &["config", "user.name", "Test User"]);
        std::fs::write(root.join("README.md"), "base\n").expect("write base file");
        for (id, proposal) in proposals {
            write_proposal(root, id, *proposal);
        }
        commit_all(root, "Base");

        let (tx, events) = mpsc::channel(512);
        let mut executor = ParallelExecutor::new(
            root.to_path_buf(),
            test_config(workspace_base.path()),
            Some(tx),
        );
        let queue = Arc::new(DynamicQueue::new());
        executor.set_dynamic_queue(queue.clone());
        let ids: Vec<String> = proposals.iter().map(|(id, _)| id.to_string()).collect();
        let state = Arc::new(RwLock::new(OrchestratorState::new(ids.clone(), 10)));
        executor.set_shared_orchestrator_state(state.clone());

        let marks = Arc::new(ExecutionMarkStore::new());
        let scheduler = Arc::new(RecordingScheduler::new());
        scheduler.set_running(true);
        let operator = Arc::new(OperatorCommandService::new(
            state.clone(),
            queue,
            Arc::new(NoopQueueHooks),
            marks.clone(),
        ));
        let run_control = RunControlService::new(
            state.clone(),
            operator,
            scheduler,
            Arc::new(ResolveReservations::new()),
            Arc::new(StartEligibility::new()),
        );

        Self {
            executor,
            state,
            run_control,
            marks,
            queued: Vec::new(),
            in_flight: HashSet::new(),
            join_set: JoinSet::new(),
            cleanup_guard: WorkspaceCleanupGuard::new(VcsBackend::Git, root.to_path_buf()),
            reanalysis_reason: ReanalysisReason::Initial,
            iteration: 1,
            analyzer_targets: Arc::new(Mutex::new(Vec::new())),
            events,
            repo_root: root.to_path_buf(),
            _repo_dir: repo_dir,
            _workspace_base: workspace_base,
        }
    }

    async fn admit(&mut self, change_id: &str) {
        self.state
            .write()
            .await
            .apply_command(ReducerCommand::AddToQueue(change_id.to_string()));
        self.marks.set(change_id, true);
        if !self.queued.iter().any(|change| change.id == change_id) {
            self.queued.push(test_change(change_id));
        }
    }

    /// One scheduler loop pass in production order.
    async fn pass(&mut self) {
        let analyzer = recording_analyzer(self.analyzer_targets.clone());

        let retry_edges = self.executor.consume_explicit_retry_edges().await;
        let in_flight = self.in_flight.clone();
        let dynamic_queue_added = self
            .executor
            .check_dynamic_queue_and_add_changes(
                &mut self.queued,
                &in_flight,
                &mut self.reanalysis_reason,
            )
            .await;
        let reconciliation = self
            .executor
            .reconcile_queued_candidates_from_shared_state(&mut self.queued, &in_flight)
            .await;
        self.reanalysis_reason = ParallelExecutor::derive_pass_reanalysis_reason(
            self.reanalysis_reason,
            retry_edges,
            reconciliation,
            dynamic_queue_added,
        );
        let outcome = self
            .executor
            .evaluate_queued_reanalysis_and_dispatch(
                ReanalysisDispatchContext {
                    queued: &mut self.queued,
                    in_flight: &mut self.in_flight,
                    max_parallelism: 4,
                    iteration: self.iteration,
                    reanalysis_reason: self.reanalysis_reason,
                    analyzer: &analyzer,
                    join_set: &mut self.join_set,
                    cleanup_guard: &mut self.cleanup_guard,
                    work_snapshot: None,
                },
                &mut self.reanalysis_reason,
            )
            .await
            .expect("scheduler evaluation should not fail");
        if let Some((_, iteration)) = outcome {
            self.iteration = iteration;
        }
        // Each pass is a fresh debounce-free evaluation, as a later wake is.
        *self.executor.last_queue_change_at.lock().await = None;
    }

    /// Drain events without letting the reducer see them (adoption lag).
    fn drain(&mut self) -> Observed {
        let mut observed = Observed::default();
        while let Ok(event) = self.events.try_recv() {
            match event {
                ExecutionEvent::ProcessingError { id, error } => {
                    observed.processing_errors.push((id, error))
                }
                ExecutionEvent::Error { message } => observed.process_errors.push(message),
                ExecutionEvent::DependencyBlocked {
                    change_id,
                    dependency_ids,
                } => observed
                    .dependency_blocked
                    .push((change_id, dependency_ids)),
                ExecutionEvent::WorkspacePreparationStarted { change_id } => {
                    observed.dispatch_started.push(change_id)
                }
                _ => {}
            }
        }
        observed
    }

    /// Adopt `observed` change-local errors the way the authoritative
    /// dispatcher does: reducer error plus a revoked execution mark.
    async fn adopt(&self, observed: &Observed) {
        let mut guard = self.state.write().await;
        for (id, error) in &observed.processing_errors {
            guard.apply_execution_event(&ExecutionEvent::ProcessingError {
                id: id.clone(),
                error: error.clone(),
            });
            self.marks.set(id, false);
        }
    }

    async fn status(&self, change_id: &str) -> String {
        self.state
            .read()
            .await
            .display_status(change_id)
            .to_string()
    }

    async fn error_detail(&self, change_id: &str) -> Option<String> {
        self.state
            .read()
            .await
            .all_error_details()
            .remove(change_id)
    }

    fn analyzed(&self, change_id: &str) -> usize {
        self.analyzer_targets
            .lock()
            .expect("analyzer targets")
            .iter()
            .filter(|id| id.as_str() == change_id)
            .count()
    }

    /// Operator marks the errored row and presses F5.
    async fn retry(&mut self, change_id: &str) {
        self.marks.set(change_id, true);
        self.run_control
            .start(OperatorMode::Running)
            .await
            .expect("a marked retry-eligible error is startable");
    }

    async fn shutdown(mut self) {
        self.join_set.abort_all();
        while self.join_set.join_next().await.is_some() {}
    }
}

fn assert_safe_diagnostic(error: &str, change_id: &str, case: &str) {
    assert!(
        error.contains(&format!("Change '{change_id}'")),
        "{case}: names the change: {error}"
    );
    assert!(
        error.contains(&format!("openspec/changes/{change_id}/proposal.md")),
        "{case}: names the repository-relative proposal: {error}"
    );
    assert!(
        error.contains("unreadable proposal") || error.contains("invalid proposal metadata"),
        "{case}: fixed bounded category: {error}"
    );
    assert!(
        error.contains("Correct and commit") && error.contains("explicitly Retry"),
        "{case}: actionable remedy: {error}"
    );
    assert!(!error.contains(SENTINEL), "{case}: no raw value: {error}");
    assert!(
        !error.contains("/tmp") && !error.contains("Failed to"),
        "{case}: no absolute path or raw parser text: {error}"
    );
}

/// Invalid-only queue, every reader failure, LF and CRLF: the row settles to
/// a change-local error before analysis, with no self-dependency, no global
/// Error, no analyzer target, no dispatch, and one diagnostic across repeats.
#[tokio::test(start_paused = true)]
async fn queued_proposal_diagnostic_invalid_only_settles_to_change_local_error() {
    for (case, proposal) in invalid_fixtures() {
        let mut harness = Harness::new(&[(ALPHA, proposal.as_deref()), (BETA, Some(VALID))]);
        harness.admit(ALPHA).await;
        // An unrelated, unadmitted marked row must be left exactly alone.
        harness.marks.set(BETA, true);

        for _ in 0..3 {
            harness.pass().await;
        }
        // Three unadopted passes: the reducer has not caught up, so the
        // duplicate is suppressed rather than flooded.
        let observed = harness.drain();
        let errors = observed.errors_for(ALPHA);
        assert_eq!(
            errors.len(),
            1,
            "{case}: one diagnostic transition: {observed:?}"
        );
        assert_safe_diagnostic(errors[0], ALPHA, case);
        assert!(
            observed.process_errors.is_empty(),
            "{case}: no global Error"
        );
        assert!(
            observed.dependency_blocked.is_empty(),
            "{case}: not a dependency wait: {observed:?}"
        );
        assert_eq!(observed.dispatches_of(ALPHA), 0, "{case}: no Apply");
        assert_eq!(
            harness.analyzed(ALPHA),
            0,
            "{case}: never an analyzer target"
        );

        harness.adopt(&observed).await;
        assert_eq!(harness.status(ALPHA).await, "error", "{case}");
        let detail = harness.error_detail(ALPHA).await.expect("error_detail");
        assert_safe_diagnostic(&detail, ALPHA, case);
        {
            let guard = harness.state.read().await;
            assert!(
                !guard.all_blocker_views().contains_key(ALPHA),
                "{case}: no blocker, self-dependency or external prerequisite"
            );
        }

        // Unchanged observations after adoption: no new transition.
        for _ in 0..3 {
            harness.pass().await;
        }
        let repeat = harness.drain();
        assert!(repeat.processing_errors.is_empty(), "{case}: {repeat:?}");
        assert_eq!(repeat.dispatches_of(ALPHA), 0, "{case}");
        assert_eq!(harness.status(ALPHA).await, "error", "{case}");

        assert!(harness.marks.is_marked(BETA), "{case}: unrelated mark kept");
        assert_eq!(harness.status(BETA).await, "not queued", "{case}");
        harness.shutdown().await;
    }
}

/// Mixed queue: the invalid row errors, the valid sibling is analyzed and
/// dispatched once, and a genuine dependency wait stays a dependency wait.
#[tokio::test(start_paused = true)]
async fn queued_proposal_diagnostic_mixed_queue_keeps_valid_work_moving() {
    let invalid = format!("---\ndependencies:\n  token: {SENTINEL}\n---\n# Bad\r\n");
    let gamma = "---\ndependencies: [beta]\n---\n# Gamma\n";
    let mut harness = Harness::new(&[
        (ALPHA, Some(invalid.as_str())),
        (BETA, Some(VALID)),
        (GAMMA, Some(gamma)),
    ]);
    harness.admit(ALPHA).await;
    harness.admit(BETA).await;
    harness.admit(GAMMA).await;

    harness.pass().await;
    harness.pass().await;
    let observed = harness.drain();

    assert_eq!(observed.errors_for(ALPHA).len(), 1, "{observed:?}");
    assert!(observed.errors_for(BETA).is_empty());
    assert!(
        observed.errors_for(GAMMA).is_empty(),
        "a dependency wait is not an error"
    );
    assert!(observed.process_errors.is_empty(), "{observed:?}");
    assert_eq!(
        observed.dispatches_of(BETA),
        1,
        "valid sibling dispatched once"
    );
    assert_eq!(observed.dispatches_of(ALPHA), 0);
    assert_eq!(
        observed.dispatches_of(GAMMA),
        0,
        "gamma still waits on beta"
    );
    assert!(harness.analyzed(BETA) >= 1);
    assert_eq!(harness.analyzed(ALPHA), 0);
    assert!(
        observed
            .dependency_blocked
            .iter()
            .all(|(id, deps)| id == GAMMA && deps == &vec![BETA.to_string()]),
        "only gamma's genuine wait is a dependency block: {observed:?}"
    );
    harness.shutdown().await;
}

/// Fix-only stays errored; Retry after repair dispatches once; Retry of
/// still-invalid data returns to the same error without any execution.
#[tokio::test(start_paused = true)]
async fn queued_proposal_diagnostic_requires_explicit_retry_after_repair() {
    let invalid = format!("---\ndependencies: [{SENTINEL}\n---\n# Bad\n");
    let mut harness = Harness::new(&[(ALPHA, Some(invalid.as_str()))]);
    harness.admit(ALPHA).await;
    harness.pass().await;
    let first = harness.drain();
    assert_eq!(first.errors_for(ALPHA).len(), 1);
    harness.adopt(&first).await;
    assert_eq!(harness.status(ALPHA).await, "error");

    // ── Still-invalid Retry: back to the same error, zero execution ────────
    harness.retry(ALPHA).await;
    assert_eq!(harness.status(ALPHA).await, "queued");
    harness.pass().await;
    harness.pass().await;
    let invalid_retry = harness.drain();
    assert_eq!(
        invalid_retry.errors_for(ALPHA).len(),
        1,
        "a still-invalid Retry publishes the error again: {invalid_retry:?}"
    );
    assert_eq!(
        invalid_retry.errors_for(ALPHA)[0],
        first.errors_for(ALPHA)[0]
    );
    assert_eq!(invalid_retry.dispatches_of(ALPHA), 0);
    assert_eq!(harness.analyzed(ALPHA), 0);
    harness.adopt(&invalid_retry).await;
    assert_eq!(harness.status(ALPHA).await, "error");

    // ── Fix only: committed repair does not restart the row ───────────────
    write_proposal(&harness.repo_root, ALPHA, Some(VALID));
    commit_all(&harness.repo_root, "Fix alpha proposal");
    for _ in 0..3 {
        harness.pass().await;
    }
    let fix_only = harness.drain();
    assert!(fix_only.processing_errors.is_empty(), "{fix_only:?}");
    assert_eq!(fix_only.dispatches_of(ALPHA), 0, "no silent restart");
    assert_eq!(harness.status(ALPHA).await, "error");

    // ── Explicit Retry after repair: dispatched exactly once ──────────────
    harness.retry(ALPHA).await;
    harness.pass().await;
    harness.pass().await;
    let recovered = harness.drain();
    assert!(recovered.processing_errors.is_empty(), "{recovered:?}");
    assert_eq!(recovered.dispatches_of(ALPHA), 1, "{recovered:?}");
    assert!(recovered.process_errors.is_empty());
    harness.shutdown().await;
}

/// A rebuilt reducer projection that lost the error is reconciled from the
/// repository again: the epoch is a dedup cache, not current-state authority.
#[tokio::test(start_paused = true)]
async fn queued_proposal_diagnostic_rebuilt_projection_is_reconciled_from_repository() {
    let invalid = "---\n# unclosed\n".to_string();
    let mut harness = Harness::new(&[(ALPHA, Some(invalid.as_str()))]);
    harness.admit(ALPHA).await;
    harness.pass().await;
    let first = harness.drain();
    harness.adopt(&first).await;
    // The scheduler observes adoption on a later pass.
    harness.pass().await;
    assert!(harness.drain().processing_errors.is_empty());

    // Replace the reducer with a reconstruction that never saw the error.
    let rebuilt = Arc::new(RwLock::new(OrchestratorState::new(
        vec![ALPHA.to_string()],
        10,
    )));
    rebuilt
        .write()
        .await
        .apply_command(ReducerCommand::AddToQueue(ALPHA.to_string()));
    harness
        .executor
        .set_shared_orchestrator_state(rebuilt.clone());
    harness.state = rebuilt;
    if !harness.queued.iter().any(|change| change.id == ALPHA) {
        harness.queued.push(test_change(ALPHA));
    }
    assert_eq!(harness.status(ALPHA).await, "queued");

    harness.pass().await;
    harness.pass().await;
    let reconciled = harness.drain();
    assert_eq!(
        reconciled.errors_for(ALPHA).len(),
        1,
        "the rebuilt projection is not left plain queued: {reconciled:?}"
    );
    assert_eq!(reconciled.dispatches_of(ALPHA), 0);
    harness.adopt(&reconciled).await;
    assert_eq!(harness.status(ALPHA).await, "error");
    harness.shutdown().await;
}

/// Reader controls: valid empty, null and absent frontmatter are not newly
/// rejected, and a genuine dependency is still a dependency wait.
#[tokio::test(start_paused = true)]
async fn queued_proposal_diagnostic_valid_reader_controls_are_unchanged() {
    let controls = [
        ("empty-block", "---\n---\n# Change\n"),
        ("empty-block-crlf", "---\r\n---\r\n# Change\r\n"),
        ("no-frontmatter", "# Legacy proposal\n"),
        ("null-root", "---\n~\n---\n# Change\n"),
    ];
    let mut proposals: Vec<(&str, Option<&str>)> = controls
        .iter()
        .map(|(id, content)| (*id, Some(*content)))
        .collect();
    proposals.push((GAMMA, Some("---\ndependencies: [beta]\n---\n# Gamma\n")));
    proposals.push((BETA, Some(VALID)));
    let mut harness = Harness::new(&proposals);
    for (id, _) in controls {
        harness.admit(id).await;
    }
    harness.admit(GAMMA).await;
    harness.admit(BETA).await;

    let classification = harness
        .executor
        .classify_queued_work(&harness.queued, &HashSet::new())
        .await;
    for (id, _) in controls {
        assert_eq!(
            classification.class_for(id),
            Some(QueuedWorkClass::DispatchableApply),
            "{id} keeps its reader behavior"
        );
    }
    assert_eq!(
        classification.class_for(GAMMA),
        Some(QueuedWorkClass::DependencyBlocked)
    );
    assert!(classification.proposal_metadata_error.is_empty());
    harness.shutdown().await;
}

/// The late dispatch guard: a proposal broken after the pre-analysis check
/// uses the same bounded change-local contract, never a self-dependency.
#[tokio::test(start_paused = true)]
async fn queued_proposal_diagnostic_late_dispatch_race_uses_same_contract() {
    let mut harness = Harness::new(&[(ALPHA, Some(VALID))]);
    harness.admit(ALPHA).await;
    write_proposal(
        &harness.repo_root,
        ALPHA,
        Some(&format!("---\ndependencies:\n  k: {SENTINEL}\n---\n")),
    );
    let analysis = AnalysisResult {
        order: vec![ALPHA.to_string()],
        dependencies: HashMap::new(),
        groups: None,
    };
    for _ in 0..2 {
        assert!(harness
            .executor
            .select_changes_for_dispatch(&analysis, 1, &HashSet::new())
            .await
            .is_empty());
    }
    let observed = harness.drain();
    let errors = observed.errors_for(ALPHA);
    assert_eq!(errors.len(), 1, "{observed:?}");
    assert_safe_diagnostic(errors[0], ALPHA, "late");
    assert!(observed.process_errors.is_empty());
    assert!(observed.dependency_blocked.is_empty());
    {
        let guard = harness.state.read().await;
        assert!(!guard.all_blocker_views().contains_key(ALPHA));
    }
    harness.shutdown().await;
}

//! `cflx openspec verify` — runtime-supervised repository-local verification.
//!
//! This is the request path the design leaves open: Apply (or an operator) asks
//! for an execution, and the *runtime* performs it. That distinction is the
//! whole reason the command exists. An agent that runs `cargo test` itself
//! produces output nobody can bind to a commit; the same command run through
//! here is started, waited on, hashed, and recorded by the process that also
//! reads Git — so Acceptance can later prove the result belongs to exactly this
//! tree instead of taking a claim on faith.
//!
//! Two modes, one decision procedure:
//!
//! * `--plan` reports what Acceptance would decide right now and executes
//!   nothing.
//! * The default runs only what cannot be reused, so a second invocation against
//!   an unchanged tree is nearly free and an invocation after any real change
//!   runs the command again.
//!
//! Nothing here is a verdict. A failing command exits non-zero with its artifact
//! path, and that is a *command* result: whether the change is acceptable stays
//! an Acceptance question.

use std::path::{Path, PathBuf};

use crate::archive_layout;
use crate::openspec::VerificationDeclaration;
use crate::orchestration::acceptance::verification_evidence::{
    decide_reuse, eligible_request, plan_reuse, CaptureOutcome, GitRepositoryFacts, ReusePolicy,
    RuntimeVerificationExecutor, SystemClock, VerificationRequest,
};

/// Exit status when at least one selected verification neither reused nor
/// captured successfully.
const EXIT_UNSATISFIED: i32 = 1;

/// Resolve the proposal that declares one change's verifications.
///
/// The active proposal wins unconditionally: an archive entry sharing the ID is
/// a previously archived change of that name, never a competing declaration
/// source for a live one. Only when no active proposal exists does the archive
/// answer, through the repository's own archive identity rules — and it answers
/// with exactly one entry or with a refusal.
fn proposal_path(workspace: &Path, change_id: &str) -> Result<PathBuf, String> {
    let active = workspace
        .join(archive_layout::ACTIVE_CHANGES_PREFIX)
        .join(change_id)
        .join("proposal.md");
    if active.is_file() {
        return Ok(active);
    }
    let archive_dir = workspace.join(archive_layout::ARCHIVE_PREFIX);
    match archive_layout::find_archived_proposal(change_id, &archive_dir) {
        Ok(Some(archived)) => Ok(archived),
        Ok(None) => Err(format!(
            "Change '{change_id}' has no proposal at {} and no archived proposal under {}",
            active.display(),
            archive_dir.display()
        )),
        Err(error) => Err(error.message()),
    }
}

/// Read the declarations of one change from the workspace.
fn declarations(workspace: &Path, change_id: &str) -> Result<Vec<VerificationDeclaration>, String> {
    let proposal = proposal_path(workspace, change_id)?;
    Ok(crate::openspec::parse_proposal_metadata_from_file(&proposal).verifications)
}

/// Keep only the declaration the operator named, if they named one.
fn select(
    declarations: Vec<VerificationDeclaration>,
    verification_id: Option<&str>,
) -> Result<Vec<VerificationDeclaration>, String> {
    let Some(wanted) = verification_id else {
        return Ok(declarations);
    };
    let selected: Vec<_> = declarations
        .into_iter()
        .filter(|declaration| declaration.id.as_deref().map(str::trim) == Some(wanted))
        .collect();
    if selected.is_empty() {
        return Err(format!(
            "verification-id '{wanted}' is not declared in proposal.md verifications"
        ));
    }
    Ok(selected)
}

/// Report the reuse plan without executing anything.
async fn report_plan(
    workspace: &Path,
    declarations: &[VerificationDeclaration],
    json_output: bool,
) -> Result<(), String> {
    let plan = plan_reuse(
        &GitRepositoryFacts,
        workspace,
        declarations,
        ReusePolicy::default(),
    )
    .await;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&plan.to_json()).unwrap_or_default()
        );
        return Ok(());
    }
    if plan.is_empty() {
        println!("No repository-local verifications are declared.");
        return Ok(());
    }
    for decision in &plan.decisions {
        println!("{}", decision.summary());
    }
    Ok(())
}

/// Run everything that cannot be reused, then report per verification.
async fn run_verifications(
    workspace: &Path,
    declarations: &[VerificationDeclaration],
    json_output: bool,
) -> Result<i32, String> {
    let facts = GitRepositoryFacts;
    let policy = ReusePolicy::default();
    let mut reports = Vec::new();
    let mut unsatisfied = 0;

    for declaration in declarations {
        // A declaration with no local reuse decision is not something this
        // command runs at all: deployed, credentialed, and device observations
        // are owned elsewhere by design.
        let Some(decision) = decide_reuse(&facts, workspace, declaration, policy).await else {
            continue;
        };
        if decision.is_reuse() {
            reports.push(decision.to_json());
            continue;
        }
        let request: VerificationRequest = match eligible_request(declaration) {
            Ok(request) => request,
            Err(reason) => {
                // Not supervisable, and saying so is the whole report: the
                // declaration is what has to change.
                reports.push(serde_json::json!({
                    "verification_id": declaration.id.clone().unwrap_or_default(),
                    "outcome": "not_captured",
                    "reason": reason.code(),
                    "detail": reason.detail(),
                }));
                unsatisfied += 1;
                continue;
            }
        };
        let executor = RuntimeVerificationExecutor::new(
            workspace.to_path_buf(),
            GitRepositoryFacts,
            crate::orchestration::acceptance::verification_evidence::DirectCommandSupervisor,
            SystemClock,
        );
        let outcome = executor.capture(&request).await;
        if !matches!(outcome, CaptureOutcome::Captured(_)) {
            unsatisfied += 1;
        }
        reports.push(outcome.to_json());
    }

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"verifications": reports}))
                .unwrap_or_default()
        );
    } else if reports.is_empty() {
        println!("No repository-local verifications are declared.");
    } else {
        for report in &reports {
            println!(
                "{}: {}{}",
                report["verification_id"].as_str().unwrap_or_default(),
                report["outcome"].as_str().unwrap_or_default(),
                report["detail"]
                    .as_str()
                    .map(|detail| format!(" — {detail}"))
                    .unwrap_or_default()
            );
        }
    }
    Ok(if unsatisfied == 0 {
        0
    } else {
        EXIT_UNSATISFIED
    })
}

/// Entry point for `cflx openspec verify`.
///
/// Returns the process exit status: `0` when every selected verification is
/// reused or freshly captured, and [`EXIT_UNSATISFIED`] when at least one is
/// neither.
pub async fn cmd_verify(
    change_id: &str,
    verification_id: Option<&str>,
    plan_only: bool,
    json_output: bool,
) -> Result<i32, String> {
    let workspace = std::env::current_dir()
        .map_err(|error| format!("Failed to resolve the current directory: {error}"))?;
    let workspace: PathBuf = workspace;
    let selected = select(declarations(&workspace, change_id)?, verification_id)?;
    if plan_only {
        report_plan(&workspace, &selected, json_output).await?;
        return Ok(0);
    }
    run_verifications(&workspace, &selected, json_output).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::acceptance::verification_evidence::{
        EvidenceDefect, RepositoryFacts, RerunReason, ReuseDecision, ToolIdentity,
    };

    fn declaration(id: &str) -> VerificationDeclaration {
        VerificationDeclaration {
            id: Some(id.to_string()),
            execution_class: Some("repository-local".to_string()),
            completion_role: Some("change-blocking".to_string()),
            automation: Some("Cargo.toml".to_string()),
            evidence: Some("cargo test".to_string()),
            rerun: Some("cargo test".to_string()),
            ..VerificationDeclaration::default()
        }
    }

    #[test]
    fn selection_keeps_only_the_named_verification() {
        let selected = select(
            vec![declaration("alpha"), declaration("beta")],
            Some("beta"),
        )
        .expect("selection succeeds");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id.as_deref(), Some("beta"));
    }

    #[test]
    fn selection_refuses_an_undeclared_verification_id() {
        let error = select(vec![declaration("alpha")], Some("missing")).unwrap_err();
        assert!(error.contains("is not declared"));
    }

    #[test]
    fn selection_without_a_filter_keeps_every_declaration() {
        let selected = select(vec![declaration("alpha"), declaration("beta")], None).unwrap();
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn a_change_without_a_proposal_is_an_error_rather_than_an_empty_plan() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let error = declarations(workspace.path(), "absent-change").unwrap_err();
        assert!(error.contains("has no proposal"));
    }

    #[test]
    fn declarations_are_read_from_the_workspace_proposal() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let change_dir = workspace.path().join("openspec/changes/example");
        std::fs::create_dir_all(&change_dir).expect("create change dir");
        std::fs::write(
            change_dir.join("proposal.md"),
            "---\nverifications:\n  - id: local-tests\n    phase: pre-integration\n    execution_class: repository-local\n    completion_role: change-blocking\n    prerequisites: []\n---\n# Example\n",
        )
        .expect("write proposal");

        let declared = declarations(workspace.path(), "example").expect("declarations");
        assert_eq!(declared.len(), 1);
        assert_eq!(declared[0].id.as_deref(), Some("local-tests"));
    }

    /// A proposal declaring one supervisable repository-local verification.
    const PROPOSAL: &str = "---\nverifications:\n  - id: local-tests\n    phase: pre-integration\n    automation: Cargo.toml\n    rerun: cargo test --lib\n    execution_class: repository-local\n    completion_role: change-blocking\n    prerequisites: []\n---\n# Example\n";

    /// Write `PROPOSAL` at a workspace-relative directory.
    fn write_proposal(workspace: &Path, relative_dir: &str) {
        let dir = workspace.join(relative_dir);
        std::fs::create_dir_all(&dir).expect("create proposal dir");
        std::fs::write(dir.join("proposal.md"), PROPOSAL).expect("write proposal");
    }

    /// Repository facts that are never consulted: every decision in these tests
    /// is settled by the absent evidence sidecar before Git is observed at all.
    struct UnobservedFacts;

    #[async_trait::async_trait]
    impl RepositoryFacts for UnobservedFacts {
        async fn head_commit(&self, _workspace: &Path) -> Result<String, String> {
            Err("repository facts must not be observed here".to_string())
        }
        async fn head_tree(&self, _workspace: &Path) -> Result<String, String> {
            Err("repository facts must not be observed here".to_string())
        }
        async fn tracked_blob_oid(&self, _workspace: &Path, _path: &str) -> Result<String, String> {
            Err("repository facts must not be observed here".to_string())
        }
        async fn hash_file(&self, _workspace: &Path, _path: &Path) -> Result<String, String> {
            Err("repository facts must not be observed here".to_string())
        }
        async fn porcelain_status(&self, _workspace: &Path) -> Result<String, String> {
            Err("repository facts must not be observed here".to_string())
        }
        async fn resolve_tool(
            &self,
            _workspace: &Path,
            _program: &str,
        ) -> Result<ToolIdentity, String> {
            Err("repository facts must not be observed here".to_string())
        }
    }

    #[tokio::test]
    async fn an_archived_change_keeps_its_verification_id_and_repository_level_automation() {
        let workspace = tempfile::tempdir().expect("tempdir");
        write_proposal(
            workspace.path(),
            "openspec/changes/archive/2026-07-09-example",
        );

        // The declaration source moved; the declaration itself did not.
        assert_eq!(
            proposal_path(workspace.path(), "example").expect("archived proposal"),
            workspace
                .path()
                .join("openspec/changes/archive/2026-07-09-example/proposal.md")
        );
        let declared = declarations(workspace.path(), "example").expect("declarations");
        assert_eq!(declared.len(), 1);
        assert_eq!(declared[0].id.as_deref(), Some("local-tests"));

        // Automation stays repository-relative and runs from the workspace root,
        // never from the archived directory.
        let request = eligible_request(&declared[0]).expect("supervisable request");
        assert_eq!(request.verification_id, "local-tests");
        assert_eq!(request.automation_path, "Cargo.toml");
        assert_eq!(request.cwd, ".");
        assert_eq!(request.argv, vec!["cargo", "test", "--lib"]);

        // And the archived declaration still plans under the same ID.
        let plan = plan_reuse(
            &UnobservedFacts,
            workspace.path(),
            &declared,
            ReusePolicy::default(),
        )
        .await;
        assert_eq!(plan.decisions.len(), 1);
        assert_eq!(plan.decisions[0].verification_id(), "local-tests");
    }

    #[test]
    fn an_archived_change_resolves_from_the_direct_entry_too() {
        let workspace = tempfile::tempdir().expect("tempdir");
        write_proposal(workspace.path(), "openspec/changes/archive/example");

        assert_eq!(
            proposal_path(workspace.path(), "example").expect("archived proposal"),
            workspace
                .path()
                .join("openspec/changes/archive/example/proposal.md")
        );
    }

    #[test]
    fn an_active_proposal_outranks_a_same_named_archive_entry() {
        let workspace = tempfile::tempdir().expect("tempdir");
        write_proposal(workspace.path(), "openspec/changes/example");
        write_proposal(
            workspace.path(),
            "openspec/changes/archive/2026-07-09-example",
        );
        write_proposal(workspace.path(), "openspec/changes/archive/example");

        // Two archive entries would be ambiguous on their own; with an active
        // proposal present they are not consulted at all.
        assert_eq!(
            proposal_path(workspace.path(), "example").expect("active proposal"),
            workspace
                .path()
                .join("openspec/changes/example/proposal.md")
        );
        assert_eq!(
            declarations(workspace.path(), "example")
                .expect("declarations")
                .len(),
            1
        );
    }

    #[test]
    fn competing_archive_entries_fail_closed_instead_of_picking_one() {
        let workspace = tempfile::tempdir().expect("tempdir");
        write_proposal(
            workspace.path(),
            "openspec/changes/archive/2026-07-09-example",
        );
        write_proposal(workspace.path(), "openspec/changes/archive/example");

        let error = declarations(workspace.path(), "example").unwrap_err();
        assert!(error.contains("Ambiguous archive for 'example'"));
        assert!(error.contains("2026-07-09-example"));
    }

    #[test]
    fn a_nested_archive_layout_reports_the_repository_diagnostic() {
        let workspace = tempfile::tempdir().expect("tempdir");
        write_proposal(
            workspace.path(),
            "openspec/changes/archive/2026-07-09/example",
        );

        let error = declarations(workspace.path(), "example").unwrap_err();
        assert!(error.contains("Invalid archive layout for 'example'"));
        assert!(error.contains("2026-07-09/example"));
    }

    #[test]
    fn non_canonical_or_proposal_less_archive_entries_never_resolve() {
        let workspace = tempfile::tempdir().expect("tempdir");
        // A canonical entry without a proposal, a suffix collision, a malformed
        // date, and another change's entry: none of them declare 'example'.
        std::fs::create_dir_all(workspace.path().join("openspec/changes/archive/example"))
            .expect("create empty entry");
        write_proposal(workspace.path(), "openspec/changes/archive/prefix-example");
        write_proposal(
            workspace.path(),
            "openspec/changes/archive/2026-7-9-example",
        );
        write_proposal(workspace.path(), "openspec/changes/archive/other-change");

        let error = declarations(workspace.path(), "example").unwrap_err();
        assert!(error.contains("has no proposal"));
        assert!(error.contains("no archived proposal"));
    }

    #[test]
    fn a_reuse_decision_renders_one_operator_line() {
        let decision = ReuseDecision::Rerun {
            verification_id: "local-tests".to_string(),
            reason: RerunReason::Defect(EvidenceDefect::Missing),
        };
        let line = decision.summary();
        assert!(line.starts_with("local-tests: rerun"));
        assert!(line.contains("evidence_missing"));
    }
}

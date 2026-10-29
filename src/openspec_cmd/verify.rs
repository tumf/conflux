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
//!
//! Like every other Acceptance writer, the envelope and its artifact go into the
//! Conflux-owned external store rather than into the repository being verified.
//! The store is resolved and proven before the first command starts, so an
//! unusable one is reported instead of silently producing an unrecordable run —
//! and a repository that still carries legacy `.cflx/verification-evidence`
//! content is reported rather than read, migrated, or cleaned up.

use std::path::{Path, PathBuf};

use crate::archive_layout;
use crate::openspec::{VerificationDeclaration, VerificationSource};
use crate::orchestration::acceptance::evidence_location::{prepare_store, AcceptanceStore};
use crate::orchestration::acceptance::execution_manifest::plan_verification_source;
use crate::orchestration::acceptance::verification_evidence::{
    decide_reuse, eligible_request, plan_reuse, CaptureOutcome, DeclarationDefect, EvidenceStore,
    GitRepositoryFacts, ReusePolicy, RuntimeVerificationExecutor, SystemClock, VerificationRequest,
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

/// Read the declaration source of one change from the workspace.
///
/// Through the fallible reader, never the tolerant display parser: structural
/// metadata defects travel with the answer, and a proposal that cannot be read
/// is the same proposal-resolution error as one that does not exist.
fn declarations(workspace: &Path, change_id: &str) -> Result<VerificationSource, String> {
    let proposal = proposal_path(workspace, change_id)?;
    crate::openspec::read_verification_source(&proposal).map_err(|error| {
        format!(
            "Change '{change_id}' proposal at {} could not be read ({:?})",
            error.path.display(),
            error.kind
        )
    })
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

/// Wrap per-verification reports in the envelope an operator can resolve paths
/// against.
///
/// Every path inside a report is store-relative, because the envelope on disk
/// has to survive a state root that moves. A report is read from wherever the
/// operator happens to be standing, so the resolved store root travels with it:
/// `store_root` plus `artifact_path` names a real file, and `artifact_path`
/// alone names nothing. The store-relative spelling is left exactly as the
/// envelope records it.
fn report_envelope(store: &EvidenceStore, body: serde_json::Value) -> serde_json::Value {
    let mut envelope = body;
    if let Some(object) = envelope.as_object_mut() {
        object.insert(
            "store_root".to_string(),
            serde_json::Value::String(store.root().display().to_string()),
        );
    }
    envelope
}

/// Report the reuse plan without executing anything.
async fn report_plan(
    workspace: &Path,
    store: &EvidenceStore,
    declarations: &[VerificationDeclaration],
    json_output: bool,
) -> Result<(), String> {
    let plan = plan_reuse(
        &GitRepositoryFacts,
        workspace,
        store,
        declarations,
        ReusePolicy::default(),
    )
    .await;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&report_envelope(store, plan.to_json()))
                .unwrap_or_default()
        );
        return Ok(());
    }
    if plan.is_empty() {
        println!("No repository-local verifications are declared.");
        return Ok(());
    }
    println!("Evidence store: {}", store.root().display());
    for decision in &plan.decisions {
        println!("{}", decision.summary());
    }
    Ok(())
}

/// Run everything that cannot be reused, then report per verification.
async fn run_verifications(
    workspace: &Path,
    store: &EvidenceStore,
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
        let Some(decision) = decide_reuse(&facts, workspace, store, declaration, policy).await
        else {
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
            store.clone(),
            GitRepositoryFacts,
            crate::orchestration::acceptance::verification_evidence::DirectCommandSupervisor,
            SystemClock,
        );
        let outcome = executor.capture(&request).await;
        if !matches!(outcome, CaptureOutcome::Captured { .. }) {
            unsatisfied += 1;
        }
        reports.push(outcome.to_json());
    }

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&report_envelope(
                store,
                serde_json::json!({"verifications": reports})
            ))
            .unwrap_or_default()
        );
    } else if reports.is_empty() {
        println!("No repository-local verifications are declared.");
    } else {
        println!("Evidence store: {}", store.root().display());
        for report in &reports {
            println!(
                "{}: {}{}{}",
                report["verification_id"].as_str().unwrap_or_default(),
                report["outcome"].as_str().unwrap_or_default(),
                report["detail"]
                    .as_str()
                    .map(|detail| format!(" — {detail}"))
                    .unwrap_or_default(),
                // The openable path, not the store-relative one: a failing gate's
                // output is worth nothing if reading it needs a second lookup.
                report["artifact_location"]
                    .as_str()
                    .map(|location| format!(" — {location}"))
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

/// Report an invalid required declaration set.
///
/// Printed instead of a plan or a run: when the required set as a whole is
/// invalid, no declared command — not even a valid one — is executed, and no
/// evidence record is read or written.
fn report_declaration_defects(change_id: &str, defects: &[DeclarationDefect], json_output: bool) {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "change_id": change_id,
                "declaration_defects": defects
                    .iter()
                    .map(DeclarationDefect::to_json)
                    .collect::<Vec<_>>(),
                "verifications": [],
            }))
            .unwrap_or_default()
        );
        return;
    }
    println!(
        "Required verification declarations of '{change_id}' are invalid; nothing was executed."
    );
    for defect in defects {
        println!("{change_id}: {}", defect.summary());
    }
}

/// Entry point for `cflx openspec verify`.
///
/// Returns the process exit status: `0` when every selected verification is
/// reused or freshly captured, and [`EXIT_UNSATISFIED`] when at least one is
/// neither — or when the proposal's required declarations are invalid.
pub async fn cmd_verify(
    change_id: &str,
    verification_id: Option<&str>,
    plan_only: bool,
    json_output: bool,
) -> Result<i32, String> {
    let workspace = std::env::current_dir()
        .map_err(|error| format!("Failed to resolve the current directory: {error}"))?;
    verify_in_workspace(
        &workspace,
        change_id,
        verification_id,
        plan_only,
        json_output,
    )
    .await
}

/// [`cmd_verify`] against an explicit workspace.
pub(crate) async fn verify_in_workspace(
    workspace: &Path,
    change_id: &str,
    verification_id: Option<&str>,
    plan_only: bool,
    json_output: bool,
) -> Result<i32, String> {
    let workspace: PathBuf = workspace.to_path_buf();
    let source = declarations(&workspace, change_id)?;
    // The whole proposal is planned before any selection: source defects,
    // duplicates, and the required set are properties of the list, so
    // `--verification-id` cannot narrow defect evaluation. Defects stop here,
    // before the store is even resolved, so no evidence is read for reuse or
    // modified.
    let plan = plan_verification_source(&source);
    if !plan.is_valid() {
        report_declaration_defects(change_id, &plan.defects, json_output);
        return Ok(EXIT_UNSATISFIED);
    }
    let selected = select(source.declarations, verification_id)?;
    let store = resolve_store(&workspace, change_id)?;
    if plan_only {
        report_plan(&workspace, &store.evidence(), &selected, json_output).await?;
        return Ok(0);
    }
    run_verifications(&workspace, &store.evidence(), &selected, json_output).await
}

/// Resolve the Conflux-owned external store this command reads and writes.
///
/// `cflx openspec verify` runs against the repository the operator is standing
/// in, so the project and the workspace are the same directory; the store is
/// still resolved outside both, and a refusal is reported as an error rather
/// than worked around.
fn resolve_store(workspace: &Path, change_id: &str) -> Result<AcceptanceStore, String> {
    // Storage settings only: this command starts no AI subprocess, so an
    // installation that has not configured its commands yet must still be able
    // to resolve the state root its evidence belongs in.
    let state_base_dir = crate::config::OrchestratorConfig::load_storage_settings(None)
        .ok()
        .and_then(|config| config.get_state_base_dir().map(str::to_string));
    prepare_store(state_base_dir.as_deref(), workspace, workspace, change_id).map_err(|refusal| {
        format!(
            "Verification evidence cannot be stored outside the repository: {}",
            refusal.evidence()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::acceptance::execution_manifest::plan_declared_gates;
    use crate::orchestration::acceptance::verification_evidence::{
        EvidenceDefect, RepositoryFacts, RerunReason, ReuseDecision, ToolIdentity,
        VerificationReusePlan,
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

        let declared = declarations(workspace.path(), "example")
            .expect("declarations")
            .declarations;
        assert_eq!(declared.len(), 1);
        assert_eq!(declared[0].id.as_deref(), Some("local-tests"));
    }

    /// `verify --plan` reports for a declaration carrying a leading assignment
    /// name the binding and never its value, in text and JSON alike; an invalid
    /// assignment is a typed defect whose report carries no value either.
    #[tokio::test]
    async fn rerun_leading_env_verify_reports_never_carry_a_binding_value() {
        const SENTINEL: &str = "rerun-leading-env-sentinel-2d6f08";
        let workspace = tempfile::tempdir().expect("tempdir");
        let with = |command: String| VerificationDeclaration {
            rerun: Some(command.clone()),
            evidence: Some(command),
            phase: Some("pre-integration".to_string()),
            ..declaration("local-tests")
        };

        let declared = vec![with(format!(
            "DJANGO_SETTINGS_MODULE={SENTINEL} cargo test"
        ))];
        assert!(plan_declared_gates(&declared).is_valid());
        let plan = plan_reuse(
            &UnobservedFacts,
            workspace.path(),
            &EvidenceStore::new(workspace.path().join("../external-acceptance-store")),
            &declared,
            ReusePolicy::default(),
        )
        .await;
        assert_eq!(plan.decisions.len(), 1);
        for decision in &plan.decisions {
            assert!(!decision.summary().contains(SENTINEL));
        }
        assert!(!plan.to_json().to_string().contains(SENTINEL));

        for command in [
            format!("1BAD={SENTINEL} cargo test"),
            format!("A={SENTINEL}"),
        ] {
            let defects = plan_declared_gates(&[with(command)]).defects;
            assert_eq!(defects.len(), 1);
            assert!(matches!(
                defects[0].code.as_str(),
                "declaration_invalid_assignment" | "declaration_missing_command"
            ));
            assert!(!defects[0].summary().contains(SENTINEL));
            assert!(!defects[0].to_json().to_string().contains(SENTINEL));
        }
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
        let declared = declarations(workspace.path(), "example")
            .expect("declarations")
            .declarations;
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
            &EvidenceStore::new(workspace.path().join("../external-acceptance-store")),
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
                .declarations
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

    /// Write one captured artifact into an external store, the way a real
    /// capture would, and hand back the store bound to it.
    fn store_with_artifact(root: &Path, verification_id: &str, output: &[u8]) -> EvidenceStore {
        let store = EvidenceStore::new(root.join("cflx-state/acceptance/project/workspace/change"));
        store.ensure_directory().expect("create store directory");
        std::fs::write(store.artifact_path(verification_id), output).expect("write artifact");
        store
    }

    #[test]
    fn a_json_report_lets_a_caller_locate_the_captured_artifact() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = store_with_artifact(temp.path(), "local-tests", b"error: 1 test failed");
        let outcome = CaptureOutcome::CommandFailed {
            verification_id: "local-tests".to_string(),
            exit_code: 101,
            artifact_path: EvidenceStore::artifact_relative_path("local-tests"),
            artifact_location: store.artifact_path("local-tests"),
        };

        let report = report_envelope(
            &store,
            serde_json::json!({"verifications": [outcome.to_json()]}),
        );

        // Route one: the absolute location travels on the per-verification
        // report and opens directly.
        let entry = &report["verifications"][0];
        let location = entry["artifact_location"]
            .as_str()
            .expect("a failing verification names an openable artifact");
        assert_eq!(
            std::fs::read(location).expect("the reported location opens"),
            b"error: 1 test failed"
        );

        // Route two: the store-relative spelling the envelope schema requires is
        // unchanged, and the envelope's own store root is what resolves it — so
        // a reused decision, which carries no absolute path of its own, is
        // locatable too.
        let root = report["store_root"]
            .as_str()
            .expect("the report names the resolved store root");
        assert_eq!(root, store.root().display().to_string());
        let relative = entry["artifact_path"]
            .as_str()
            .expect("the store-relative artifact path is preserved");
        assert_eq!(relative, "gates/local-tests.log");
        assert_eq!(
            std::fs::read(Path::new(root).join(relative))
                .expect("store_root + artifact_path opens"),
            b"error: 1 test failed"
        );
    }

    #[test]
    fn a_plan_report_carries_the_store_root_it_was_resolved_against() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = store_with_artifact(temp.path(), "local-tests", b"test result: ok");
        let plan = VerificationReusePlan {
            decisions: vec![ReuseDecision::Rerun {
                verification_id: "local-tests".to_string(),
                reason: RerunReason::Defect(EvidenceDefect::Missing),
            }],
        };

        let report = report_envelope(&store, plan.to_json());
        assert_eq!(
            report["store_root"].as_str(),
            Some(store.root().display().to_string().as_str())
        );
        // Wrapping adds the root and nothing else: the plan body is untouched.
        assert_eq!(report["verifications"], plan.to_json()["verifications"]);
        assert_eq!(report["rerun"], serde_json::json!(["local-tests"]));
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

    fn write_proposal_content(workspace: &Path, change: &str, content: &str) {
        let dir = workspace.join("openspec/changes").join(change);
        std::fs::create_dir_all(&dir).expect("create proposal dir");
        std::fs::write(dir.join("proposal.md"), content).expect("write proposal");
    }

    /// A wrong-typed field or an unclosed frontmatter is a source defect that
    /// stops `verify` before the store is resolved — with or without
    /// `--verification-id` — never a successful empty plan.
    #[tokio::test]
    async fn verification_declaration_integrity_verify_refuses_defective_sources() {
        for content in [
            "---\nverifications:\n  - id: local-tests\n    rerun: [cargo, test]\n    completion_role: change-blocking\n---\n# Example\n",
            "---\nverifications:\n  - id: local-tests\n# Example without a closing delimiter\n",
        ] {
            let workspace = tempfile::tempdir().expect("tempdir");
            write_proposal_content(workspace.path(), "example", content);

            let source = declarations(workspace.path(), "example").expect("readable");
            assert!(source.declarations.is_empty());
            assert_eq!(source.defects.len(), 1);
            let plan = plan_verification_source(&source);
            assert_eq!(plan.defects[0].code, "declaration_metadata_invalid");

            for verification_id in [None, Some("local-tests")] {
                for plan_only in [true, false] {
                    let status = verify_in_workspace(
                        workspace.path(),
                        "example",
                        verification_id,
                        plan_only,
                        true,
                    )
                    .await
                    .expect("defects are a reported outcome");
                    assert_eq!(status, EXIT_UNSATISFIED);
                }
            }
            let mut entries = std::fs::read_dir(workspace.path())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<Vec<_>>();
            entries.sort();
            assert_eq!(entries, vec![std::ffi::OsString::from("openspec")]);
        }
    }

    /// A proposal that cannot be resolved or read uses the proposal-resolution
    /// error route, never an empty declaration list.
    #[tokio::test]
    async fn verification_declaration_integrity_verify_unreadable_proposal_is_an_error() {
        let workspace = tempfile::tempdir().expect("tempdir");
        let error = verify_in_workspace(workspace.path(), "absent", None, true, true)
            .await
            .unwrap_err();
        assert!(error.contains("has no proposal"), "{error}");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            write_proposal_content(workspace.path(), "locked", PROPOSAL);
            let path = workspace.path().join("openspec/changes/locked/proposal.md");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
            // Privileged runners can read a 0000 file; the route is only
            // observable where the read genuinely fails.
            if std::fs::read(&path).is_err() {
                let error = verify_in_workspace(workspace.path(), "locked", None, true, true)
                    .await
                    .unwrap_err();
                assert!(error.contains("could not be read"), "{error}");
                assert!(error.contains("PermissionDenied"), "{error}");
            }
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
    }

    /// Legitimately empty sources keep their zero-defect plan.
    #[test]
    fn verification_declaration_integrity_verify_keeps_empty_sources_valid() {
        for content in [
            "# Example\n",
            "---\nverifications:\n---\n# Example\n",
            "---\nverifications: []\n---\n# Example\n",
        ] {
            let workspace = tempfile::tempdir().expect("tempdir");
            write_proposal_content(workspace.path(), "example", content);
            let source = declarations(workspace.path(), "example").expect("readable");
            assert!(plan_verification_source(&source).is_valid());
            assert!(source.declarations.is_empty());
        }
    }
}

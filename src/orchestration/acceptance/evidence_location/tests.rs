//! Unit coverage for the external Acceptance store resolver.
//!
//! Every case here is filesystem-scoped and in-memory otherwise: no Git, no
//! subprocess, no reviewer. The end-to-end proof that a real Acceptance
//! invocation writes nothing into a target lives in
//! `src/parallel/tests/acceptance_execution_boundary.rs`.

use super::*;

use tempfile::TempDir;

/// A target repository and a managed worktree that are genuinely separate
/// directories, as they are in a parallel run.
fn target_pair() -> (TempDir, TempDir) {
    (
        TempDir::new().expect("project tempdir"),
        TempDir::new().expect("workspace tempdir"),
    )
}

#[tokio::test]
async fn prepare_store_creates_an_external_directory_outside_every_target() {
    let external = TempDir::new().expect("external tempdir");
    let (project, workspace) = target_pair();

    let store = scoped_acceptance_root_for_test(external.path().to_path_buf(), async {
        prepare_store(None, project.path(), workspace.path(), "add-a-thing")
    })
    .await
    .expect("a root outside both targets resolves");

    assert!(store.root().is_dir(), "the store must be created up front");
    assert!(
        store.root().starts_with(external.path()),
        "the store must live under the resolved external root: {:?}",
        store.root()
    );
    assert!(
        !store.root().starts_with(project.path()) && !store.root().starts_with(workspace.path()),
        "constitutional law 4: nothing Conflux owns may live inside a target"
    );
    assert!(
        store
            .root()
            .file_name()
            .is_some_and(|name| name == "add-a-thing"),
        "the change is the leaf component: {:?}",
        store.root()
    );

    // Nothing was written into either target.
    for target in [project.path(), workspace.path()] {
        assert_eq!(
            std::fs::read_dir(target)
                .expect("read target")
                .filter_map(Result::ok)
                .count(),
            0,
            "resolving a store must not create anything in {target:?}"
        );
    }
}

/// The store, the evidence envelopes, and the manifest all agree on one root,
/// so there is no second location a writer could drift to.
#[tokio::test]
async fn every_writer_shares_the_one_resolved_root() {
    let external = TempDir::new().expect("external tempdir");
    let (project, workspace) = target_pair();

    let store = scoped_acceptance_root_for_test(external.path().to_path_buf(), async {
        prepare_store(None, project.path(), workspace.path(), "add-a-thing")
    })
    .await
    .expect("store resolves");

    assert_eq!(store.evidence().root(), store.root());
    assert_eq!(store.manifests().path(), store.root().join("manifest.json"));
    assert_eq!(
        store.evidence().artifact_path("focused-gate"),
        store.root().join("gates/focused-gate.log")
    );
}

/// An unusable external path is refused with the typed state-path category, and
/// — the part that matters — creates nothing in the target on the way out.
#[tokio::test]
async fn an_unwritable_root_refuses_without_touching_the_target() {
    let blocker = TempDir::new().expect("external tempdir");
    // A *file* where the root directory must be: `create_dir_all` cannot
    // succeed, and no amount of retrying will change that.
    let root = blocker.path().join("occupied");
    std::fs::write(&root, b"not a directory").expect("write blocker");
    let (project, workspace) = target_pair();

    let refusal = scoped_acceptance_root_for_test(root, async {
        prepare_store(None, project.path(), workspace.path(), "add-a-thing")
    })
    .await
    .expect_err("an uncreatable root must fail closed");

    assert_eq!(
        refusal.category(),
        crate::orchestration::acceptance::execution_manifest::AcceptanceHoldCategory::StatePathUnavailable
    );
    assert!(
        refusal.evidence().contains("acceptance_store_"),
        "the hold must name the resolver's own code: {}",
        refusal.evidence()
    );
    assert!(
        !workspace.path().join(".cflx").exists(),
        "a state-path failure must never fall back into the target"
    );
}

/// A root inside the managed worktree is refused rather than used, which is the
/// containment law stated as a test.
#[tokio::test]
async fn a_root_inside_the_target_is_refused() {
    let (project, workspace) = target_pair();

    let refusal = scoped_acceptance_root_for_test(workspace.path().join("acceptance"), async {
        prepare_store(None, project.path(), workspace.path(), "add-a-thing")
    })
    .await
    .expect_err("a root inside the workspace must fail closed");

    assert_eq!(
        refusal.category(),
        crate::orchestration::acceptance::execution_manifest::AcceptanceHoldCategory::StatePathUnavailable
    );
    assert!(
        refusal
            .evidence()
            .contains("acceptance_store_inside_target"),
        "{}",
        refusal.evidence()
    );
    assert!(
        !workspace.path().join("acceptance").exists(),
        "refusal must happen before anything is created"
    );
}

/// Legacy target-local evidence is detected by existence alone — including when
/// it is self-ignored — and is neither opened, parsed, migrated, nor deleted.
#[tokio::test]
async fn legacy_target_evidence_is_reported_and_left_alone() {
    let external = TempDir::new().expect("external tempdir");
    let (project, workspace) = target_pair();
    let legacy = workspace.path().join(".cflx/verification-evidence");
    std::fs::create_dir_all(&legacy).expect("create legacy dir");
    // The self-ignoring marker the old runtime wrote, plus a record. Neither may
    // be read back, and neither may be removed.
    std::fs::write(legacy.join(".gitignore"), "*\n").expect("write ignore");
    std::fs::write(legacy.join("focused-gate.json"), b"{\"schema\":\"v1\"}")
        .expect("write legacy record");

    let refusal = scoped_acceptance_root_for_test(external.path().to_path_buf(), async {
        prepare_store(None, project.path(), workspace.path(), "add-a-thing")
    })
    .await
    .expect_err("legacy target evidence must fail closed");

    assert_eq!(
        refusal.category(),
        crate::orchestration::acceptance::execution_manifest::AcceptanceHoldCategory::LegacyTargetEvidence
    );
    assert!(
        refusal.evidence().contains(".cflx/verification-evidence"),
        "the hold must name the path to clean up: {}",
        refusal.evidence()
    );
    assert!(
        legacy.join("focused-gate.json").is_file()
            && std::fs::read(legacy.join("focused-gate.json")).expect("read legacy")
                == b"{\"schema\":\"v1\"}",
        "legacy content must be left exactly as it was found"
    );
    assert!(legacy.join(".gitignore").is_file());
}

/// Only that one path is probed. An unrelated ignored directory is not the
/// runtime's business, and evaluating it would reintroduce the Conflux-specific
/// status filtering the constitution forbids.
#[tokio::test]
async fn unrelated_ignored_paths_are_not_evaluated() {
    let external = TempDir::new().expect("external tempdir");
    let (project, workspace) = target_pair();
    std::fs::create_dir_all(workspace.path().join("target/debug")).expect("create build dir");
    std::fs::create_dir_all(workspace.path().join("node_modules")).expect("create deps dir");
    std::fs::write(
        workspace.path().join(".gitignore"),
        "target\nnode_modules\n",
    )
    .expect("write ignore");

    scoped_acceptance_root_for_test(external.path().to_path_buf(), async {
        prepare_store(None, project.path(), workspace.path(), "add-a-thing")
    })
    .await
    .expect("unrelated ignored paths must not refuse anything");
}

/// A store is cache: dropping it is always permitted and never touches a target.
#[tokio::test]
async fn clearing_a_store_removes_only_the_external_directory() {
    let external = TempDir::new().expect("external tempdir");
    let (project, workspace) = target_pair();
    std::fs::write(workspace.path().join("deliverable.rs"), "fn main() {}\n")
        .expect("write deliverable");

    let store = scoped_acceptance_root_for_test(external.path().to_path_buf(), async {
        prepare_store(None, project.path(), workspace.path(), "add-a-thing")
    })
    .await
    .expect("store resolves");
    std::fs::create_dir_all(store.root().join("gates")).expect("create gates");
    std::fs::write(store.root().join("gates/focused-gate.log"), b"output").expect("write log");

    store.clear();

    assert!(!store.root().exists(), "the cache directory is removed");
    assert!(
        workspace.path().join("deliverable.rs").is_file(),
        "clearing out-of-worktree state must not modify the target worktree"
    );
}

/// The same inputs resolve to the same directory, which is what lets a later
/// attempt find the cache an earlier one wrote.
#[tokio::test]
async fn resolution_is_deterministic_and_isolated_per_change() {
    let external = TempDir::new().expect("external tempdir");
    let (project, workspace) = target_pair();

    let (first, again, other_change) =
        scoped_acceptance_root_for_test(external.path().to_path_buf(), async {
            (
                prepare_store(None, project.path(), workspace.path(), "alpha")
                    .expect("alpha resolves")
                    .root()
                    .to_path_buf(),
                prepare_store(None, project.path(), workspace.path(), "alpha")
                    .expect("alpha resolves again")
                    .root()
                    .to_path_buf(),
                prepare_store(None, project.path(), workspace.path(), "beta")
                    .expect("beta resolves")
                    .root()
                    .to_path_buf(),
            )
        })
        .await;

    assert_eq!(first, again);
    assert_ne!(first, other_change);
}

/// An unsafe change identifier never reaches the filesystem.
#[tokio::test]
async fn an_unsafe_change_id_is_refused_before_anything_is_created() {
    let external = TempDir::new().expect("external tempdir");
    let (project, workspace) = target_pair();

    let refusal = scoped_acceptance_root_for_test(external.path().to_path_buf(), async {
        prepare_store(None, project.path(), workspace.path(), "../escape")
    })
    .await
    .expect_err("a traversing change id must fail closed");

    assert!(refusal
        .evidence()
        .contains("acceptance_store_unsafe_component"));
    assert_eq!(
        std::fs::read_dir(external.path())
            .expect("read external root")
            .filter_map(Result::ok)
            .count(),
        0,
        "nothing may be created for a refused identifier"
    );
}

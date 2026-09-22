//! Manifest identity, storage, admission, and deadline-partition regressions.
//!
//! Every test here is named `acceptance_execution_boundary_*` so the proposal's
//! declared gate — `cargo test acceptance_execution_boundary --lib` — selects
//! exactly the coverage this change owns.

use super::*;
use crate::openspec::VerificationDeclaration;

fn declaration(
    id: &str,
    phase: &str,
    execution_class: &str,
    completion_role: &str,
    command: &str,
) -> VerificationDeclaration {
    VerificationDeclaration {
        id: Some(id.to_string()),
        requirement: Some("requirement".to_string()),
        phase: Some(phase.to_string()),
        owner: Some("owner".to_string()),
        trigger: Some("pull-request-validation".to_string()),
        automation: Some("src/orchestration/acceptance.rs".to_string()),
        evidence: Some(command.to_string()),
        rerun: Some(command.to_string()),
        prerequisites: Some(Vec::new()),
        execution_class: Some(execution_class.to_string()),
        completion_role: Some(completion_role.to_string()),
    }
}

fn gate(id: &str) -> ManifestGate {
    ManifestGate {
        verification_id: id.to_string(),
        argv: vec!["cargo".to_string(), "test".to_string(), id.to_string()],
        cwd: ".".to_string(),
        automation_path: "src/orchestration/acceptance.rs".to_string(),
        automation_blob_oid: "b".repeat(40),
        tool: ToolIdentity {
            path: "/usr/bin/cargo".to_string(),
            executable_digest: "c".repeat(64),
            version: Some("cargo 1.80.0".to_string()),
        },
        artifact_path: format!(".cflx/verification-evidence/{id}.log"),
        artifact_digest: Some("d".repeat(64)),
    }
}

fn manifest(change_id: &str, gates: Vec<ManifestGate>) -> AcceptanceExecutionManifest {
    AcceptanceExecutionManifest {
        schema: MANIFEST_SCHEMA.to_string(),
        change_id: change_id.to_string(),
        candidate_commit_oid: "a".repeat(40),
        candidate_tree_oid: "e".repeat(40),
        review_base_commit: "f".repeat(40),
        review_range: format!("{}..{}", "f".repeat(40), "a".repeat(40)),
        review_base_ref: Some("main".to_string()),
        change_input_digest: "1".repeat(64),
        skill: SkillIdentity {
            name: "cflx-accept".to_string(),
            digest: "2".repeat(64),
        },
        executable_identity: "3".repeat(64),
        gates,
        created_at: chrono::Utc::now(),
        absolute_deadline_secs: 3600,
        work_budget_secs: 3570,
        terminal: None,
    }
}

// ============================================================================
// Eligibility: the manifest is the closed blocking allowlist
// ============================================================================

/// Only `pre-integration` + `repository-local` + `change-blocking` declarations
/// with a supervisable argv become gates. Everything else is outside the
/// blocking boundary, which is what stops an unrelated declaration from turning
/// into a completion prerequisite.
#[test]
fn acceptance_execution_boundary_selects_only_eligible_declarations() {
    let declarations = vec![
        declaration(
            "focused-gate",
            "pre-integration",
            "repository-local",
            "change-blocking",
            "cargo test focused --lib",
        ),
        declaration(
            "post-integration-observation",
            "post-integration",
            "repository-automation",
            "operational-observation",
            "gh workflow run ci",
        ),
        declaration(
            "deployed-check",
            "pre-integration",
            "deployed-service",
            "change-blocking",
            "curl https://example.invalid",
        ),
        declaration(
            "not-blocking",
            "pre-integration",
            "repository-local",
            "operational-observation",
            "cargo test other --lib",
        ),
        declaration(
            "shell-pipeline",
            "pre-integration",
            "repository-local",
            "change-blocking",
            "cargo test | tee out.log",
        ),
    ];

    let gates = eligible_gates(&declarations);

    assert_eq!(
        gates
            .iter()
            .map(|gate| gate.verification_id.as_str())
            .collect::<Vec<_>>(),
        vec!["focused-gate"],
        "only the eligible repository-local change-blocking pre-integration \
         declaration may enter the blocking boundary"
    );
}

/// A change that declares nothing eligible still gets a manifest with an empty
/// allowlist. "There are no blocking gates" is the statement that stops a
/// reviewer from inventing some; an absent manifest would not say it.
#[test]
fn acceptance_execution_boundary_empty_allowlist_is_a_valid_boundary() {
    let empty = manifest("no-gates", Vec::new());

    assert!(!empty.has_gates());
    assert_eq!(
        empty.to_review_json()["declared_blocking_verifications"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
}

/// Gates are ordered by verification ID and duplicates collapse, so the same
/// declarations always hash the same way regardless of authoring order.
#[test]
fn acceptance_execution_boundary_gate_order_is_canonical() {
    let declarations = vec![
        declaration(
            "zulu",
            "pre-integration",
            "repository-local",
            "change-blocking",
            "cargo test zulu --lib",
        ),
        declaration(
            "alpha",
            "pre-integration",
            "repository-local",
            "change-blocking",
            "cargo test alpha --lib",
        ),
        declaration(
            "alpha",
            "pre-integration",
            "repository-local",
            "change-blocking",
            "cargo test alpha --lib",
        ),
    ];

    let gates = eligible_gates(&declarations);

    assert_eq!(
        gates
            .iter()
            .map(|gate| gate.verification_id.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "zulu"]
    );
}

// ============================================================================
// Fingerprint identity
// ============================================================================

/// The fingerprint is stable across timestamps, budgets, and terminal records.
/// If any of those moved it, recording "the deadline expired" would itself make
/// the next question look different and the unchanged-retry refusal could never
/// fire.
#[test]
fn acceptance_execution_boundary_fingerprint_excludes_timing_and_terminal_state() {
    let base = manifest("alpha", vec![gate("focused")]);
    let expected = base.fingerprint();

    let mut moved = base.clone();
    moved.created_at = chrono::Utc::now() + chrono::Duration::hours(3);
    moved.absolute_deadline_secs = 300;
    moved.work_budget_secs = 270;
    moved.review_base_ref = Some("develop".to_string());
    moved.terminal = Some(ManifestTerminal {
        hold_category: Some(AcceptanceHoldCategory::ReviewDeadlineExhausted),
        fingerprint: expected.clone(),
        cleanup_confirmed: true,
        cleanup_diagnostics: String::new(),
        recorded_at: chrono::Utc::now(),
    });

    assert_eq!(
        moved.fingerprint(),
        expected,
        "timestamps, budgets, the base ref name, and the terminal record must \
         not participate in the input fingerprint"
    );
}

/// Every declared fingerprint component actually changes the fingerprint.
///
/// This is the test that makes the refusal message honest: it tells an operator
/// "change one of these", so each one has to work.
#[test]
fn acceptance_execution_boundary_every_named_component_changes_the_fingerprint() {
    let base = manifest("alpha", vec![gate("focused")]);
    let original = base.fingerprint();

    /// One named way to change the manifest, paired with the operator-facing
    /// component it stands for.
    type NamedMutation = (&'static str, Box<dyn Fn(&mut AcceptanceExecutionManifest)>);

    let mutations: Vec<NamedMutation> = vec![
        (
            "candidate_commit",
            Box::new(|m: &mut AcceptanceExecutionManifest| m.candidate_commit_oid = "9".repeat(40)),
        ),
        (
            "candidate_tree",
            Box::new(|m: &mut AcceptanceExecutionManifest| m.candidate_tree_oid = "8".repeat(40)),
        ),
        (
            "review_base",
            Box::new(|m: &mut AcceptanceExecutionManifest| m.review_base_commit = "7".repeat(40)),
        ),
        (
            "review_range",
            Box::new(|m: &mut AcceptanceExecutionManifest| {
                m.review_range = "range-moved".to_string()
            }),
        ),
        (
            "change_inputs",
            Box::new(|m: &mut AcceptanceExecutionManifest| m.change_input_digest = "6".repeat(64)),
        ),
        (
            "skill_identity",
            Box::new(|m: &mut AcceptanceExecutionManifest| m.skill.digest = "5".repeat(64)),
        ),
        (
            "executable_identity",
            Box::new(|m: &mut AcceptanceExecutionManifest| m.executable_identity = "4".repeat(64)),
        ),
        (
            "declaration",
            Box::new(|m: &mut AcceptanceExecutionManifest| m.gates.push(gate("second"))),
        ),
        (
            "automation_blob",
            Box::new(|m: &mut AcceptanceExecutionManifest| {
                m.gates[0].automation_blob_oid = "0".repeat(40)
            }),
        ),
        (
            "command_identity",
            Box::new(|m: &mut AcceptanceExecutionManifest| {
                m.gates[0].argv.push("--nocapture".to_string())
            }),
        ),
        (
            "tool_identity",
            Box::new(|m: &mut AcceptanceExecutionManifest| {
                m.gates[0].tool.executable_digest = "1".repeat(64)
            }),
        ),
        (
            "artifact_digest",
            Box::new(|m: &mut AcceptanceExecutionManifest| {
                m.gates[0].artifact_digest = Some("2".repeat(64))
            }),
        ),
    ];

    for (component, mutate) in &mutations {
        let mut mutated = base.clone();
        mutate(&mut mutated);
        assert_ne!(
            mutated.fingerprint(),
            original,
            "changing '{component}' must produce a different Acceptance input fingerprint"
        );
    }

    let named: Vec<&str> = mutations.iter().map(|(name, _)| *name).collect();
    for component in FINGERPRINT_COMPONENTS {
        assert!(
            named.contains(component),
            "the operator-facing component list names '{component}' but nothing proves it matters"
        );
    }
}

/// Two independently constructed manifests with the same inputs agree. This is
/// the restart property in miniature: the digest comes from the values, not from
/// how the struct was built or when.
#[test]
fn acceptance_execution_boundary_fingerprint_is_reproducible() {
    let left = manifest("alpha", vec![gate("focused"), gate("second")]);
    let mut right = manifest("alpha", vec![gate("second"), gate("focused")]);
    right
        .gates
        .sort_by(|a, b| a.verification_id.cmp(&b.verification_id));
    right.created_at = left.created_at + chrono::Duration::seconds(41);

    assert_eq!(left.fingerprint(), right.fingerprint());
}

// ============================================================================
// Parsing: fail closed
// ============================================================================

#[test]
fn acceptance_execution_boundary_parses_a_well_formed_manifest() {
    let original = manifest("alpha", vec![gate("focused")]);
    let bytes = serde_json::to_vec(&original).unwrap();

    let parsed = parse_manifest(&bytes).expect("a well-formed manifest must parse");

    assert_eq!(parsed.fingerprint(), original.fingerprint());
}

#[test]
fn acceptance_execution_boundary_refuses_unknown_schema() {
    let mut original = manifest("alpha", vec![gate("focused")]);
    original.schema = "conflux-acceptance-execution-manifest-v99".to_string();
    let bytes = serde_json::to_vec(&original).unwrap();

    assert!(matches!(
        parse_manifest(&bytes),
        Err(ManifestDefect::UnknownSchema(_))
    ));
}

#[test]
fn acceptance_execution_boundary_refuses_short_object_ids() {
    let mut original = manifest("alpha", vec![gate("focused")]);
    original.candidate_commit_oid = "abc1234".to_string();
    let bytes = serde_json::to_vec(&original).unwrap();

    match parse_manifest(&bytes) {
        Err(ManifestDefect::Malformed(detail)) => {
            assert!(detail.contains("candidate_commit_oid"), "{detail}")
        }
        other => panic!("a short commit ID must fail closed, got {other:?}"),
    }
}

/// Out-of-order gates are refused because the fingerprint is order-sensitive:
/// accepting a reordered list would let one declaration set hash two ways.
#[test]
fn acceptance_execution_boundary_refuses_non_canonical_gate_order() {
    let mut original = manifest("alpha", vec![gate("zulu"), gate("alpha")]);
    original.terminal = None;
    let bytes = serde_json::to_vec(&original).unwrap();

    match parse_manifest(&bytes) {
        Err(ManifestDefect::Malformed(detail)) => {
            assert!(
                detail.contains("canonical verification-id order"),
                "{detail}"
            )
        }
        other => panic!("non-canonical gate order must fail closed, got {other:?}"),
    }
}

#[test]
fn acceptance_execution_boundary_refuses_malformed_json() {
    assert!(matches!(
        parse_manifest(b"{not json"),
        Err(ManifestDefect::Malformed(_))
    ));
}

// ============================================================================
// Storage: atomic, repository-local, restart-derivable
// ============================================================================

#[test]
fn acceptance_execution_boundary_stores_and_reloads_from_the_workspace() {
    let workspace = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(workspace.path().to_path_buf());
    let original = manifest("alpha", vec![gate("focused")]);

    store.store(&original).expect("manifest must persist");
    let reloaded = store.load("alpha").expect("manifest must reload");

    assert_eq!(reloaded.fingerprint(), original.fingerprint());
    assert!(
        workspace
            .path()
            .join(manifest_relative_path("alpha"))
            .is_file(),
        "the manifest must live inside the Git-excluded repository evidence area"
    );
    assert!(
        workspace
            .path()
            .join(EVIDENCE_DIR)
            .join(".gitignore")
            .is_file(),
        "the evidence directory must stay self-ignoring"
    );
}

#[test]
fn acceptance_execution_boundary_missing_manifest_is_not_an_error_shape() {
    let workspace = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(workspace.path().to_path_buf());

    assert_eq!(store.load("absent"), Err(ManifestDefect::Missing));
}

#[test]
fn acceptance_execution_boundary_refuses_unstorable_change_id() {
    let workspace = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(workspace.path().to_path_buf());

    assert!(matches!(
        store.load("../escape"),
        Err(ManifestDefect::Malformed(_))
    ));
    let mut escaping = manifest("alpha", Vec::new());
    escaping.change_id = "../escape".to_string();
    assert!(store.store(&escaping).is_err());
}

// ============================================================================
// Admission: unchanged input is refused, changed input is admitted
// ============================================================================

fn held(manifest: &AcceptanceExecutionManifest) -> AcceptanceExecutionManifest {
    let mut stored = manifest.clone();
    stored.terminal = Some(ManifestTerminal {
        hold_category: Some(AcceptanceHoldCategory::ReviewDeadlineExhausted),
        fingerprint: manifest.fingerprint(),
        cleanup_confirmed: true,
        cleanup_diagnostics: "owned process group confirmed quiescent".to_string(),
        recorded_at: chrono::Utc::now(),
    });
    stored
}

#[test]
fn acceptance_execution_boundary_refuses_unchanged_input_after_a_hold() {
    let current = manifest("alpha", vec![gate("focused")]);
    let stored = held(&current);

    let admission = classify_admission(&current.fingerprint(), Some(&stored));

    match &admission {
        AcceptanceAdmission::Refuse {
            outcome,
            category,
            fingerprint,
            components,
        } => {
            assert_eq!(*outcome, UNCHANGED_ACCEPTANCE_INPUT);
            assert_eq!(*category, AcceptanceHoldCategory::ReviewDeadlineExhausted);
            assert_eq!(fingerprint, &current.fingerprint());
            assert_eq!(components, &FINGERPRINT_COMPONENTS.to_vec());
        }
        other => panic!("unchanged input must be refused, got {other:?}"),
    }
    let detail = admission.detail().expect("a refusal must explain itself");
    for component in FINGERPRINT_COMPONENTS {
        assert!(
            detail.contains(component),
            "the refusal must name '{component}' as a way to restore eligibility: {detail}"
        );
    }
}

#[test]
fn acceptance_execution_boundary_changed_input_permits_a_new_attempt() {
    let previous = manifest("alpha", vec![gate("focused")]);
    let stored = held(&previous);
    let mut current = previous.clone();
    current.candidate_commit_oid = "9".repeat(40);

    assert_eq!(
        classify_admission(&current.fingerprint(), Some(&stored)),
        AcceptanceAdmission::Admit,
        "a repository-visible change to the candidate must permit one new bounded attempt"
    );
}

/// No stored manifest, no terminal record, and a terminal record with no hold
/// category all admit. The guard exists to stop one specific loop, not to become
/// a general gate on Acceptance.
#[test]
fn acceptance_execution_boundary_admits_every_non_hold_shape() {
    let current = manifest("alpha", vec![gate("focused")]);

    assert_eq!(
        classify_admission(&current.fingerprint(), None),
        AcceptanceAdmission::Admit
    );
    assert_eq!(
        classify_admission(&current.fingerprint(), Some(&current)),
        AcceptanceAdmission::Admit
    );

    let mut settled = current.clone();
    settled.terminal = Some(ManifestTerminal {
        hold_category: None,
        fingerprint: current.fingerprint(),
        cleanup_confirmed: true,
        cleanup_diagnostics: String::new(),
        recorded_at: chrono::Utc::now(),
    });
    assert_eq!(
        classify_admission(&current.fingerprint(), Some(&settled)),
        AcceptanceAdmission::Admit
    );
}

/// A recorded hold whose category is a *deadline* refuses an unchanged
/// fingerprint; one that is a runtime or manifest defect does not.
///
/// The difference is what the category is evidence *about*. A deadline answered
/// this exact input: re-asking it can only expire again. A runtime defect —
/// dirty worktree, unreadable evidence directory, transient Git failure —
/// answered the environment, and cleanliness is not a fingerprint component, so
/// an equal fingerprint after one says nothing. Refusing on it would make a
/// recoverable environment fact a permanent refusal.
#[test]
fn acceptance_execution_boundary_only_deadline_categories_refuse_unchanged_input() {
    let current = manifest("alpha", vec![gate("focused")]);
    let with_category = |category: AcceptanceHoldCategory| {
        let mut stored = current.clone();
        stored.terminal = Some(ManifestTerminal {
            hold_category: Some(category),
            fingerprint: current.fingerprint(),
            cleanup_confirmed: true,
            cleanup_diagnostics: String::new(),
            recorded_at: chrono::Utc::now(),
        });
        stored
    };

    for category in [
        AcceptanceHoldCategory::DeclaredGateDeadlineExhausted,
        AcceptanceHoldCategory::ReviewDeadlineExhausted,
        AcceptanceHoldCategory::ReviewCleanupUnproven,
    ] {
        assert!(category.refuses_unchanged_retry(), "{category:?}");
        let stored = with_category(category);
        assert!(
            classify_admission(&current.fingerprint(), Some(&stored)).is_refused(),
            "an equal fingerprint after {category:?} must be refused"
        );
    }

    for category in [
        AcceptanceHoldCategory::RuntimeDefect,
        AcceptanceHoldCategory::ManifestMalformed,
    ] {
        assert!(!category.refuses_unchanged_retry(), "{category:?}");
        let stored = with_category(category);
        assert_eq!(
            classify_admission(&current.fingerprint(), Some(&stored)),
            AcceptanceAdmission::Admit,
            "{category:?} says nothing about the input, so it must admit a fresh bounded attempt"
        );
    }
}

/// The same asymmetry through the workspace-derived path a restart takes: a
/// stored `RuntimeDefect` hold does not survive as a permanent refusal.
#[test]
fn acceptance_execution_boundary_runtime_defect_hold_admits_from_the_store() {
    let workspace = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(workspace.path().to_path_buf());
    let current = manifest("alpha", vec![gate("focused")]);
    let mut stored = current.clone();
    stored.terminal = Some(ManifestTerminal {
        hold_category: Some(AcceptanceHoldCategory::RuntimeDefect),
        fingerprint: current.fingerprint(),
        cleanup_confirmed: true,
        cleanup_diagnostics: "worktree was dirty".to_string(),
        recorded_at: chrono::Utc::now(),
    });
    store.store(&stored).unwrap();

    assert_eq!(
        classify_admission_from_store(workspace.path(), "alpha", &current.fingerprint()),
        AcceptanceAdmission::Admit,
        "cleaning the worktree changes no fingerprint component, so a runtime-defect hold \
         must not become a permanent refusal"
    );
}

/// Restart: delete every out-of-worktree file and the same worktree answers the
/// same way. This is constitutional law 1 expressed as a test — the workspace
/// alone decides the next action.
#[test]
fn acceptance_execution_boundary_restart_derives_the_same_refusal_from_the_workspace() {
    let workspace = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(workspace.path().to_path_buf());
    let current = manifest("alpha", vec![gate("focused")]);
    store.store(&held(&current)).unwrap();

    // A "restart" here is the total absence of any carried-over value: the
    // classification below reads nothing but the workspace path and the
    // recomputed fingerprint.
    let after_restart =
        classify_admission_from_store(workspace.path(), "alpha", &current.fingerprint());

    assert!(after_restart.is_refused());

    let mut moved = current.clone();
    moved.candidate_tree_oid = "7".repeat(40);
    assert_eq!(
        classify_admission_from_store(workspace.path(), "alpha", &moved.fingerprint()),
        AcceptanceAdmission::Admit
    );
}

/// A manifest nobody can read admits rather than refuses: a change must not be
/// stranded on a defect it did not cause, and the worst case is one bounded
/// attempt that rewrites the file correctly.
#[test]
fn acceptance_execution_boundary_malformed_stored_manifest_admits() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join(EVIDENCE_DIR)).unwrap();
    std::fs::write(
        workspace.path().join(manifest_relative_path("alpha")),
        b"{ truncated",
    )
    .unwrap();

    assert_eq!(
        classify_admission_from_store(workspace.path(), "alpha", &"a".repeat(64)),
        AcceptanceAdmission::Admit
    );
}

// ============================================================================
// Deadline partition
// ============================================================================

/// The configured minimum effective deadline is 300s, so the reserve must leave
/// exactly 270s of work. That number is in the spec, so it is asserted directly.
#[test]
fn acceptance_execution_boundary_reserves_thirty_seconds_for_finalization() {
    let partition = partition_deadline(300);

    assert_eq!(partition.reserve_secs, FINALIZATION_RESERVE_SECS);
    assert_eq!(partition.work_secs, 270);
    assert_eq!(
        partition.work_duration(),
        Some(std::time::Duration::from_secs(270))
    );
}

#[test]
fn acceptance_execution_boundary_reserve_scales_to_larger_budgets() {
    let partition = partition_deadline(3600);

    assert_eq!(partition.work_secs, 3570);
    assert_eq!(partition.reserve_secs, 30);
    assert_eq!(
        partition.work_secs + partition.reserve_secs,
        partition.absolute_secs
    );
}

/// A disabled limit stays disabled. Reserving a finalization window out of an
/// unbounded budget would invent a deadline the operator switched off.
#[test]
fn acceptance_execution_boundary_disabled_limit_has_no_partition() {
    let partition = partition_deadline(0);

    assert!(!partition.is_bounded());
    assert_eq!(partition.work_duration(), None);
}

/// A pathologically small positive limit still leaves work time rather than
/// turning every invocation into an immediate hold.
#[test]
fn acceptance_execution_boundary_tiny_limit_still_leaves_work_time() {
    for absolute in [1u64, 5, 29, 30, 31] {
        let partition = partition_deadline(absolute);
        assert!(
            partition.work_secs >= 1,
            "a positive limit of {absolute}s must leave at least one work second"
        );
        assert_eq!(partition.work_secs + partition.reserve_secs, absolute);
    }
}

// ============================================================================
// Hold projection
// ============================================================================

/// Every hold category is a non-resumable execution stop with no unblock
/// condition — which is exactly what keeps the reducer's classifier off the
/// external `blocked` path.
#[test]
fn acceptance_execution_boundary_holds_project_as_non_resumable_execution_stops() {
    for category in [
        AcceptanceHoldCategory::DeclaredGateDeadlineExhausted,
        AcceptanceHoldCategory::ReviewDeadlineExhausted,
        AcceptanceHoldCategory::ReviewCleanupUnproven,
        AcceptanceHoldCategory::ManifestMalformed,
        AcceptanceHoldCategory::RuntimeDefect,
    ] {
        let hold = AcceptanceExecutionHold {
            category,
            budget_secs: 3600,
            cleanup_confirmed: true,
            cleanup_diagnostics: "confirmed".to_string(),
            fingerprint: "a".repeat(64),
            evidence: vec!["focused-gate: executed and passed".to_string()],
        };
        let blocker = hold.to_stalled_blocker("alpha");

        assert!(!hold.permits_retry());
        assert_eq!(blocker.phase, "acceptance");
        assert_eq!(blocker.category, category.as_str());
        assert!(!blocker.resumable, "{category:?} must be non-resumable");
        assert!(
            blocker.unblock_condition.is_none(),
            "{category:?} is an execution stop, not a wait on a named prerequisite"
        );
        assert!(blocker.worktree_preserved);
        assert!(
            blocker
                .evidence
                .iter()
                .any(|line| line.contains(&hold.fingerprint)),
            "the hold must carry the fingerprint that refuses an unchanged retry"
        );
        assert_eq!(
            AcceptanceHoldCategory::parse(category.as_str()),
            Some(category)
        );
    }
}

/// Unproven cleanup is reported, not silently acknowledged: "we stopped it" and
/// "it is actually gone" stay separate facts.
#[test]
fn acceptance_execution_boundary_unproven_cleanup_travels_with_the_hold() {
    let hold = AcceptanceExecutionHold {
        category: AcceptanceHoldCategory::ReviewCleanupUnproven,
        budget_secs: 300,
        cleanup_confirmed: false,
        cleanup_diagnostics: "pid 4242 still live in group".to_string(),
        fingerprint: "b".repeat(64),
        evidence: Vec::new(),
    };

    let blocker = hold.to_stalled_blocker("alpha");

    assert!(blocker
        .evidence
        .iter()
        .any(|line| line.contains("unproven") && line.contains("pid 4242")));
}

// ============================================================================
// Digests
// ============================================================================

/// Change-input digests are path-tagged and order-independent: moving content
/// between files changes the digest, and filesystem iteration order does not.
#[test]
fn acceptance_execution_boundary_change_input_digest_is_path_tagged_and_ordered() {
    let a = vec![
        ("proposal.md".to_string(), b"one".to_vec()),
        ("tasks.md".to_string(), b"two".to_vec()),
    ];
    let reordered = vec![
        ("tasks.md".to_string(), b"two".to_vec()),
        ("proposal.md".to_string(), b"one".to_vec()),
    ];
    let swapped = vec![
        ("proposal.md".to_string(), b"two".to_vec()),
        ("tasks.md".to_string(), b"one".to_vec()),
    ];

    assert_eq!(digest_change_inputs(&a), digest_change_inputs(&reordered));
    assert_ne!(digest_change_inputs(&a), digest_change_inputs(&swapped));
}

/// Length-prefixed content means two files cannot be concatenated into a third
/// that hashes the same way.
#[test]
fn acceptance_execution_boundary_change_input_digest_resists_concatenation() {
    let split = vec![
        ("a".to_string(), b"xy".to_vec()),
        ("b".to_string(), b"z".to_vec()),
    ];
    let joined = vec![
        ("a".to_string(), b"xyz".to_vec()),
        ("b".to_string(), Vec::new()),
    ];

    assert_ne!(digest_change_inputs(&split), digest_change_inputs(&joined));
}

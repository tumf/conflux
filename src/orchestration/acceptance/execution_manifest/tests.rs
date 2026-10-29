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
        env_names: Vec::new(),
        env_binding_digest: empty_env_binding_digest(),
        env: EnvBindings::default(),
        cwd: ".".to_string(),
        automation_path: "src/orchestration/acceptance.rs".to_string(),
        automation_blob_oid: "b".repeat(40),
        tool: ToolIdentity {
            path: "/usr/bin/cargo".to_string(),
            executable_digest: "c".repeat(64),
            version: Some("cargo 1.80.0".to_string()),
        },
        artifact_path: EvidenceStore::artifact_relative_path(id),
        artifact_digest: Some("d".repeat(64)),
    }
}

/// A live hold for `manifest`'s current fingerprint.
fn live_hold(
    manifest: &AcceptanceExecutionManifest,
    category: AcceptanceHoldCategory,
) -> LiveAcceptanceHold {
    LiveAcceptanceHold {
        category,
        fingerprint: manifest.fingerprint(),
        review_base_ref: manifest.review_base_ref.clone(),
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
        declaration_defects: Vec::new(),
        created_at: chrono::Utc::now(),
        absolute_deadline_secs: 3600,
        work_budget_secs: 3570,
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
            "not-blocking",
            "pre-integration",
            "repository-local",
            "operational-observation",
            "cargo test other --lib",
        ),
        declaration(
            "observation-pipeline",
            "pre-integration",
            "repository-local",
            "operational-observation",
            "cargo test | tee out.log",
        ),
    ];

    let plan = plan_declared_gates(&declarations);

    assert!(plan.is_valid(), "{:?}", plan.defects);
    assert_eq!(
        plan.gates
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

/// Gates are ordered by verification ID, so the same declarations always hash
/// the same way regardless of authoring order. (Duplicates no longer collapse:
/// they are `declaration_duplicate_id` defects.)
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
    ];

    let plan = plan_declared_gates(&declarations);

    assert_eq!(
        plan.gates
            .iter()
            .map(|gate| gate.verification_id.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "zulu"]
    );
}

// ============================================================================
// Fingerprint identity
// ============================================================================

/// The fingerprint is stable across timestamps and budgets. If either moved it,
/// recording "the deadline expired" would itself make the next question look
/// different and the unchanged-retry refusal could never fire.
#[test]
fn acceptance_execution_boundary_fingerprint_excludes_timing_and_budget() {
    let base = manifest("alpha", vec![gate("focused")]);
    let expected = base.fingerprint();

    let mut moved = base.clone();
    moved.created_at = chrono::Utc::now() + chrono::Duration::hours(3);
    moved.absolute_deadline_secs = 300;
    moved.work_budget_secs = 270;
    moved.review_base_ref = Some("develop".to_string());

    assert_eq!(
        moved.fingerprint(),
        expected,
        "timestamps, budgets, and the base ref name must not participate in the \
         input fingerprint"
    );
}

/// The manifest carries no terminal record at all any more. That is the shape
/// change: a manifest that could hold one would be a durable artifact capable
/// of refusing work after a restart, which constitutional law 1 forbids.
#[test]
fn acceptance_execution_boundary_manifest_serializes_no_terminal_record() {
    let value = serde_json::to_value(manifest("alpha", vec![gate("focused")])).unwrap();
    let object = value.as_object().expect("a manifest is a JSON object");
    assert!(
        !object.contains_key("terminal"),
        "no persisted field may carry retry authority: {:?}",
        object.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        object["schema"], MANIFEST_SCHEMA,
        "the relocated, authority-free manifest is its own schema version"
    );
}

/// A version-1 manifest — the repository-local, terminal-record-carrying shape —
/// is refused rather than migrated. Reading one would mean resurrecting the
/// durable admission this change removed.
#[test]
fn acceptance_execution_boundary_version_one_manifest_is_refused() {
    let mut value = serde_json::to_value(manifest("alpha", vec![gate("focused")])).unwrap();
    value["schema"] = serde_json::json!("conflux-acceptance-execution-manifest-v1");
    value["terminal"] = serde_json::json!({
        "hold_category": "review_deadline_exhausted",
        "fingerprint": "a".repeat(64),
        "cleanup_confirmed": true,
        "cleanup_diagnostics": "",
        "recorded_at": "2026-08-23T00:00:00Z",
    });

    let defect = parse_manifest(&serde_json::to_vec(&value).unwrap()).unwrap_err();
    assert_eq!(defect.code(), "manifest_unknown_schema");
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
    let original = manifest("alpha", vec![gate("zulu"), gate("alpha")]);
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

/// A store root, a target, and the assertion that the two never meet.
fn external_store(temp: &tempfile::TempDir, change_id: &str) -> ManifestStore {
    ManifestStore::new(
        temp.path()
            .join("cflx-state/acceptance/project-slug/workspace-slug")
            .join(change_id),
    )
}

#[test]
fn acceptance_execution_boundary_stores_and_reloads_from_the_external_store() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target");
    std::fs::create_dir_all(&target).unwrap();
    let store = external_store(&temp, "alpha");
    let original = manifest("alpha", vec![gate("focused")]);

    store.store(&original).expect("manifest must persist");
    let reloaded = store.load().expect("manifest must reload");

    assert_eq!(reloaded.fingerprint(), original.fingerprint());
    assert!(
        store.path().is_file() && store.path().ends_with("alpha/manifest.json"),
        "the manifest belongs at the change's own external store root: {:?}",
        store.path()
    );
    assert_eq!(
        std::fs::read_dir(&target)
            .unwrap()
            .filter_map(Result::ok)
            .count(),
        0,
        "persisting a manifest must write nothing into the target"
    );
    assert!(
        !store.path().parent().unwrap().join(".gitignore").exists(),
        "no self-ignoring file may be written any more"
    );
}

/// Diagnostics are written for an operator and never read back. Storing them
/// must therefore be possible, and nothing in this module may consume them.
#[test]
fn acceptance_execution_boundary_diagnostics_are_inert_external_records() {
    let temp = tempfile::tempdir().unwrap();
    let store = external_store(&temp, "alpha");
    let current = manifest("alpha", vec![gate("focused")]);

    store
        .record_diagnostics(&AcceptanceDiagnostics {
            hold_category: Some(AcceptanceHoldCategory::ReviewDeadlineExhausted),
            fingerprint: current.fingerprint(),
            cleanup_confirmed: true,
            cleanup_diagnostics: "owned process group confirmed quiescent".to_string(),
            evidence: vec!["semantic review emitted no canonical verdict".to_string()],
            recorded_at: chrono::Utc::now(),
            recorded_by_pid: std::process::id(),
        })
        .expect("diagnostics must persist");

    assert!(store.diagnostics_path().is_file());
    // The record exists on disk and still refuses nothing: admission reads live
    // typed state, which is empty here.
    let holds = LiveAcceptanceHolds::new();
    assert_eq!(
        holds.classify("alpha", &current.fingerprint()),
        AcceptanceAdmission::Admit,
        "a persisted diagnostics record must never refuse an attempt"
    );
}

#[test]
fn acceptance_execution_boundary_missing_manifest_is_not_an_error_shape() {
    let temp = tempfile::tempdir().unwrap();

    assert_eq!(
        external_store(&temp, "absent").load(),
        Err(ManifestDefect::Missing)
    );
}

#[test]
fn acceptance_execution_boundary_refuses_unstorable_change_id() {
    let temp = tempfile::tempdir().unwrap();
    let store = external_store(&temp, "alpha");

    let mut escaping = manifest("alpha", Vec::new());
    escaping.change_id = "../escape".to_string();
    assert!(
        store.store(&escaping).is_err(),
        "a manifest whose change id could escape its store is refused"
    );
    assert!(!store.path().exists());
}

// ============================================================================
// Admission: unchanged input is refused, changed input is admitted
// ============================================================================

fn held(manifest: &AcceptanceExecutionManifest) -> LiveAcceptanceHold {
    live_hold(manifest, AcceptanceHoldCategory::ReviewDeadlineExhausted)
}

#[test]
fn acceptance_execution_boundary_refuses_unchanged_input_after_a_hold() {
    let current = manifest("alpha", vec![gate("focused")]);
    let live = held(&current);

    let admission = classify_admission(&current.fingerprint(), Some(&live));

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
    let live = held(&previous);
    let mut current = previous.clone();
    current.candidate_commit_oid = "9".repeat(40);

    assert_eq!(
        classify_admission(&current.fingerprint(), Some(&live)),
        AcceptanceAdmission::Admit,
        "a repository-visible change to the candidate must permit one new bounded attempt"
    );
}

/// No live hold at all admits. The guard exists to stop one specific loop within
/// one process, not to become a general gate on Acceptance.
#[test]
fn acceptance_execution_boundary_admits_when_this_owner_remembers_nothing() {
    let current = manifest("alpha", vec![gate("focused")]);

    assert_eq!(
        classify_admission(&current.fingerprint(), None),
        AcceptanceAdmission::Admit
    );
    assert_eq!(
        LiveAcceptanceHolds::new().classify("alpha", &current.fingerprint()),
        AcceptanceAdmission::Admit,
        "an empty registry — which is exactly what a restarted owner has — admits"
    );
}

/// A recorded hold whose category is a *deadline* refuses an unchanged
/// fingerprint; every other category does not.
///
/// The difference is what the category is evidence *about*. A deadline answered
/// this exact input: re-asking it can only expire again. A runtime defect, an
/// unwritable state root, or legacy target evidence answered the *environment*,
/// and none of those facts is a fingerprint component, so an equal fingerprint
/// after one says nothing. Refusing on it would make a recoverable environment
/// fact a permanent refusal.
#[test]
fn acceptance_execution_boundary_only_deadline_categories_refuse_unchanged_input() {
    let current = manifest("alpha", vec![gate("focused")]);

    for category in [
        AcceptanceHoldCategory::DeclaredGateDeadlineExhausted,
        AcceptanceHoldCategory::ReviewDeadlineExhausted,
        AcceptanceHoldCategory::ReviewCleanupUnproven,
    ] {
        assert!(category.refuses_unchanged_retry(), "{category:?}");
        assert!(
            classify_admission(&current.fingerprint(), Some(&live_hold(&current, category)))
                .is_refused(),
            "an equal fingerprint after {category:?} must be refused"
        );
    }

    for category in [
        AcceptanceHoldCategory::RuntimeDefect,
        AcceptanceHoldCategory::ManifestMalformed,
        AcceptanceHoldCategory::StatePathUnavailable,
        AcceptanceHoldCategory::LegacyTargetEvidence,
    ] {
        assert!(!category.refuses_unchanged_retry(), "{category:?}");
        assert_eq!(
            classify_admission(&current.fingerprint(), Some(&live_hold(&current, category))),
            AcceptanceAdmission::Admit,
            "{category:?} says nothing about the input, so it must admit a fresh bounded attempt"
        );
    }
}

/// A non-refusing category is not even remembered: recording it would only give
/// a later reader something misleading to find, since admission would admit on
/// it anyway.
#[test]
fn acceptance_execution_boundary_registry_remembers_only_refusing_holds() {
    let current = manifest("alpha", vec![gate("focused")]);
    let holds = LiveAcceptanceHolds::new();

    holds.record(
        "alpha",
        live_hold(&current, AcceptanceHoldCategory::ReviewDeadlineExhausted),
    );
    assert!(holds.get("alpha").is_some());

    holds.record(
        "alpha",
        live_hold(&current, AcceptanceHoldCategory::StatePathUnavailable),
    );
    assert!(
        holds.get("alpha").is_none(),
        "an environment-fact hold replaces the refusal rather than accumulating beside it"
    );
    assert_eq!(
        holds.classify("alpha", &current.fingerprint()),
        AcceptanceAdmission::Admit
    );
}

/// Restart: the refusal lives only in this process, so a new registry — which is
/// exactly what a restarted owner has — admits one fresh bounded attempt without
/// requiring any repository mutation.
///
/// This is constitutional law 1 expressed as a test. The durable record that
/// used to refuse here is gone, so deleting out-of-worktree state can no longer
/// change the next action either.
#[test]
fn acceptance_execution_boundary_restart_admits_one_fresh_attempt() {
    let current = manifest("alpha", vec![gate("focused")]);

    let owner = LiveAcceptanceHolds::new();
    owner.record("alpha", held(&current));
    assert!(
        owner.classify("alpha", &current.fingerprint()).is_refused(),
        "the owner that recorded the hold still refuses the unchanged input"
    );

    // "Restart": a new registry, the same unchanged fingerprint.
    let restarted = LiveAcceptanceHolds::new();
    assert_eq!(
        restarted.classify("alpha", &current.fingerprint()),
        AcceptanceAdmission::Admit,
        "a restarted owner must admit one fresh bounded attempt with no repository change"
    );

    // And the live owner still restores eligibility the ordinary way.
    let mut moved = current.clone();
    moved.candidate_tree_oid = "7".repeat(40);
    assert_eq!(
        owner.classify("alpha", &moved.fingerprint()),
        AcceptanceAdmission::Admit
    );
}

/// Admitting a fresh attempt clears the hold, so a refusal describes the latest
/// answer rather than accumulating across attempts.
#[test]
fn acceptance_execution_boundary_clearing_a_hold_restores_admission() {
    let current = manifest("alpha", vec![gate("focused")]);
    let holds = LiveAcceptanceHolds::new();
    holds.record("alpha", held(&current));
    holds.record("beta", held(&current));

    holds.clear("alpha");

    assert_eq!(
        holds.classify("alpha", &current.fingerprint()),
        AcceptanceAdmission::Admit
    );
    assert!(
        holds.classify("beta", &current.fingerprint()).is_refused(),
        "clearing one change must not disturb another"
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

// ============================================================================
// Shared declaration planner: required declarations fail closed
// ============================================================================

fn required(id: &str, command: &str) -> VerificationDeclaration {
    declaration(
        id,
        "pre-integration",
        "repository-local",
        "change-blocking",
        command,
    )
}

/// The single defect a one-declaration plan produced, with no gate beside it.
fn only_defect(declarations: &[VerificationDeclaration]) -> DeclarationDefect {
    let plan = plan_declared_gates(declarations);
    assert!(
        plan.gates.is_empty(),
        "an invalid required manifest offers no gate: {:?}",
        plan.gates
    );
    assert_eq!(plan.defects.len(), 1, "{:?}", plan.defects);
    plan.defects.into_iter().next().unwrap()
}

#[test]
fn required_verification_fail_closed_valid_required_declaration_is_one_gate() {
    let plan = plan_declared_gates(&[required("gate", "cargo test focused --lib")]);

    assert!(plan.is_valid());
    assert_eq!(plan.defects, Vec::new());
    assert_eq!(plan.gates.len(), 1);
    assert_eq!(plan.gates[0].verification_id, "gate");
    assert_eq!(
        plan.gates[0].argv,
        vec!["cargo", "test", "focused", "--lib"]
    );
}

#[test]
fn required_verification_fail_closed_shell_syntax_is_a_typed_defect() {
    for (command, construct) in [
        ("cargo test | tee out.log", "'|' at character offset 11"),
        ("cargo test && cargo clippy", "'&' at character offset 11"),
        ("cargo test; cargo clippy", "';' at character offset 10"),
        ("cargo test > out.log", "'>' at character offset 11"),
        ("cargo test $(echo focused)", "'$' at character offset 11"),
        ("cargo test 'focused", "unbalanced quote"),
    ] {
        let defect = only_defect(&[required("gate", command)]);
        assert_eq!(defect.code, "declaration_shell_syntax", "{command}");
        assert_eq!(defect.field, "rerun", "{command}");
        assert_eq!(defect.verification_id, "gate");
        assert_eq!(defect.position, 1);
        assert!(
            defect.detail.contains(construct),
            "{command}: detail must name the construct: {}",
            defect.detail
        );
        assert!(
            !defect.detail.contains(command),
            "the detail must not echo the command: {}",
            defect.detail
        );
    }
}

#[test]
fn required_verification_fail_closed_evidence_source_is_named_when_rerun_is_absent() {
    let mut declaration = required("gate", "cargo test | tee out.log");
    declaration.rerun = None;

    let defect = only_defect(&[declaration]);

    assert_eq!(defect.code, "declaration_shell_syntax");
    assert_eq!(defect.field, "evidence");
}

#[test]
fn required_verification_fail_closed_empty_command_is_a_typed_defect() {
    let defect = only_defect(&[required("gate", "   ")]);

    assert_eq!(defect.code, "declaration_missing_command");
    assert_eq!(defect.field, "rerun");
}

#[test]
fn required_verification_fail_closed_missing_automation_is_a_typed_defect() {
    for automation in [None, Some(""), Some("/etc/passwd"), Some("../outside.rs")] {
        let mut declaration = required("gate", "cargo test --lib");
        declaration.automation = automation.map(str::to_string);

        let defect = only_defect(&[declaration]);

        assert_eq!(
            defect.code, "declaration_missing_automation",
            "{automation:?}"
        );
        assert_eq!(defect.field, "automation");
    }
}

#[test]
fn required_verification_fail_closed_identity_defects_name_the_position() {
    let mut missing = required("ignored", "cargo test --lib");
    missing.id = None;
    let defect = only_defect(&[missing]);
    assert_eq!(defect.code, "declaration_missing_id");
    assert_eq!(defect.field, "id");
    assert_eq!(defect.verification_id, "<position 1>");

    let defect = only_defect(&[required("../escape", "cargo test --lib")]);
    assert_eq!(defect.code, "declaration_unstorable_id");
    assert_eq!(defect.field, "id");
}

#[test]
fn required_verification_fail_closed_duplicate_ids_are_reported_not_deduplicated() {
    let identical = vec![
        required("gate", "cargo test --lib"),
        required("gate", "cargo test --lib"),
    ];
    let different = vec![
        required("gate", "cargo test --lib"),
        required("other", "cargo test other --lib"),
        required("gate", "cargo test something-else --lib"),
    ];

    for (declarations, positions) in [(identical, "1, 2"), (different, "1, 3")] {
        let plan = plan_declared_gates(&declarations);
        assert!(
            plan.gates.is_empty(),
            "no gate may survive an invalid required manifest: {:?}",
            plan.gates
        );
        let duplicates: Vec<_> = plan
            .defects
            .iter()
            .filter(|defect| defect.code == "declaration_duplicate_id")
            .collect();
        assert_eq!(duplicates.len(), 2, "every occurrence is a defect");
        for defect in duplicates {
            assert_eq!(defect.verification_id, "gate");
            assert_eq!(defect.field, "id");
            assert!(
                defect.detail.contains(&format!("positions {positions}")),
                "{}",
                defect.detail
            );
        }
        assert_eq!(plan.defects.len(), 2, "{:?}", plan.defects);
    }
}

#[test]
fn required_verification_fail_closed_non_required_declarations_are_never_evaluated() {
    let mut legacy = declaration(
        "legacy",
        "pre-integration",
        "repository-local",
        "change-blocking",
        "cargo test | tee legacy.log",
    );
    legacy.execution_class = None;
    legacy.completion_role = None;
    let observation = declaration(
        "observation",
        "post-integration",
        "deployed-service",
        "operational-observation",
        "curl https://example.invalid/$PATH | jq .",
    );

    let mixed = plan_declared_gates(&[required("gate", "cargo test --lib"), observation.clone()]);
    assert!(mixed.is_valid(), "{:?}", mixed.defects);
    assert_eq!(mixed.gates.len(), 1);

    let only_non_required = plan_declared_gates(&[legacy, observation]);
    assert!(
        only_non_required.is_valid(),
        "{:?}",
        only_non_required.defects
    );
    assert!(only_non_required.gates.is_empty());
    assert!(plan_declared_gates(&[]).is_valid());
}

#[test]
fn required_verification_fail_closed_role_conflicts_name_the_field() {
    let defect = only_defect(&[declaration(
        "gate",
        "post-integration",
        "repository-local",
        "change-blocking",
        "cargo test --lib",
    )]);
    assert_eq!(defect.code, "declaration_role_conflict");
    assert_eq!(defect.field, "phase");

    let defect = only_defect(&[declaration(
        "gate",
        "pre-integration",
        "deployed-service",
        "change-blocking",
        "cargo test --lib",
    )]);
    assert_eq!(defect.code, "declaration_role_conflict");
    assert_eq!(defect.field, "execution_class");
}

/// A valid declaration next to a defective one does not become a gate: the
/// required set is judged as a whole.
#[test]
fn required_verification_fail_closed_one_defect_invalidates_every_gate() {
    let plan = plan_declared_gates(&[
        required("good", "cargo test --lib"),
        required("bad", "cargo test | tee out.log"),
    ]);

    assert!(!plan.is_valid());
    assert!(plan.gates.is_empty());
    assert_eq!(plan.defects.len(), 1);
    assert_eq!(plan.defects[0].verification_id, "bad");
    assert_eq!(plan.defects[0].position, 2);
}

/// The frozen manifest records its defects, they move the fingerprint, they
/// reach the review projection, and a stored manifest that carries a gate
/// beside a defect is refused rather than trusted.
#[test]
fn required_verification_fail_closed_manifest_distinguishes_invalid_from_empty() {
    let empty = manifest("declaration-invalid", Vec::new());
    let mut invalid = empty.clone();
    invalid.declaration_defects = only_defect_list();

    assert!(!empty.is_declaration_invalid());
    assert!(invalid.is_declaration_invalid());
    assert_ne!(empty.fingerprint(), invalid.fingerprint());
    assert_eq!(
        invalid.to_review_json()["declaration_defects"][0]["code"],
        "declaration_shell_syntax"
    );
    assert_eq!(
        empty.to_review_json()["declaration_defects"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );

    let round_trip = parse_manifest(&serde_json::to_vec(&invalid).unwrap()).unwrap();
    assert_eq!(round_trip.declaration_defects, invalid.declaration_defects);

    let mut contradictory = invalid.clone();
    contradictory.gates = vec![gate("alpha")];
    assert!(matches!(
        parse_manifest(&serde_json::to_vec(&contradictory).unwrap()),
        Err(ManifestDefect::Malformed(_))
    ));
}

fn only_defect_list() -> Vec<DeclarationDefect> {
    vec![only_defect(&[required("gate", "cargo test | tee out.log")])]
}

// ============================================================================
// Leading environment assignments (`rerun_leading_env`)
// ============================================================================

/// A manifest gate planned from one real declaration, exactly as the builder
/// derives identity from it.
fn gate_from_command(command: &str) -> ManifestGate {
    let plan = plan_declared_gates(&[required("focused", command)]);
    assert!(plan.is_valid(), "{:?}", plan.defects);
    let request = plan.gates.into_iter().next().unwrap();
    ManifestGate {
        argv: request.argv.clone(),
        env_names: request.env.names(),
        env_binding_digest: request.env.digest(),
        env: request.env,
        ..gate("focused")
    }
}

#[test]
fn rerun_leading_env_binding_changes_move_the_manifest_fingerprint() {
    let fingerprint =
        |command: &str| manifest("alpha", vec![gate_from_command(command)]).fingerprint();
    let base = fingerprint("A=1 cargo test focused --lib");
    assert_eq!(base, fingerprint("A=1 cargo test focused --lib"));
    for changed in [
        "A=2 cargo test focused --lib",
        "A=1 B=1 cargo test focused --lib",
        "cargo test focused --lib",
    ] {
        assert_ne!(base, fingerprint(changed), "'{changed}'");
    }
}

#[test]
fn rerun_leading_env_planner_reports_assignment_defects_on_the_command_field() {
    let invalid = only_defect(&[required("gate", "A-B=x cargo test")]);
    assert_eq!(invalid.code, "declaration_invalid_assignment");
    assert_eq!(invalid.field, "rerun");
    assert_eq!(invalid.verification_id, "gate");

    let assignment_only = only_defect(&[required("gate", "A=1 B=2")]);
    assert_eq!(assignment_only.code, "declaration_missing_command");
    assert_eq!(assignment_only.field, "rerun");

    // With no `rerun`, the defect names the `evidence` field it came from.
    let evidence_only = only_defect(&[VerificationDeclaration {
        rerun: None,
        ..required("gate", "1ABC=x cargo test")
    }]);
    assert_eq!(evidence_only.code, "declaration_invalid_assignment");
    assert_eq!(evidence_only.field, "evidence");
}

#[test]
fn rerun_leading_env_stored_manifest_carries_names_and_digest_only() {
    let temp = tempfile::tempdir().unwrap();
    let store = external_store(&temp, "alpha");
    let sentinel = "rerun-leading-env-sentinel-c40e11";
    let original = manifest(
        "alpha",
        vec![gate_from_command(&format!(
            "DJANGO_SETTINGS_MODULE={sentinel} cargo test focused --lib"
        ))],
    );
    store.store(&original).expect("manifest must persist");
    let bytes = std::fs::read_to_string(store.path()).unwrap();
    assert!(bytes.contains("env_binding_digest"));
    assert!(bytes.contains("DJANGO_SETTINGS_MODULE"));
    assert!(!bytes.contains(sentinel), "{bytes}");

    let reloaded = store.load().expect("manifest must reload");
    assert_eq!(reloaded.fingerprint(), original.fingerprint());
    assert!(
        reloaded.gates[0].env.is_empty(),
        "values are never reloaded"
    );
}

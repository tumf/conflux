//! Typed DTO, closed-command, and error-code tests.

use crate::web::remote_control_api::dto::{
    is_valid_correlation_id, new_hex_id, ApiError, CommandRecord, CommandRequest, CommandSpec,
    CommandState, ErrorCode, ALL_ERROR_CODES, MAX_CORRELATION_ID_LEN, SUPPORTED_COMMANDS,
};
use axum::http::StatusCode;

#[test]
fn closed_command_set_matches_the_advertised_list() {
    // Capability discovery and the parser must never drift apart: a client that
    // trusts `capabilities` has to be able to submit every name it lists.
    for name in SUPPORTED_COMMANDS {
        let body = match name {
            "start" | "stop" | "cancel_stop" | "force_stop" | "set_all_execution_marks" => {
                format!(r#"{{"type":"{name}"}}"#)
            }
            "set_execution_mark" => {
                r#"{"type":"set_execution_mark","change_id":"a","marked":true}"#.to_string()
            }
            "set_queue_intent" => {
                r#"{"type":"set_queue_intent","change_id":"a","queued":true}"#.to_string()
            }
            "retry_errors" => r#"{"type":"retry_errors","change_ids":["a"]}"#.to_string(),
            "create_worktree" => {
                r#"{"type":"create_worktree","target":{"change_id":"a"},"params":{}}"#.to_string()
            }
            "delete_worktree" | "merge_worktree" | "merge_worktree_abort_on_conflict" => {
                format!(r#"{{"type":"{name}","target":{{"worktree_id":"a"}},"params":{{}}}}"#)
            }
            "refresh_proposal" => format!(
                r#"{{"type":"refresh_proposal","change_id":"a","source_ref":"develop","expected_source":"{}","expected_head":"{}"}}"#,
                "a".repeat(40),
                "b".repeat(40)
            ),
            "write_change_text" => {
                r#"{"type":"write_change_text","target":{"change_id":"a"},"params":{"path":"tasks.md","expected":{"kind":"absent"},"content":"x"}}"#.to_string()
            }
            "write_worktree_text" => {
                r#"{"type":"write_worktree_text","target":{"worktree_id":"a"},"params":{"path":"src/lib.rs","expected":{"kind":"absent"},"content":"x"}}"#.to_string()
            }
            _ => format!(r#"{{"type":"{name}","change_id":"a"}}"#),
        };
        let parsed: CommandSpec =
            serde_json::from_str(&body).unwrap_or_else(|e| panic!("{name} must parse: {e}"));
        assert_eq!(parsed.type_name(), name);
    }
}

#[test]
fn unknown_command_type_fails_typed_validation() {
    let error = serde_json::from_str::<CommandSpec>(r#"{"type":"delete_everything"}"#).unwrap_err();
    assert!(
        error.to_string().contains("unknown variant"),
        "unexpected error: {error}"
    );
}

#[test]
fn command_envelope_requires_revision_and_idempotency_key() {
    let missing_revision = r#"{"type":"start","idempotency_key":"k1"}"#;
    assert!(serde_json::from_str::<CommandRequest>(missing_revision).is_err());

    let missing_key = r#"{"type":"start","expected_revision":0}"#;
    assert!(serde_json::from_str::<CommandRequest>(missing_key).is_err());

    let complete = r#"{"type":"start","expected_revision":0,"idempotency_key":"k1"}"#;
    let parsed: CommandRequest = serde_json::from_str(complete).unwrap();
    assert_eq!(parsed.expected_revision, 0);
    assert_eq!(parsed.idempotency_key, "k1");
    assert_eq!(parsed.correlation_id, None);
}

#[test]
fn identity_ignores_member_order_whitespace_key_and_correlation() {
    let a: CommandRequest = serde_json::from_str(
        r#"{"type":"set_queue_intent","change_id":"c1","queued":true,
            "expected_revision":7,"idempotency_key":"key-a","correlation_id":"trace-1"}"#,
    )
    .unwrap();
    let b: CommandRequest = serde_json::from_str(
        r#"{"idempotency_key":"key-b","expected_revision":7,"queued":true,"change_id":"c1","type":"set_queue_intent"}"#,
    )
    .unwrap();

    assert_eq!(
        a.identity(),
        b.identity(),
        "member order, whitespace, key, and correlation must not affect identity"
    );
}

#[test]
fn identity_includes_expected_revision_and_params() {
    let base: CommandRequest = serde_json::from_str(
        r#"{"type":"set_queue_intent","change_id":"c1","queued":true,"expected_revision":7,"idempotency_key":"k"}"#,
    )
    .unwrap();
    let other_revision: CommandRequest = serde_json::from_str(
        r#"{"type":"set_queue_intent","change_id":"c1","queued":true,"expected_revision":8,"idempotency_key":"k"}"#,
    )
    .unwrap();
    let other_param: CommandRequest = serde_json::from_str(
        r#"{"type":"set_queue_intent","change_id":"c1","queued":false,"expected_revision":7,"idempotency_key":"k"}"#,
    )
    .unwrap();
    let other_target: CommandRequest = serde_json::from_str(
        r#"{"type":"set_queue_intent","change_id":"c2","queued":true,"expected_revision":7,"idempotency_key":"k"}"#,
    )
    .unwrap();

    assert_ne!(base.identity(), other_revision.identity());
    assert_ne!(base.identity(), other_param.identity());
    assert_ne!(base.identity(), other_target.identity());
}

#[test]
fn schema_defaults_are_applied_before_identity_comparison() {
    // `retry_errors` defaults `change_ids` to an empty list; an omitted field and
    // an explicit empty list must be the same intent.
    let omitted: CommandRequest = serde_json::from_str(
        r#"{"type":"retry_errors","expected_revision":0,"idempotency_key":"k"}"#,
    )
    .unwrap();
    let explicit: CommandRequest = serde_json::from_str(
        r#"{"type":"retry_errors","change_ids":[],"expected_revision":0,"idempotency_key":"k"}"#,
    )
    .unwrap();
    assert_eq!(omitted.identity(), explicit.identity());
}

#[test]
fn command_target_is_reported_only_for_single_change_commands() {
    assert_eq!(CommandSpec::Start.target(), None);
    assert_eq!(
        CommandSpec::RetryErrors {
            change_ids: vec!["a".to_string()]
        }
        .target(),
        None
    );
    assert_eq!(
        CommandSpec::ResolveMerge {
            change_id: "c1".to_string()
        }
        .target(),
        Some("c1")
    );
}

#[test]
fn error_codes_map_to_stable_transport_statuses() {
    assert_eq!(
        ErrorCode::Unauthorized.http_status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(ErrorCode::Forbidden.http_status(), StatusCode::FORBIDDEN);
    assert_eq!(ErrorCode::NotFound.http_status(), StatusCode::NOT_FOUND);
    assert_eq!(
        ErrorCode::ValidationFailed.http_status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        ErrorCode::RegistryCapacity.http_status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        ErrorCode::InternalError.http_status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    // Several distinct causes deliberately share 409, which is exactly why the
    // client is told to branch on `error_code` instead.
    for code in [
        ErrorCode::StaleRevision,
        ErrorCode::LifecycleConflict,
        ErrorCode::TargetIneligible,
        ErrorCode::RootBusy,
        ErrorCode::IdempotencyMismatch,
    ] {
        assert_eq!(code.http_status(), StatusCode::CONFLICT, "{code:?}");
    }
}

#[test]
fn every_error_code_has_a_stable_wire_name() {
    for code in ALL_ERROR_CODES {
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, format!("\"{}\"", code.as_str()));
    }
}

#[test]
fn error_body_carries_code_message_correlation_and_optional_revision() {
    let plain = ApiError::new(ErrorCode::NotFound, "gone", "trace-1");
    let json = serde_json::to_value(&plain).unwrap();
    assert_eq!(json["error_code"], "not_found");
    assert_eq!(json["message"], "gone");
    assert_eq!(json["correlation_id"], "trace-1");
    assert!(json.get("current_revision").is_none());

    let with_revision =
        ApiError::new(ErrorCode::StaleRevision, "stale", "trace-2").with_revision(12);
    let json = serde_json::to_value(&with_revision).unwrap();
    assert_eq!(json["current_revision"], 12);
}

#[test]
fn correlation_ids_accept_the_documented_charset_and_bounds() {
    assert!(is_valid_correlation_id("abc.DEF_012:-"));
    assert!(is_valid_correlation_id(&"a".repeat(MAX_CORRELATION_ID_LEN)));

    assert!(!is_valid_correlation_id(""));
    assert!(!is_valid_correlation_id(
        &"a".repeat(MAX_CORRELATION_ID_LEN + 1)
    ));
    assert!(!is_valid_correlation_id("has newline\n"));
    assert!(!is_valid_correlation_id("has space"));
    assert!(!is_valid_correlation_id("日本語"));
}

#[test]
fn generated_ids_are_128_bit_lowercase_hex() {
    let id = new_hex_id();
    assert_eq!(id.len(), 32);
    assert!(id
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
    assert_ne!(id, new_hex_id(), "IDs must not be constant");
}

#[test]
fn command_record_status_reflects_its_lifecycle_state() {
    let mut record = CommandRecord {
        command_id: new_hex_id(),
        instance_id: new_hex_id(),
        command_type: "start".to_string(),
        state: CommandState::Running,
        expected_revision: 0,
        result_revision: None,
        correlation_id: "trace".to_string(),
        idempotency_key: "k".to_string(),
        created_at: "now".to_string(),
        completed_at: None,
        detail: None,
        error_code: None,
        result: None,
    };
    assert_eq!(record.http_status(), StatusCode::ACCEPTED);

    record.state = CommandState::Succeeded;
    assert_eq!(record.http_status(), StatusCode::OK);

    record.state = CommandState::NoOp;
    assert_eq!(record.http_status(), StatusCode::OK);

    record.state = CommandState::Failed;
    record.error_code = Some(ErrorCode::TargetIneligible);
    assert_eq!(record.http_status(), StatusCode::CONFLICT);
}

// ============================================================================
// Additive-field compatibility: a client reading an older owner
// ============================================================================
//
// The owner shape these tests reproduce is the one every `/api/v2` build before
// the candidate proposal refresh surface serves (v0.6.328-rc.1 is the deployed
// instance of it): `actions` carries every member except `refresh_proposal`,
// and `capabilities` carries no `proposal_refresh` block and no
// `refresh_proposal` command. The fixtures are derived from this build's own
// serializer with exactly those members removed — and only those, which is what
// the DTO history between that owner and this build shows was added — so they
// cannot drift from the wire shape the way a hand-copied snapshot would.

mod legacy_owner_shape {
    use crate::web::remote_control_api::dto::{
        ActionBlockedReason, AttentionState, CapabilitiesResponse, ChangeResource, ChangeTiming,
        ParallelEligibility, QueueIntent,
    };
    use crate::web::remote_control_api::projection::change_actions_for_test;

    /// One projected row as this build serializes it.
    fn current_row(display_status: &str) -> serde_json::Value {
        let row = ChangeResource {
            id: "alpha".to_string(),
            display_status: display_status.to_string(),
            progress_status: "pending".to_string(),
            completed_tasks: 0,
            total_tasks: 2,
            progress_percent: 0.0,
            dependencies: Vec::new(),
            iteration_number: None,
            execution_marked: false,
            queue_intent: QueueIntent::NotQueued,
            attention: AttentionState::None,
            blocker: None,
            error_detail: None,
            actions: change_actions_for_test("select", display_status, None),
            parallel: ParallelEligibility::default(),
            timing: ChangeTiming::default(),
            latest_activity: None,
            worktree: None,
            acceptance_provenance: None,
        };
        serde_json::to_value(row).expect("a projected row serializes")
    }

    /// The same row as an owner that predates candidate refresh publishes it.
    fn legacy_row(display_status: &str) -> serde_json::Value {
        let mut row = current_row(display_status);
        let removed = row["actions"]
            .as_object_mut()
            .expect("actions is an object")
            .remove("refresh_proposal");
        assert!(
            removed.is_some(),
            "this build publishes `refresh_proposal`; the legacy shape is defined by its absence"
        );
        row
    }

    /// `GET /api/v2/capabilities` as an owner that predates candidate refresh
    /// publishes it: the closed command set without `refresh_proposal`, and no
    /// `proposal_refresh` block at all.
    fn legacy_capabilities() -> serde_json::Value {
        serde_json::json!({
            "api_version": "v2",
            "instance_id": "i-legacy",
            "commands": [
                "start", "stop", "cancel_stop", "force_stop", "set_all_execution_marks",
                "set_execution_mark", "set_queue_intent", "retry_change", "retry_errors",
                "stop_and_dequeue", "force_stop_change", "resolve_merge",
                "create_worktree", "delete_worktree", "merge_worktree"
            ],
            "transports": [],
            "error_codes": [],
            "limits": {
                "max_events": 1, "max_logs": 1, "max_commands": 1,
                "max_idempotency_records": 1, "command_record_ttl_secs": 1,
                "max_correlation_id_len": 64
            },
            "authentication_required": false,
            "command_execution": { "available": true },
            "worktrees": serde_json::to_value(
                crate::web::remote_control_api::worktrees::WorktreeCapabilities::default()
            )
            .expect("the worktree surface serializes"),
            "parallel": { "max_concurrent": 1, "vcs_backend": "git", "blocked_reasons": [] }
        })
    }

    #[test]
    fn a_row_without_refresh_eligibility_decodes_with_the_table_absent() {
        let row: ChangeResource = serde_json::from_value(legacy_row("error"))
            .expect("an older owner's row must still decode on this build");
        assert_eq!(
            row.actions.refresh_proposal, None,
            "absence is preserved as absence, never projected onto a permission"
        );
        // Every pre-existing member is still the published value, not a default.
        assert!(row.actions.set_execution_mark.allowed);
        assert!(row.actions.retry_change.allowed);
        assert_eq!(
            row.actions.force_stop_change.blocked_reason,
            Some(ActionBlockedReason::NotAdmitted)
        );
    }

    #[test]
    fn a_row_with_refresh_eligibility_decodes_with_the_published_table() {
        let row: ChangeResource =
            serde_json::from_value(current_row("error")).expect("this build's own row decodes");
        let published = row
            .actions
            .refresh_proposal
            .expect("a current owner publishes the refresh table");
        assert_eq!(
            published.blocked_reason,
            Some(ActionBlockedReason::NoCandidateWorktree),
            "the published refusal survives the round trip unchanged"
        );
        let row: ChangeResource =
            serde_json::from_value(serde_json::to_value(row).expect("serializes"))
                .expect("round trip");
        assert_eq!(row.actions.refresh_proposal, Some(published));
    }

    #[test]
    fn the_server_always_writes_the_refresh_table() {
        let row = current_row("not queued");
        assert!(
            row["actions"]["refresh_proposal"].is_object(),
            "`Option` is a decoding affordance for older owners, not a licence to omit: {row}"
        );
    }

    #[test]
    fn a_row_missing_a_pre_existing_required_member_is_still_rejected() {
        for member in [
            "set_execution_mark",
            "set_queue_intent",
            "retry_change",
            "stop_and_dequeue",
            "force_stop_change",
            "resolve_merge",
        ] {
            let mut row = legacy_row("error");
            row["actions"]
                .as_object_mut()
                .unwrap()
                .remove(member)
                .expect("the member was published");
            let error = serde_json::from_value::<ChangeResource>(row)
                .expect_err("a payload missing a required member is malformed");
            assert!(
                error.to_string().contains(member),
                "the rejection must name `{member}`: {error}"
            );
        }
    }

    #[test]
    fn capabilities_without_the_refresh_block_decode_as_not_served() {
        let capabilities: CapabilitiesResponse = serde_json::from_value(legacy_capabilities())
            .expect("an older owner's capabilities must still decode on this build");
        assert!(!capabilities.proposal_refresh.preview_available);
        assert!(capabilities.proposal_refresh.command.is_empty());
        assert!(!capabilities
            .commands
            .iter()
            .any(|command| command == "refresh_proposal"));
        // The sibling additive surfaces decode the same way, by the same rule.
        assert!(!capabilities.execution_sinks.available);
        assert!(!capabilities.proposal_subscriptions.available);
    }

    #[test]
    fn remote_merge_abort_old_owner_capabilities_decode_as_unavailable() {
        // An owner that predates the abort-on-conflict merge: five worktree
        // operations, no `merge_abort` block, no new command.
        let mut capabilities = legacy_capabilities();
        let worktrees = capabilities["worktrees"].as_object_mut().unwrap();
        worktrees
            .remove("merge_abort")
            .expect("this build publishes `merge_abort`");
        worktrees.insert(
            "operations".to_string(),
            serde_json::json!(["list", "detail", "create", "delete", "merge"]),
        );

        let decoded: CapabilitiesResponse = serde_json::from_value(capabilities)
            .expect("an older owner's capabilities must still decode on this build");
        assert!(!decoded.worktrees.merge_abort.available);
        assert!(decoded.worktrees.merge_abort.command.is_empty());
        assert!(!decoded
            .commands
            .iter()
            .any(|command| command == "merge_worktree_abort_on_conflict"));
        // The preserving contract reads the same on both owners.
        assert!(decoded.worktrees.merge_conflict_preserves_state);
        assert_eq!(
            decoded.worktrees.merge_conflict_recovery,
            "local_or_tui_required"
        );

        // This build always writes the block, available.
        let current = crate::web::remote_control_api::worktrees::WorktreeCapabilities::default();
        assert!(current.merge_abort.available);
        assert_eq!(
            current.merge_abort.command,
            "merge_worktree_abort_on_conflict"
        );
        assert!(current.merge_abort.predicted_conflict_refused);
        assert_eq!(
            current.merge_abort.restore_verification,
            ["head_equals_premerge", "no_merge_head", "root_clean"]
        );
        assert_eq!(
            current.merge_abort.reasons,
            [
                "predicted_conflict",
                "conflict_aborted",
                "abort_failed",
                "restore_unknown"
            ]
        );
        assert!(current.merge_conflict_preserves_state);
        assert!(current.operations.iter().any(|op| op == "merge_abort"));
    }

    #[test]
    fn capabilities_missing_a_pre_existing_required_member_are_still_rejected() {
        for member in [
            "command_execution",
            "commands",
            "limits",
            "worktrees",
            "parallel",
        ] {
            let mut capabilities = legacy_capabilities();
            capabilities
                .as_object_mut()
                .unwrap()
                .remove(member)
                .expect("the member was published");
            let error = serde_json::from_value::<CapabilitiesResponse>(capabilities)
                .expect_err("a payload missing a required member is malformed");
            assert!(
                error.to_string().contains(member),
                "the rejection must name `{member}`: {error}"
            );
        }
    }
}

// ============================================================================
// Bounded text edit
// ============================================================================

fn text_command(params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "type": "write_worktree_text",
        "target": {"worktree_id": "0123456789abcdef0123456789abcdef"},
        "params": params,
    })
}

#[test]
fn remote_text_edit_commands_are_closed_and_validated() {
    use crate::web::remote_control_api::dto::{TextExpectation, TextWriteParams};

    let id = format!("sha256:{}", "a".repeat(64));
    let parsed: CommandSpec = serde_json::from_value(text_command(serde_json::json!({
        "path": "src/lib.rs",
        "expected": {"kind": "content", "content_id": id},
        "content": "fn main() {}\n",
    })))
    .expect("the frozen shape parses");
    assert_eq!(parsed.type_name(), "write_worktree_text");
    assert_eq!(
        parsed.target(),
        None,
        "a worktree write addresses no change"
    );
    assert!(parsed.validate().is_ok());
    match &parsed {
        CommandSpec::WriteWorktreeText { params, .. } => assert_eq!(
            params,
            &TextWriteParams {
                path: "src/lib.rs".to_string(),
                expected: TextExpectation::Content {
                    content_id: id.clone()
                },
                content: "fn main() {}\n".to_string(),
            }
        ),
        other => panic!("unexpected {other:?}"),
    }
    let change: CommandSpec = serde_json::from_value(serde_json::json!({
        "type": "write_change_text",
        "target": {"change_id": "alpha"},
        "params": {"path": "tasks.md", "expected": {"kind": "absent"}, "content": ""},
    }))
    .unwrap();
    assert_eq!(change.target(), Some("alpha"));

    // Closed: no smuggled member anywhere, no missing expectation, no
    // unconditional write.
    for params in [
        serde_json::json!({"path": "a", "expected": {"kind": "absent"}, "content": "", "mode": "0755"}),
        serde_json::json!({"path": "a", "content": ""}),
        serde_json::json!({"path": "a", "expected": null, "content": ""}),
        serde_json::json!({"path": "a", "expected": {"kind": "any"}, "content": ""}),
        serde_json::json!({"path": "a", "expected": {"kind": "absent", "content_id": id}, "content": ""}),
    ] {
        assert!(
            serde_json::from_value::<CommandSpec>(text_command(params.clone())).is_err(),
            "{params}"
        );
    }
    let mut smuggled_root = text_command(
        serde_json::json!({"path": "a", "expected": {"kind": "absent"}, "content": ""}),
    );
    smuggled_root["target"]["path"] = serde_json::json!("/etc");
    assert!(serde_json::from_value::<CommandSpec>(smuggled_root).is_err());

    // Field validation serde cannot express.
    let oversize = "a".repeat(1_048_577);
    for params in [
        serde_json::json!({"path": "a", "expected": {"kind": "content", "content_id": "sha256:abc"}, "content": ""}),
        serde_json::json!({"path": "a", "expected": {"kind": "content", "content_id": format!("sha256:{}", "A".repeat(64))}, "content": ""}),
        serde_json::json!({"path": "a", "expected": {"kind": "absent"}, "content": oversize}),
        serde_json::json!({"path": "a", "expected": {"kind": "absent"}, "content": "a\u{0}b"}),
        serde_json::json!({"path": "../a", "expected": {"kind": "absent"}, "content": ""}),
        serde_json::json!({"path": "/a", "expected": {"kind": "absent"}, "content": ""}),
        serde_json::json!({"path": "", "expected": {"kind": "absent"}, "content": ""}),
    ] {
        let spec: CommandSpec = serde_json::from_value(text_command(params.clone())).unwrap();
        assert!(spec.validate().is_err(), "{params}");
    }
    let exactly_max = "a".repeat(1_048_576);
    let spec: CommandSpec = serde_json::from_value(text_command(serde_json::json!({
        "path": "a", "expected": {"kind": "absent"}, "content": exactly_max,
    })))
    .unwrap();
    assert!(
        spec.validate().is_ok(),
        "1 MiB exactly is within the ceiling"
    );
}

#[test]
fn remote_text_edit_error_codes_are_conflicts_with_stable_names() {
    for (code, name) in [
        (ErrorCode::TextStale, "text_stale"),
        (ErrorCode::TextNotEditable, "text_not_editable"),
    ] {
        assert_eq!(code.http_status(), StatusCode::CONFLICT);
        assert_eq!(code.as_str(), name);
        assert!(ALL_ERROR_CODES.contains(&code));
    }
    assert!(SUPPORTED_COMMANDS.contains(&"write_change_text"));
    assert!(SUPPORTED_COMMANDS.contains(&"write_worktree_text"));
}

#[test]
fn remote_text_edit_old_owner_capabilities_decode_as_unavailable() {
    use crate::web::remote_control_api::dto::{CapabilitiesResponse, StateResponse};

    // An owner built before the surface: every member it always published, and
    // no `text_edit`.
    let mut capabilities = serde_json::json!({
        "api_version": "v2",
        "instance_id": "i-old",
        "commands": ["start"],
        "transports": [],
        "error_codes": ["internal_error"],
        "limits": {
            "max_events": 1, "max_logs": 1, "max_commands": 1,
            "max_idempotency_records": 1, "command_record_ttl_secs": 1,
            "max_correlation_id_len": 64
        },
        "authentication_required": false,
        "command_execution": {"available": true},
        "worktrees": {
            "operations": ["list"],
            "merge_conflict_recovery": "local_or_tui_required",
            "merge_conflict_preserves_state": true,
            "delete_requires_teardown": true
        },
        "parallel": {"max_concurrent": 1, "vcs_backend": "git", "blocked_reasons": []}
    });
    let decoded: CapabilitiesResponse = serde_json::from_value(capabilities.clone())
        .expect("an older owner's capabilities still decode");
    assert!(
        !decoded.text_edit.available,
        "absent decodes as unavailable"
    );
    assert!(decoded.text_edit.commands.is_empty());

    capabilities["text_edit"] =
        serde_json::to_value(crate::web::remote_control_api::text_edit::capability(true)).unwrap();
    let current: CapabilitiesResponse = serde_json::from_value(capabilities).unwrap();
    assert!(current.text_edit.available);

    // A state body without any text member still decodes: the surface adds no
    // snapshot member at all.
    let state = serde_json::json!({
        "instance_id": "i-old",
        "state_revision": 3,
        "event_sequence": 4,
        "snapshot": serde_json::to_value(
            crate::web::remote_control_api::dto::InstanceSnapshot::empty()
        ).unwrap(),
    });
    let decoded: StateResponse = serde_json::from_value(state).unwrap();
    assert_eq!(decoded.state_revision, 3);
}

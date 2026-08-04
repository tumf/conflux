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
            "set_parallel_mode" => r#"{"type":"set_parallel_mode","enabled":true}"#.to_string(),
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
            "delete_worktree" | "merge_worktree" => {
                format!(r#"{{"type":"{name}","target":{{"worktree_id":"a"}},"params":{{}}}}"#)
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

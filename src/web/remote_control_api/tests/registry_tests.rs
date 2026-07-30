//! Registry tests: structural identity, replay, expiry, and fail-closed capacity.

use chrono::{Duration, TimeZone, Utc};

use crate::web::remote_control_api::dto::{
    CommandIdentity, CommandRecord, CommandSpec, CommandState, ErrorCode,
};
use crate::web::remote_control_api::registry::{
    CommandOutcome, CommandRegistry, IdempotencyLookup, ReserveError,
};

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
}

fn identity(change_id: &str, revision: u64) -> CommandIdentity {
    CommandIdentity {
        command: CommandSpec::RetryChange {
            change_id: change_id.to_string(),
        },
        expected_revision: revision,
    }
}

fn record(command_id: &str, key: &str, state: CommandState) -> CommandRecord {
    CommandRecord {
        command_id: command_id.to_string(),
        instance_id: "instance".to_string(),
        command_type: "retry_change".to_string(),
        state,
        expected_revision: 0,
        result_revision: None,
        correlation_id: "trace".to_string(),
        idempotency_key: key.to_string(),
        created_at: now().to_rfc3339(),
        completed_at: match state {
            CommandState::Running => None,
            _ => Some(now().to_rfc3339()),
        },
        detail: None,
        error_code: None,
    }
}

#[test]
fn unknown_key_is_admitted_and_bound_atomically() {
    let mut registry = CommandRegistry::new(10, 3600);
    assert!(matches!(
        registry.lookup("k1", &identity("c1", 0), now()),
        IdempotencyLookup::Unknown
    ));

    registry
        .reserve(
            "k1",
            identity("c1", 0),
            record("cmd1", "k1", CommandState::Running),
            now(),
        )
        .unwrap();

    assert_eq!(registry.command_len(), 1);
    assert_eq!(
        registry.idempotency_len(),
        1,
        "a command record must never exist without its idempotency binding"
    );
}

#[test]
fn structurally_equal_replay_returns_the_original_record() {
    let mut registry = CommandRegistry::new(10, 3600);
    registry
        .reserve(
            "k1",
            identity("c1", 5),
            record("cmd1", "k1", CommandState::Running),
            now(),
        )
        .unwrap();
    registry.complete(
        "cmd1",
        CommandOutcome {
            state: CommandState::Succeeded,
            result_revision: 6,
            detail: Some("done".to_string()),
            error_code: None,
        },
    );

    let IdempotencyLookup::Replay(replayed) = registry.lookup("k1", &identity("c1", 5), now())
    else {
        panic!("an exact replay must resolve");
    };
    assert_eq!(replayed.command_id, "cmd1");
    assert_eq!(replayed.state, CommandState::Succeeded);
    assert_eq!(replayed.result_revision, Some(6));
}

#[test]
fn same_key_with_different_identity_is_a_mismatch() {
    let mut registry = CommandRegistry::new(10, 3600);
    registry
        .reserve(
            "k1",
            identity("c1", 5),
            record("cmd1", "k1", CommandState::Running),
            now(),
        )
        .unwrap();

    assert!(matches!(
        registry.lookup("k1", &identity("c1", 6), now()),
        IdempotencyLookup::Mismatch
    ));
    assert!(matches!(
        registry.lookup("k1", &identity("c2", 5), now()),
        IdempotencyLookup::Mismatch
    ));
}

#[test]
fn completed_records_expire_after_the_configured_ttl() {
    let mut registry = CommandRegistry::new(10, 3600);
    registry
        .reserve(
            "k1",
            identity("c1", 0),
            record("cmd1", "k1", CommandState::Running),
            now(),
        )
        .unwrap();
    registry.complete(
        "cmd1",
        CommandOutcome {
            state: CommandState::Succeeded,
            result_revision: 1,
            detail: None,
            error_code: None,
        },
    );

    // `complete` stamps the real clock, so expire relative to that.
    let far_future = Utc::now() + Duration::seconds(3601);
    assert!(matches!(
        registry.lookup("k1", &identity("c1", 0), far_future),
        IdempotencyLookup::Unknown
    ));
    assert_eq!(registry.command_len(), 0);
    assert_eq!(registry.idempotency_len(), 0);
}

#[test]
fn oldest_completed_record_is_evicted_to_admit_new_work() {
    let mut registry = CommandRegistry::new(2, 3600);
    for i in 0..2 {
        let key = format!("k{i}");
        let id = format!("cmd{i}");
        registry
            .reserve(
                &key,
                identity(&format!("c{i}"), 0),
                record(&id, &key, CommandState::Succeeded),
                now(),
            )
            .unwrap();
    }

    registry
        .reserve(
            "k9",
            identity("c9", 0),
            record("cmd9", "k9", CommandState::Running),
            now(),
        )
        .expect("a completed record may be evicted to admit new work");

    assert_eq!(registry.command_len(), 2);
    assert!(
        registry.get("cmd0").is_none(),
        "the oldest completed record went first"
    );
    assert!(registry.get("cmd9").is_some());
}

#[test]
fn in_progress_records_are_pinned_and_admission_fails_closed() {
    let mut registry = CommandRegistry::new(2, 3600);
    for i in 0..2 {
        let key = format!("k{i}");
        let id = format!("cmd{i}");
        registry
            .reserve(
                &key,
                identity(&format!("c{i}"), 0),
                record(&id, &key, CommandState::Running),
                now(),
            )
            .unwrap();
    }

    assert_eq!(
        registry.reserve(
            "k9",
            identity("c9", 0),
            record("cmd9", "k9", CommandState::Running),
            now()
        ),
        Err(ReserveError::Capacity),
        "capacity pressure must not be relieved by evicting running work"
    );
    assert!(registry.get("cmd0").is_some());
    assert!(registry.get("cmd1").is_some());
    assert!(
        registry.get("cmd9").is_none(),
        "a refused reservation leaves nothing behind"
    );
    assert_eq!(registry.command_len(), 2);
    assert_eq!(registry.idempotency_len(), 2);
}

#[test]
fn a_key_whose_command_record_vanished_fails_closed_as_a_mismatch() {
    // The pairing invariant should make this unreachable; if it is ever broken,
    // refusing the key is strictly safer than re-running the side effect.
    let mut registry = CommandRegistry::new(4, 3600);
    registry
        .reserve(
            "k1",
            identity("c1", 0),
            record("cmd1", "k1", CommandState::Succeeded),
            now(),
        )
        .unwrap();
    registry.complete(
        "cmd1",
        CommandOutcome {
            state: CommandState::Failed,
            result_revision: 1,
            detail: Some("nope".to_string()),
            error_code: Some(ErrorCode::TargetIneligible),
        },
    );
    let stored = registry.get("cmd1").unwrap();
    assert_eq!(stored.error_code, Some(ErrorCode::TargetIneligible));
    assert_eq!(stored.state, CommandState::Failed);
}

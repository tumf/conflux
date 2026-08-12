//! The stable machine-readable result contract of `cflx client`.
//!
//! One object on stdout per invocation, one stable outcome token per situation,
//! and one exit status per outcome. That triple is the whole reason this
//! namespace exists: an agent that has to branch on prose, on an HTTP status, or
//! on whether stdout happened to contain a progress line is exactly as brittle
//! as one that speaks `/api/v2` directly.
//!
//! Everything here is deliberately transport-free. API wire objects may travel
//! inside `detail` as diagnostics, but no caller needs them to decide whether the
//! operation succeeded.

// A build without the local API reaches only the feature-unavailable refusal, so
// the envelope's builders have no caller there. They stay compiled anyway: the
// contract assertions below are what keep the outcome/exit-code table honest.
#![cfg_attr(not(feature = "web-monitoring"), allow(dead_code))]

use serde::{Deserialize, Serialize};

/// Envelope version. Bumped only when an existing field changes meaning.
pub const SCHEMA_VERSION: u32 = 1;

/// The operation an envelope reports on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// `cflx client status`
    Status,
    /// `cflx client enqueue`
    Enqueue,
    /// `cflx client wait`
    Wait,
}

impl Operation {
    /// Wire representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Enqueue => "enqueue",
            Self::Wait => "wait",
        }
    }
}

/// Every stable outcome token this CLI can report.
///
/// Successful and unsuccessful outcomes live in one enum because the *pair*
/// (outcome, exit status) is the contract: a caller that learned to branch on
/// `outcome` must never have to also ask which enum it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    // ── Successful ────────────────────────────────────────────────────────
    /// `status` read one coherent snapshot of a compatible owner.
    Observed,
    /// `enqueue` admitted the requested change.
    Admitted,
    /// `enqueue` found the change already admitted; nothing was submitted.
    AlreadyAdmitted,
    /// `wait` verified the change's successful terminal outcome.
    Completed,

    // ── Unsuccessful ──────────────────────────────────────────────────────
    /// No Git repository, so no default socket identity exists.
    NotInRepository,
    /// Nothing is listening at the selected socket.
    OwnerNotRunning,
    /// The owner refused the presented credentials, or demanded some.
    AuthenticationFailed,
    /// The owner lacks a required route, field, or API version.
    IncompatibleOwner,
    /// The owner serves reads but has no command executor bound.
    OwnerNotCommandCapable,
    /// The socket began serving a different process incarnation.
    OwnerRestarted,
    /// The owner does not track the requested change.
    ChangeNotFound,
    /// The change cannot accept the requested admission right now.
    TargetIneligible,
    /// Acting would have consumed or discarded another operator's intent.
    OperatorIntentConflict,
    /// Bounded stale-revision recomputation was exhausted.
    RevisionConflict,
    /// Bounded rereads could not produce one coherent observation.
    ObservationConflict,
    /// A submitted command settled as a typed failure.
    CommandFailed,
    /// Part of a multi-command intent settled and the rest did not.
    PartialIntent,
    /// The owner's terminal mode has no repository proof this build can check.
    UnsupportedTerminalMode,
    /// Repository completion evidence was contradictory or unreadable.
    ///
    /// Distinct from "not finished yet": a base tree holding both an archive
    /// entry and the live change directory, or a base branch that cannot be
    /// read at all, says the proof is *broken*, and collapsing that into
    /// "keep waiting" would hide a repository an operator has to look at.
    EvidenceError,
    /// The change reached a terminal rejection.
    ChangeRejected,
    /// The owner reported a fatal process-level error.
    ProcessFailed,
    /// The deadline passed with no terminal outcome.
    Timeout,
    /// This build cannot speak to a local owner at all.
    FeatureUnavailable,
    /// The socket was reachable but the exchange failed.
    TransportError,
    /// The invocation itself was not usable.
    UsageError,
}

impl Outcome {
    /// Wire representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Admitted => "admitted",
            Self::AlreadyAdmitted => "already_admitted",
            Self::Completed => "completed",
            Self::NotInRepository => "not_in_repository",
            Self::OwnerNotRunning => "owner_not_running",
            Self::AuthenticationFailed => "authentication_failed",
            Self::IncompatibleOwner => "incompatible_owner",
            Self::OwnerNotCommandCapable => "owner_not_command_capable",
            Self::OwnerRestarted => "owner_restarted",
            Self::ChangeNotFound => "change_not_found",
            Self::TargetIneligible => "target_ineligible",
            Self::OperatorIntentConflict => "operator_intent_conflict",
            Self::RevisionConflict => "revision_conflict",
            Self::ObservationConflict => "observation_conflict",
            Self::CommandFailed => "command_failed",
            Self::PartialIntent => "partial_intent",
            Self::UnsupportedTerminalMode => "unsupported_terminal_mode",
            Self::EvidenceError => "evidence_error",
            Self::ChangeRejected => "change_rejected",
            Self::ProcessFailed => "process_failed",
            Self::Timeout => "timeout",
            Self::FeatureUnavailable => "feature_unavailable",
            Self::TransportError => "transport_error",
            Self::UsageError => "usage_error",
        }
    }

    /// Whether this outcome means the requested operation reached its goal.
    ///
    /// The list is short on purpose. Admission is not completion, a settled
    /// command is not an archived change, and a partially applied intent is not
    /// an applied one.
    pub fn is_success(self) -> bool {
        matches!(
            self,
            Self::Observed | Self::Admitted | Self::AlreadyAdmitted | Self::Completed
        )
    }

    /// Process exit status for this outcome.
    ///
    /// Distinct per outcome so a shell script can branch without parsing JSON,
    /// and stable: a code is never reused for a different meaning. `2` is
    /// reserved for usage errors to match the convention the rest of the CLI
    /// already follows.
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Observed | Self::Admitted | Self::AlreadyAdmitted | Self::Completed => 0,
            Self::UsageError => 2,
            Self::NotInRepository => 3,
            Self::OwnerNotRunning => 4,
            Self::AuthenticationFailed => 5,
            Self::IncompatibleOwner => 6,
            Self::OwnerNotCommandCapable => 7,
            Self::OwnerRestarted => 8,
            Self::ChangeNotFound => 9,
            Self::TargetIneligible => 10,
            Self::OperatorIntentConflict => 11,
            Self::RevisionConflict => 12,
            Self::ObservationConflict => 13,
            Self::CommandFailed => 14,
            Self::PartialIntent => 15,
            Self::UnsupportedTerminalMode => 16,
            Self::EvidenceError => 22,
            Self::ChangeRejected => 17,
            Self::ProcessFailed => 18,
            Self::Timeout => 19,
            Self::FeatureUnavailable => 20,
            Self::TransportError => 21,
        }
    }
}

/// Every outcome, in the order the contract advertises them.
///
/// Read by the CLI's own contract assertions, which is what stops an added
/// variant from silently reusing an exit status.
#[allow(dead_code)] // Read by the outcome-contract assertions, not by the binary.
pub const ALL_OUTCOMES: [Outcome; 25] = [
    Outcome::Observed,
    Outcome::Admitted,
    Outcome::AlreadyAdmitted,
    Outcome::Completed,
    Outcome::NotInRepository,
    Outcome::OwnerNotRunning,
    Outcome::AuthenticationFailed,
    Outcome::IncompatibleOwner,
    Outcome::OwnerNotCommandCapable,
    Outcome::OwnerRestarted,
    Outcome::ChangeNotFound,
    Outcome::TargetIneligible,
    Outcome::OperatorIntentConflict,
    Outcome::RevisionConflict,
    Outcome::ObservationConflict,
    Outcome::CommandFailed,
    Outcome::PartialIntent,
    Outcome::UnsupportedTerminalMode,
    Outcome::EvidenceError,
    Outcome::ChangeRejected,
    Outcome::ProcessFailed,
    Outcome::Timeout,
    Outcome::FeatureUnavailable,
    Outcome::TransportError,
    Outcome::UsageError,
];

/// The single object a `--json` invocation writes to stdout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultEnvelope {
    /// Envelope schema version.
    pub schema_version: u32,
    /// True when the operation reached its successful outcome.
    pub ok: bool,
    /// Which client operation this reports on.
    pub operation: Operation,
    /// Stable outcome token.
    pub outcome: Outcome,
    /// Owner incarnation the result was observed at, when one was reached.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    /// Change the operation addressed, when it addressed one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_id: Option<String>,
    /// Sanitized human-readable explanation. Never carries a credential.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Operation-specific facts. Always an object, empty when there are none.
    pub detail: serde_json::Value,
}

impl ResultEnvelope {
    /// Build an envelope for one operation and outcome.
    pub fn new(operation: Operation, outcome: Outcome) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            ok: outcome.is_success(),
            operation,
            outcome,
            instance_id: None,
            change_id: None,
            message: None,
            detail: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Attach the owner incarnation this result belongs to.
    pub fn with_instance(mut self, instance_id: Option<String>) -> Self {
        self.instance_id = instance_id;
        self
    }

    /// Attach the addressed change.
    pub fn with_change(mut self, change_id: impl Into<String>) -> Self {
        self.change_id = Some(change_id.into());
        self
    }

    /// Attach a sanitized explanation.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Attach operation-specific detail.
    pub fn with_detail(mut self, detail: serde_json::Value) -> Self {
        self.detail = detail;
        self
    }

    /// Exit status this envelope reports.
    pub fn exit_code(&self) -> i32 {
        self.outcome.exit_code()
    }

    /// Render the JSON form: exactly one object, one trailing newline.
    pub fn to_json_line(&self) -> String {
        // Serialization of a closed struct of owned values cannot fail, but a
        // panic here would be a silent contract break, so the fallback is still
        // a parseable envelope.
        serde_json::to_string(self).unwrap_or_else(|_| {
            format!(
                r#"{{"schema_version":{SCHEMA_VERSION},"ok":false,"operation":"{}","outcome":"transport_error","detail":{{}}}}"#,
                self.operation.as_str()
            )
        })
    }

    /// Render the concise human form.
    ///
    /// One line, outcome first, so `cflx client wait alpha | tail -1` reads the
    /// same way the JSON does.
    pub fn to_human_line(&self) -> String {
        let mut line = String::new();
        if let Some(change_id) = &self.change_id {
            line.push_str(&format!(
                "{}: {} ({change_id})",
                self.operation.as_str(),
                self.outcome.as_str()
            ));
        } else {
            line.push_str(&format!(
                "{}: {}",
                self.operation.as_str(),
                self.outcome.as_str()
            ));
        }
        if let Some(message) = &self.message {
            line.push_str(&format!(" — {message}"));
        }
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_outcome_has_a_distinct_stable_exit_code() {
        let mut codes = BTreeSet::new();
        for outcome in ALL_OUTCOMES {
            if outcome.is_success() {
                assert_eq!(outcome.exit_code(), 0, "{}", outcome.as_str());
                continue;
            }
            assert!(
                codes.insert(outcome.exit_code()),
                "{} reuses exit code {}",
                outcome.as_str(),
                outcome.exit_code()
            );
        }
        assert_eq!(codes.len(), ALL_OUTCOMES.len() - 4);
    }

    #[test]
    fn every_outcome_token_is_unique_and_snake_case() {
        let mut tokens = BTreeSet::new();
        for outcome in ALL_OUTCOMES {
            let token = outcome.as_str();
            assert!(tokens.insert(token), "{token} appears twice");
            assert!(
                token.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
                "{token} is not snake_case"
            );
            // The serde representation and the branching token are the same
            // string, so a client cannot read one and match on the other.
            let serialized = serde_json::to_string(&outcome).unwrap();
            assert_eq!(serialized, format!("\"{token}\""));
        }
        assert_eq!(tokens.len(), ALL_OUTCOMES.len());
    }

    #[test]
    fn success_is_limited_to_reached_goals() {
        let successes: Vec<&str> = ALL_OUTCOMES
            .iter()
            .filter(|outcome| outcome.is_success())
            .map(|outcome| outcome.as_str())
            .collect();
        assert_eq!(
            successes,
            vec!["observed", "admitted", "already_admitted", "completed"]
        );
    }

    #[test]
    fn a_json_envelope_is_one_object_with_a_stable_shape() {
        let envelope = ResultEnvelope::new(Operation::Enqueue, Outcome::Admitted)
            .with_instance(Some("abc".to_string()))
            .with_change("alpha");
        let line = envelope.to_json_line();
        assert!(!line.contains('\n'), "the envelope must be one line");
        let parsed: serde_json::Value = serde_json::from_str(&line).expect("parses");
        assert_eq!(parsed["schema_version"], SCHEMA_VERSION);
        assert_eq!(parsed["ok"], true);
        assert_eq!(parsed["operation"], "enqueue");
        assert_eq!(parsed["outcome"], "admitted");
        assert_eq!(parsed["instance_id"], "abc");
        assert_eq!(parsed["change_id"], "alpha");
        assert!(parsed["detail"].is_object());
    }

    #[test]
    fn absent_optional_fields_are_omitted_rather_than_nulled() {
        let envelope = ResultEnvelope::new(Operation::Status, Outcome::OwnerNotRunning);
        let parsed: serde_json::Value = serde_json::from_str(&envelope.to_json_line()).unwrap();
        let object = parsed.as_object().unwrap();
        assert!(!object.contains_key("instance_id"));
        assert!(!object.contains_key("change_id"));
        assert!(!object.contains_key("message"));
        assert_eq!(parsed["ok"], false);
        assert_eq!(envelope.exit_code(), 4);
    }

    #[test]
    fn the_human_line_names_the_operation_the_outcome_and_the_change() {
        let line = ResultEnvelope::new(Operation::Wait, Outcome::Timeout)
            .with_change("alpha")
            .with_message("deadline reached")
            .to_human_line();
        assert_eq!(line, "wait: timeout (alpha) — deadline reached");
    }
}

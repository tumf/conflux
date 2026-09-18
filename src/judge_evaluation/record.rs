//! Schema-v1 evaluation record types.
//!
//! One record is exactly one JSON object on one line. Everything here is
//! *disposable observability*: nothing in this module is read by analysis
//! normalization, provenance, queue reduction, dispatch, blocker
//! classification, Acceptance, Archive, or lifecycle routing.
//!
//! The privacy contract is enforced by construction rather than by a filter:
//! there is no field that can hold a change ID, proposal text, a request or
//! response body, a command string, an environment value, a repository path, a
//! credential, or a provider error body. A pair is named only by its opaque
//! salted identifier.

use serde::{Deserialize, Serialize};

/// The only record schema version this release writes.
///
/// A reader that meets a higher version counts the line as unsupported and
/// excludes it from every metric rather than guessing at its meaning.
pub const EVALUATION_SCHEMA_VERSION: u32 = 1;

/// One directed `(dependent, dependency)` judgment paired with the
/// authoritative analyzer's answer for the same direction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluationPairRecord {
    /// `sha256:<64 lowercase hex>` over the installation salt and the framed
    /// project-scoped directed identity. Local pseudonymization, not an
    /// authentication primitive.
    pub pair_id: String,

    /// The judge's raw probability, kept unthresholded so an alternate
    /// threshold can be recomputed from stored records alone.
    pub judge_probability: f64,

    /// `judge_probability >= yes_threshold` at the time of recording.
    pub judge_dependency: bool,

    /// The conventional analyzer's answer — the only authority — for the same
    /// directed pair.
    pub analyzer_dependency: bool,
}

/// One evaluation record: a valid shadow observation, or a bounded failure.
///
/// Serialization is deliberately flat and `Option`-with-`skip_serializing_if`
/// rather than an enum with a payload: a failure record is the *same* schema
/// minus the fields that only a validated response can supply, which keeps one
/// reader able to account for both without a second schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluationRecord {
    pub schema_version: u32,

    /// RFC3339 UTC instant the record was produced.
    pub recorded_at: String,

    /// Conflux version that produced the record, so a metric can be attributed
    /// to the build that measured it.
    pub conflux_version: String,

    /// Judgment purpose; `parallel_dependency` is the only one recorded here.
    pub purpose: String,

    /// Judge mode in effect; `shadow` is the only accepted value.
    pub mode: String,

    /// Stable outcome token. `observed` for a valid observation, otherwise the
    /// verbatim [`crate::judge_command::JudgeFailureCategory::as_str`] token.
    pub outcome: String,

    /// Measured invocation duration.
    pub duration_ms: u64,

    /// Pairs the request asked about.
    pub expected_answers: usize,

    /// Validated model identity. Absent unless a response passed schema
    /// validation: an unvalidated model identity is not evidence of anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Pairs the response actually answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returned_answers: Option<usize>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,

    /// Threshold used to project probabilities onto comparison edges.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yes_threshold: Option<f64>,

    /// Per-pair judgments. Absent on a failure record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pairs: Option<Vec<EvaluationPairRecord>>,
}

/// Outcome token for a valid observation.
pub const OUTCOME_OBSERVED: &str = "observed";

impl EvaluationRecord {
    /// Whether this record is a valid observation rather than a bounded failure.
    pub fn is_observed(&self) -> bool {
        self.outcome == OUTCOME_OBSERVED
    }

    /// Serialize to exactly one newline-terminated line.
    ///
    /// A record whose JSON would contain a literal newline cannot exist —
    /// `serde_json` escapes them — so one write is one complete line and a
    /// reader can split on `\n` without a framing protocol.
    pub fn to_jsonl(&self) -> serde_json::Result<String> {
        let mut line = serde_json::to_string(self)?;
        line.push('\n');
        Ok(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed() -> EvaluationRecord {
        EvaluationRecord {
            schema_version: EVALUATION_SCHEMA_VERSION,
            recorded_at: "2026-09-18T00:00:00Z".to_string(),
            conflux_version: "0.6.324".to_string(),
            purpose: "parallel_dependency".to_string(),
            mode: "shadow".to_string(),
            outcome: OUTCOME_OBSERVED.to_string(),
            duration_ms: 892,
            expected_answers: 2,
            model: Some("jev-1.13.0".to_string()),
            returned_answers: Some(2),
            input_tokens: Some(1038),
            output_tokens: Some(56),
            yes_threshold: Some(0.8),
            pairs: Some(vec![EvaluationPairRecord {
                pair_id: "sha256:aa".to_string(),
                judge_probability: 0.97,
                judge_dependency: true,
                analyzer_dependency: true,
            }]),
        }
    }

    #[test]
    fn observed_record_round_trips_as_one_line() {
        let record = observed();
        let line = record.to_jsonl().expect("record must serialize");

        assert!(line.ends_with('\n'));
        assert_eq!(line.matches('\n').count(), 1, "one record is one line");

        let parsed: EvaluationRecord =
            serde_json::from_str(line.trim_end()).expect("record must round-trip");
        assert_eq!(parsed, record);
        assert!(parsed.is_observed());
    }

    #[test]
    fn failure_record_omits_response_only_fields() {
        let record = EvaluationRecord {
            outcome: "timeout".to_string(),
            model: None,
            returned_answers: None,
            input_tokens: None,
            output_tokens: None,
            yes_threshold: None,
            pairs: None,
            ..observed()
        };

        let line = record.to_jsonl().expect("record must serialize");
        for absent in [
            "model",
            "returned_answers",
            "input_tokens",
            "output_tokens",
            "yes_threshold",
            "pairs",
        ] {
            assert!(
                !line.contains(absent),
                "failure record must not carry `{absent}`: {line}"
            );
        }
        assert!(!record.is_observed());
        assert!(line.contains("\"outcome\":\"timeout\""));
    }

    #[test]
    fn every_failure_category_maps_to_its_own_stable_token() {
        use crate::judge_command::JudgeFailureCategory as C;

        // The schema-v1 outcome vocabulary is the judge's own token set
        // verbatim: no renaming, no aggregation, no merged "other" bucket.
        let expected = [
            (C::Spawn, "spawn"),
            (C::InputTooLarge, "input_too_large"),
            (C::Timeout, "timeout"),
            (C::Cancelled, "cancelled"),
            (C::NonzeroExit, "nonzero_exit"),
            (C::InvalidRequest, "invalid_request"),
            (C::Authentication, "authentication"),
            (C::Transient, "transient"),
            (C::OutputTooLarge, "output_too_large"),
            (C::InvalidJson, "invalid_json"),
            (C::SchemaMismatch, "schema_mismatch"),
            (C::Io, "io"),
        ];

        let mut tokens = std::collections::BTreeSet::new();
        for (category, token) in expected {
            assert_eq!(category.as_str(), token);
            assert!(tokens.insert(token), "outcome tokens must be distinct");
            assert_ne!(token, OUTCOME_OBSERVED, "a failure is never `observed`");
        }
        assert_eq!(tokens.len(), 12);
    }
}

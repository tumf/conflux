//! Acceptance test output parsing module.
//!
//! This module provides functions to parse acceptance test output
//! and determine pass/fail status with findings.
//!
//! Verdict contract (post `adopt-json-acceptance-verdict`):
//!
//! - **Primary**: a strict JSON verdict object of the form
//!   `{"acceptance":"pass|fail|continue|gated","findings":[...]}` emitted as
//!   the final machine-readable verdict payload. JSON verdicts may appear
//!   directly as a line on stdout, or wrapped inside a supported JSONL event
//!   text payload. In either case the runtime unwraps the payload and evaluates
//!   the JSON verdict.
//! - **Fallback**: legacy plain-text standalone verdict markers of the form
//!   `ACCEPTANCE: PASS|FAIL|CONTINUE|GATED` remain supported for backward
//!   compatibility, but JSON takes priority whenever both are present.

/// Supported explicit blocker categories for a validated stalled hold.
///
/// The category is always supplied explicitly by the acceptance reviewer. The
/// runtime never derives it by searching narrative prose for words such as
/// `credential`, `token`, or `auth`: an unsupported or absent category is a
/// protocol error, not an invitation to guess.
pub const SUPPORTED_BLOCKER_CATEGORIES: &[&str] = &[
    "credential",
    "external_approval",
    "policy",
    "external_service",
    "pending_verification",
    "infrastructure",
    "schema_incompatibility",
    "human_decision",
];

/// Structured, validated external blocker payload accompanying a `gated`
/// compatibility verdict.
///
/// Every field here is authored by the acceptance reviewer. A payload that
/// satisfies [`validate_acceptance_blocker`] is the *only* input that may enter
/// the in-memory `stalled` hold; anything weaker stays on the bounded
/// protocol-error path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceBlocker {
    /// Explicit category drawn from [`SUPPORTED_BLOCKER_CATEGORIES`].
    pub category: String,
    /// Concrete non-empty evidence for the external prerequisite.
    pub evidence: Vec<String>,
    /// Operator-facing action that can unblock the hold.
    pub next_action: String,
    /// Whether acceptance can resume once the prerequisite is satisfied.
    pub resumable: bool,
    /// Optional owning team/role for the prerequisite.
    pub prerequisite_owner: Option<String>,
    /// Optional stable identifiers for the evidence entries.
    pub evidence_ids: Vec<String>,
}

impl AcceptanceBlocker {
    /// Operator-facing blocker view for lifecycle events and status displays.
    ///
    /// The category is copied verbatim; it is never re-derived from prose. This
    /// is the single projection serial and parallel share, so equivalent
    /// blockers reach the reducer as equivalent `stalled` presentations.
    pub fn to_stalled_blocker(&self) -> crate::events::StalledBlocker {
        crate::events::StalledBlocker {
            category: self.category.clone(),
            phase: "acceptance".to_string(),
            gate: "acceptance".to_string(),
            error_summary: format!(
                "validated external acceptance blocker ({}): {}",
                self.category,
                self.evidence.join(" | ")
            ),
            evidence: self.evidence.clone(),
            next_action: self.next_action.clone(),
            resumable: self.resumable,
            worktree_preserved: true,
        }
    }
}

/// Why a `gated`/legacy-`blocked` verdict failed to qualify as a validated
/// stalled blocker. Every variant routes to the bounded protocol-error path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockerRejection {
    /// No `blocker` payload accompanied the compatibility verdict at all
    /// (bare `{"acceptance":"gated"}` or plain-text `ACCEPTANCE: GATED`).
    Missing,
    /// The `blocker` field was present but not a JSON object.
    NotAnObject,
    /// `category` was absent or not a string.
    MissingCategory,
    /// `category` was present but is not in [`SUPPORTED_BLOCKER_CATEGORIES`].
    UnsupportedCategory(String),
    /// `evidence` was absent, not an array, or contained no non-blank entry.
    EmptyEvidence,
    /// `next_action` was absent or blank.
    MissingNextAction,
    /// `resumable` was absent or not a boolean.
    MissingResumable,
}

impl BlockerRejection {
    /// Short operator-facing reason, used in protocol-retry diagnostics.
    pub fn reason(&self) -> String {
        match self {
            BlockerRejection::Missing => {
                "no structured blocker payload accompanied the gated verdict".to_string()
            }
            BlockerRejection::NotAnObject => "blocker payload is not a JSON object".to_string(),
            BlockerRejection::MissingCategory => {
                "blocker payload has no explicit category".to_string()
            }
            BlockerRejection::UnsupportedCategory(category) => format!(
                "blocker category '{category}' is not one of: {}",
                SUPPORTED_BLOCKER_CATEGORIES.join(", ")
            ),
            BlockerRejection::EmptyEvidence => {
                "blocker payload has no concrete evidence entries".to_string()
            }
            BlockerRejection::MissingNextAction => "blocker payload has no next_action".to_string(),
            BlockerRejection::MissingResumable => {
                "blocker payload has no boolean resumable field".to_string()
            }
        }
    }
}

/// Validate a raw `blocker` payload from a `gated` verdict object.
///
/// Validation is purely structural and explicit: the category must be one the
/// runtime supports, evidence must be concrete, and the reviewer must state
/// both a next action and resumability. Nothing is inferred from prose.
pub fn validate_acceptance_blocker(
    payload: Option<&serde_json::Value>,
) -> std::result::Result<AcceptanceBlocker, BlockerRejection> {
    let Some(payload) = payload else {
        return Err(BlockerRejection::Missing);
    };
    if payload.is_null() {
        return Err(BlockerRejection::Missing);
    }
    let object = payload.as_object().ok_or(BlockerRejection::NotAnObject)?;

    let category = object
        .get("category")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(BlockerRejection::MissingCategory)?;
    if !SUPPORTED_BLOCKER_CATEGORIES.contains(&category) {
        return Err(BlockerRejection::UnsupportedCategory(category.to_string()));
    }

    let evidence = string_list(object.get("evidence"));
    if evidence.is_empty() {
        return Err(BlockerRejection::EmptyEvidence);
    }

    let next_action = object
        .get("next_action")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(BlockerRejection::MissingNextAction)?;

    let resumable = object
        .get("resumable")
        .and_then(serde_json::Value::as_bool)
        .ok_or(BlockerRejection::MissingResumable)?;

    Ok(AcceptanceBlocker {
        category: category.to_string(),
        evidence,
        next_action: next_action.to_string(),
        resumable,
        prerequisite_owner: object
            .get("prerequisite_owner")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        evidence_ids: string_list(object.get("evidence_ids")),
    })
}

/// Severity of a structured repository finding.
///
/// Both severities block PASS. The distinction exists only so operators can
/// triage; it never changes routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingSeverity {
    /// Reviewer-declared major defect.
    Major,
    /// Reviewer-declared minor defect. Still blocks PASS.
    Minor,
}

impl FindingSeverity {
    /// Canonical lowercase token used in the verdict schema.
    pub fn as_str(self) -> &'static str {
        match self {
            FindingSeverity::Major => "major",
            FindingSeverity::Minor => "minor",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "major" => Some(FindingSeverity::Major),
            "minor" => Some(FindingSeverity::Minor),
            _ => None,
        }
    }
}

impl std::fmt::Display for FindingSeverity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One declared repository expectation: a file plus what must become true there.
///
/// Used for both `required_changes` (implementation) and `verification`
/// (proof). `file` is always a normalized repository-relative path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingFileExpectation {
    /// Normalized repository-relative path.
    pub file: String,
    /// Expected behavior or proof at that path.
    pub description: String,
}

/// A structured, actionable repository finding.
///
/// `id` is the authoritative retry identity. It is stable for the same defect
/// across attempts and is never derived from the mutable `summary`, evidence
/// prose, or line numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryFinding {
    /// Stable reviewer-authored identity for this defect.
    pub id: String,
    /// `major` or `minor`; both block PASS.
    pub severity: FindingSeverity,
    /// One-line human-facing summary.
    pub summary: String,
    /// Concrete observed evidence.
    pub evidence: Vec<String>,
    /// Implementation files that must change, with expected behavior.
    pub required_changes: Vec<FindingFileExpectation>,
    /// Verification files that must change, with expected proof.
    pub verification: Vec<FindingFileExpectation>,
}

impl RepositoryFinding {
    /// Declared implementation paths.
    pub fn required_files(&self) -> Vec<String> {
        self.required_changes
            .iter()
            .map(|change| change.file.clone())
            .collect()
    }

    /// Declared verification paths.
    pub fn verification_files(&self) -> Vec<String> {
        self.verification
            .iter()
            .map(|change| change.file.clone())
            .collect()
    }

    /// Machine-readable payload handed to Apply, verbatim and complete.
    pub fn to_json(&self) -> serde_json::Value {
        let expectations = |items: &[FindingFileExpectation]| {
            items
                .iter()
                .map(|item| serde_json::json!({"file": item.file, "description": item.description}))
                .collect::<Vec<_>>()
        };
        serde_json::json!({
            "id": self.id,
            "severity": self.severity.as_str(),
            "summary": self.summary,
            "evidence": self.evidence,
            "required_changes": expectations(&self.required_changes),
            "verification": expectations(&self.verification),
        })
    }

    /// Single-line rendering that keeps every actionable field.
    ///
    /// Runtime-owned follow-up records are line-oriented, so the complete
    /// payload has to survive as one line without losing evidence, required
    /// changes, or verification expectations.
    pub fn to_single_line(&self) -> String {
        let render = |items: &[FindingFileExpectation]| {
            items
                .iter()
                .map(|item| format!("{} — {}", item.file, item.description))
                .collect::<Vec<_>>()
                .join("; ")
        };
        format!(
            "[{}] ({}) {} | evidence: {} | required_changes: {} | verification: {}",
            self.id,
            self.severity,
            self.summary,
            self.evidence.join("; "),
            render(&self.required_changes),
            render(&self.verification),
        )
    }
}

/// Why an object entry in `findings` failed to qualify as a structured
/// repository finding. Every variant routes to bounded protocol handling: none
/// of them may degrade into a normalized path-only repair instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingRejection {
    /// The entry was neither a legacy string nor a JSON object.
    NotStringOrObject,
    /// `id` was absent or blank.
    MissingId,
    /// Two findings in the same verdict claimed the same `id`.
    DuplicateId(String),
    /// `severity` was absent or not `major`/`minor`.
    UnsupportedSeverity(String),
    /// `summary` was absent or blank.
    MissingSummary,
    /// `evidence` was absent, not an array, or had no concrete entry.
    EmptyEvidence,
    /// `required_changes` was absent or contained no valid entry.
    EmptyRequiredChanges,
    /// `verification` was absent or contained no valid entry.
    EmptyVerification,
    /// A declared path was absolute, escaped the workspace, or was blank.
    InvalidPath { field: &'static str, path: String },
    /// A declared entry had a path but no description.
    MissingDescription { field: &'static str, path: String },
}

impl FindingRejection {
    /// Short operator-facing reason, used in protocol-retry diagnostics.
    pub fn reason(&self) -> String {
        match self {
            FindingRejection::NotStringOrObject => {
                "finding entry is neither a legacy string nor a structured object".to_string()
            }
            FindingRejection::MissingId => {
                "structured finding has no stable non-empty id".to_string()
            }
            FindingRejection::DuplicateId(id) => {
                format!("structured finding id '{id}' appears more than once in one verdict")
            }
            FindingRejection::UnsupportedSeverity(severity) => {
                format!("structured finding severity '{severity}' is not 'major' or 'minor'")
            }
            FindingRejection::MissingSummary => {
                "structured finding has no non-empty summary".to_string()
            }
            FindingRejection::EmptyEvidence => {
                "structured finding has no concrete evidence entries".to_string()
            }
            FindingRejection::EmptyRequiredChanges => {
                "structured finding declares no required_changes entry".to_string()
            }
            FindingRejection::EmptyVerification => {
                "structured finding declares no verification entry".to_string()
            }
            FindingRejection::InvalidPath { field, path } => format!(
                "structured finding {field} path '{path}' is not a repository-relative path inside \
                 the workspace"
            ),
            FindingRejection::MissingDescription { field, path } => format!(
                "structured finding {field} entry for '{path}' has no description of the expected \
                 behavior or proof"
            ),
        }
    }
}

/// Normalize a declared path to a repository-relative form.
///
/// Returns `None` for absolute paths, Windows drive prefixes, blank input, and
/// anything containing a `..` component: a finding may never point outside the
/// workspace.
pub fn normalize_repository_path(raw: &str) -> Option<String> {
    let candidate = raw.trim().replace('\\', "/");
    if candidate.is_empty() || candidate.starts_with('/') || candidate.starts_with('~') {
        return None;
    }
    // Reject `C:/...` style absolute paths without rejecting `src/a.rs:10`.
    if candidate.len() >= 2 && candidate.as_bytes()[1] == b':' {
        return None;
    }
    let mut components = Vec::new();
    for component in candidate.split('/') {
        match component {
            "" | "." => continue,
            ".." => return None,
            other => components.push(other),
        }
    }
    (!components.is_empty()).then(|| components.join("/"))
}

fn parse_expectations(
    value: Option<&serde_json::Value>,
    field: &'static str,
) -> std::result::Result<Vec<FindingFileExpectation>, FindingRejection> {
    let entries = value
        .and_then(|value| value.as_array())
        .ok_or(match field {
            "required_changes" => FindingRejection::EmptyRequiredChanges,
            _ => FindingRejection::EmptyVerification,
        })?;
    let mut expectations = Vec::new();
    for entry in entries {
        let object = entry.as_object().ok_or(FindingRejection::InvalidPath {
            field,
            path: entry.to_string(),
        })?;
        let raw_path = object
            .get("file")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let file =
            normalize_repository_path(raw_path).ok_or_else(|| FindingRejection::InvalidPath {
                field,
                path: raw_path.to_string(),
            })?;
        let description = object
            .get("description")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| FindingRejection::MissingDescription {
                field,
                path: file.clone(),
            })?;
        expectations.push(FindingFileExpectation {
            file,
            description: description.to_string(),
        });
    }
    if expectations.is_empty() {
        return Err(match field {
            "required_changes" => FindingRejection::EmptyRequiredChanges,
            _ => FindingRejection::EmptyVerification,
        });
    }
    Ok(expectations)
}

/// Validate one object entry of a FAIL `findings` array.
///
/// Validation is purely structural. Nothing is inferred from prose, and a
/// failure never degrades into a lossy path-only instruction.
pub fn validate_repository_finding(
    value: &serde_json::Value,
) -> std::result::Result<RepositoryFinding, FindingRejection> {
    let object = value
        .as_object()
        .ok_or(FindingRejection::NotStringOrObject)?;

    let id = object
        .get("id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(FindingRejection::MissingId)?;

    let raw_severity = object
        .get("severity")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or_default();
    let severity = FindingSeverity::parse(raw_severity)
        .ok_or_else(|| FindingRejection::UnsupportedSeverity(raw_severity.to_string()))?;

    let summary = object
        .get("summary")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(FindingRejection::MissingSummary)?;

    let evidence = string_list(object.get("evidence"));
    if evidence.is_empty() {
        return Err(FindingRejection::EmptyEvidence);
    }

    Ok(RepositoryFinding {
        id: id.to_string(),
        severity,
        summary: summary.to_string(),
        evidence,
        required_changes: parse_expectations(object.get("required_changes"), "required_changes")?,
        verification: parse_expectations(object.get("verification"), "verification")?,
    })
}

/// One acceptance finding as it travels through history, follow-up state,
/// prompts, and retry accounting.
///
/// This is the single shared representation. A structured finding keeps its
/// complete payload; a legacy string finding keeps its complete original text.
/// Neither is ever replaced by its normalized comparison identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceFinding {
    text: String,
    structured: Option<RepositoryFinding>,
}

impl AcceptanceFinding {
    /// Wrap a legacy string finding, preserving its complete original text.
    pub fn legacy(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            structured: None,
        }
    }

    /// Wrap a validated structured finding.
    pub fn structured(finding: RepositoryFinding) -> Self {
        Self {
            text: finding.to_single_line(),
            structured: Some(finding),
        }
    }

    /// Complete actionable text. Never a normalized identity.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Structured payload, when the reviewer supplied one.
    pub fn structured_payload(&self) -> Option<&RepositoryFinding> {
        self.structured.as_ref()
    }

    /// Stable reviewer-authored ID, when the finding is structured.
    pub fn id(&self) -> Option<&str> {
        self.structured.as_ref().map(|finding| finding.id.as_str())
    }

    /// Whether this finding declares required implementation/verification paths
    /// that strict diff coverage can enforce.
    pub fn declares_paths(&self) -> bool {
        self.structured.is_some()
    }

    /// Machine-readable payload for the Apply repair prompt.
    pub fn to_json(&self) -> serde_json::Value {
        match &self.structured {
            Some(finding) => finding.to_json(),
            None => serde_json::json!({"finding": self.text}),
        }
    }
}

impl From<String> for AcceptanceFinding {
    fn from(value: String) -> Self {
        Self::legacy(value)
    }
}

impl From<&str> for AcceptanceFinding {
    fn from(value: &str) -> Self {
        Self::legacy(value)
    }
}

impl std::ops::Deref for AcceptanceFinding {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.text
    }
}

impl AsRef<str> for AcceptanceFinding {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

impl std::fmt::Display for AcceptanceFinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.text)
    }
}

// Text-equality conveniences. Comparing against a plain string checks the
// complete finding text; it never compares a normalized identity.
impl PartialEq<str> for AcceptanceFinding {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

impl PartialEq<&str> for AcceptanceFinding {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

impl PartialEq<String> for AcceptanceFinding {
    fn eq(&self, other: &String) -> bool {
        &self.text == other
    }
}

/// Complete texts of a finding list, in order.
pub fn finding_texts(findings: &[AcceptanceFinding]) -> Vec<String> {
    findings
        .iter()
        .map(|finding| finding.text().to_string())
        .collect()
}

/// Wrap legacy string findings without changing their text.
pub fn legacy_findings<I, S>(findings: I) -> Vec<AcceptanceFinding>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    findings
        .into_iter()
        .map(|finding| AcceptanceFinding::legacy(finding))
        .collect()
}

/// Parse one `findings` array into shared findings.
///
/// Strings stay legacy. Objects must validate; a malformed object is a protocol
/// error rather than a lossy degradation. Duplicate structured IDs in one
/// verdict are rejected because retry accounting keys on the ID.
pub fn parse_finding_entries(
    entries: &[serde_json::Value],
) -> std::result::Result<Vec<AcceptanceFinding>, FindingRejection> {
    let mut findings = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    for entry in entries {
        if let Some(text) = entry.as_str() {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                continue;
            }
            findings.push(AcceptanceFinding::legacy(trimmed));
            continue;
        }
        let structured = validate_repository_finding(entry)?;
        if !seen_ids.insert(structured.id.clone()) {
            return Err(FindingRejection::DuplicateId(structured.id));
        }
        findings.push(AcceptanceFinding::structured(structured));
    }
    Ok(findings)
}

/// Collect the non-blank string entries of a JSON array field.
fn string_list(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|value| value.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_str())
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Result of parsing acceptance output
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceResult {
    /// Acceptance passed
    Pass,
    /// Acceptance failed with findings
    Fail { findings: Vec<AcceptanceFinding> },
    /// Acceptance requires more investigation - continue later
    Continue,
    /// A FAIL verdict carried a `findings` entry that claimed to be structured
    /// but did not validate.
    ///
    /// Like [`AcceptanceResult::BareBlocker`] this is a bounded protocol error:
    /// the runtime asks for a corrected verdict instead of dispatching ambiguous
    /// path-only repair work.
    MalformedFinding { rejection: FindingRejection },
    /// A `gated` (or legacy `blocked`) compatibility verdict arrived without a
    /// payload that qualifies as a validated external blocker.
    ///
    /// This is an acceptance protocol error, not a stalled hold: it creates no
    /// lifecycle transition, no blocker category, and no durable record. The
    /// runtime retries acceptance within a bounded budget and then reports a
    /// terminal protocol error.
    BareBlocker { rejection: BlockerRejection },
    /// A `gated` compatibility verdict carrying a validated structured external
    /// blocker. Only this variant may enter the durable `stalled` hold.
    Stalled { blocker: AcceptanceBlocker },
    /// No canonical verdict was found in the output. The acceptance command
    /// completed without emitting the machine-readable verdict it owes
    /// (for example a status-only or waiting narrative), which is a protocol
    /// failure distinct from an intentional [`AcceptanceResult::Continue`].
    MissingVerdict,
}

/// Canonical plain-text verdict variants. These remain supported as a
/// backward-compatible fallback when no JSON verdict object is present.
///
/// The runtime requires an unwrapped standalone line that exactly equals one
/// of these markers (after stripping tolerated markdown decorations).
/// Trailing-text concatenation such as `ACCEPTANCE: PASSAll ...` or
/// `ACCEPTANCE: PASS## ...` does NOT satisfy the fallback contract and falls
/// through to the missing-verdict protocol failure.
pub(crate) const CANONICAL_VERDICTS: &[(&str, &str)] = &[
    ("ACCEPTANCE: PASS", "pass"),
    ("ACCEPTANCE: FAIL", "fail"),
    ("ACCEPTANCE: CONTINUE", "continue"),
    ("ACCEPTANCE: GATED", "gated"),
    // Legacy backward-compatibility marker during migration.
    ("ACCEPTANCE: BLOCKED", "gated"),
];

/// A strict JSON acceptance verdict object parsed from one line/text payload.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JsonVerdict {
    /// Canonical verdict kind: `pass`/`fail`/`continue`/`gated`.
    pub kind: &'static str,
    /// Raw `findings` array entries, when present. Validation happens in
    /// [`acceptance_result_from_json`] so a malformed structured entry can be
    /// reported as a protocol error instead of being silently dropped.
    pub findings: Vec<serde_json::Value>,
    /// Raw `blocker` payload, when present. Validated separately so the parser
    /// can report *why* a compatibility verdict failed to qualify as a stall.
    pub blocker: Option<serde_json::Value>,
}

/// Attempt to parse a single line/text payload as a strict JSON acceptance
/// verdict object. Returns `Some(verdict)` when the trimmed content is a JSON
/// object with an `acceptance` field equal to one of
/// `pass`/`fail`/`continue`/`gated` (case-insensitive).
/// Legacy `blocked` input is accepted as backward-compatible alias.
///
/// Accepted shapes:
///
/// ```json
/// {"acceptance":"pass"}
/// {"acceptance":"fail","findings":["src/foo.rs:10 issue"]}
/// {"acceptance":"gated","blocker":{"category":"credential",
///   "evidence":["STAGING_API_KEY is unset in the verification environment"],
///   "next_action":"provision STAGING_API_KEY then retry acceptance",
///   "resumable":true}}
/// ```
pub(crate) fn parse_json_verdict(text: &str) -> Option<JsonVerdict> {
    let trimmed = text.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let obj = value.as_object()?;
    let raw_kind = obj.get("acceptance")?.as_str()?;
    let kind = match raw_kind.trim().to_ascii_lowercase().as_str() {
        "pass" => "pass",
        "fail" => "fail",
        "continue" => "continue",
        "gated" => "gated",
        "blocked" => "gated",
        _ => return None,
    };
    let findings = obj
        .get("findings")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Some(JsonVerdict {
        kind,
        findings,
        blocker: obj.get("blocker").cloned(),
    })
}

/// Detect a canonical verdict kind in a single output line.
///
/// Preference order:
///
/// 1. Strict JSON verdict object on the line itself (primary contract).
/// 2. Strict JSON verdict object inside an unwrapped supported agent JSONL
///    event text payload.
/// 3. Legacy plain-text standalone canonical marker on the line itself, or
///    inside the unwrapped event payload (backward-compatible fallback).
///
/// Returns `None` when no verdict is recognized. This helper is used both for
/// streaming verdict detection (grace-period trigger) and for full-output
/// parsing fallback.
/// Returns the canonical verdict kind for a single output line, or `None` when
/// the line is not a standalone canonical verdict.
///
/// Matching is strict: the line must equal one of [`CANONICAL_VERDICTS`] markers
/// exactly after trimming whitespace and stripping defensively-tolerated markdown
/// decorations (bold/italic/underline + leading heading/blockquote/bullet
/// prefixes). Trailing-text concatenation onto the marker is rejected.
pub(crate) fn canonical_verdict_kind(line: &str) -> Option<&'static str> {
    let normalized = strip_markdown_decorations(line.trim());
    CANONICAL_VERDICTS
        .iter()
        .find(|(marker, _)| normalized == *marker)
        .map(|(_, kind)| *kind)
}

#[cfg(test)]
fn detect_verdict_in_line(line: &str) -> Option<&'static str> {
    VerdictStreamDetector::default().detect(line)
}

#[derive(Default)]
pub(crate) struct VerdictStreamDetector {
    code_fence: Option<(char, usize)>,
}

impl VerdictStreamDetector {
    pub(crate) fn detect(&mut self, line: &str) -> Option<&'static str> {
        if let Some(text) = crate::stream_json_textifier::extract_text_from_stream_json(line.trim())
        {
            for inner in text.lines() {
                if let Some(kind) = self.detect_unwrapped(inner) {
                    return Some(kind);
                }
            }
            return None;
        }
        self.detect_unwrapped(line)
    }

    fn detect_unwrapped(&mut self, line: &str) -> Option<&'static str> {
        let trimmed = line.trim();
        if let Some((marker, length, closing)) = verdict_markdown_fence(trimmed, self.code_fence) {
            if closing {
                self.code_fence = None;
            } else if self.code_fence.is_none() {
                self.code_fence = Some((marker, length));
            }
            return None;
        }
        if self.code_fence.is_some() {
            return None;
        }
        parse_json_verdict(trimmed)
            .map(|verdict| verdict.kind)
            .or_else(|| canonical_verdict_kind(trimmed))
    }
}

fn verdict_markdown_fence(line: &str, open: Option<(char, usize)>) -> Option<(char, usize, bool)> {
    let marker = line.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }
    let length = line
        .chars()
        .take_while(|character| *character == marker)
        .count();
    if length < 3 {
        return None;
    }
    let remainder = &line[length..];
    let closing = open.is_some_and(|(open_marker, open_length)| {
        marker == open_marker && length >= open_length && remainder.trim().is_empty()
    });
    Some((marker, length, closing))
}

/// Parse acceptance output text and determine pass/fail/continue/gated status.
///
/// Contract (JSON-primary, text-fallback):
///
/// - Primary: a strict JSON verdict object
///   `{"acceptance":"pass|fail|continue|gated","findings":[...]}` emitted
///   either directly as a line, or wrapped inside a supported JSONL event text
///   payload unwrapped by the runtime.
///   The first JSON verdict encountered wins, regardless of any earlier text
///   marker.
/// - Fallback: a standalone legacy line equal to one of `ACCEPTANCE: PASS`,
///   `ACCEPTANCE: FAIL`, `ACCEPTANCE: CONTINUE`, `ACCEPTANCE: GATED`,
///   or (legacy-compatibility) `ACCEPTANCE: BLOCKED`.
///   Bold/italic/underline wrappers and leading heading/blockquote/bullet
///   prefixes are tolerated defensively, but trailing-text concatenation onto
///   the marker (for example `ACCEPTANCE: PASSAll ...` or
///   `ACCEPTANCE: PASS## ...`) is NOT a canonical marker. When no JSON verdict
///   is found, the first canonical fallback line wins.
///
/// If neither is observed, the result is [`AcceptanceResult::MissingVerdict`]:
/// the acceptance command completed without emitting a canonical verdict. This
/// is an explicit protocol failure and is intentionally distinct from an
/// agent-emitted canonical `CONTINUE`, so a premature agent exit (for example a
/// status-only "waiting for verification" narrative) cannot masquerade as an
/// intentional continuation request.
pub fn parse_acceptance_output(output: &str) -> AcceptanceResult {
    let mut fallback_kind: Option<&'static str> = None;
    let mut fallback_findings = Vec::new();
    let mut collecting_findings = false;
    let mut detector = VerdictStreamDetector::default();

    for raw_line in output.lines() {
        let unwrapped = crate::stream_json_textifier::extract_text_from_stream_json(raw_line)
            .unwrap_or_else(|| raw_line.to_string());
        for line in unwrapped.lines() {
            let candidate = line.trim();
            let fence = verdict_markdown_fence(candidate, detector.code_fence);
            let detected = detector.detect_unwrapped(candidate);
            if fence.is_some() || detector.code_fence.is_some() {
                continue;
            }

            if let Some(verdict) = parse_json_verdict(candidate) {
                return acceptance_result_from_json(verdict);
            }

            if fallback_kind.is_none() {
                if let Some(kind) = detected {
                    fallback_kind = Some(kind);
                    collecting_findings = false;
                }
                continue;
            }

            if fallback_kind == Some("fail") {
                if candidate == "FINDINGS:" {
                    collecting_findings = true;
                } else if collecting_findings {
                    if let Some(finding) = candidate.strip_prefix("- ") {
                        fallback_findings.push(finding.to_string());
                    } else if !candidate.is_empty() {
                        collecting_findings = false;
                    }
                }
            }
        }
    }

    match fallback_kind {
        Some("pass") => AcceptanceResult::Pass,
        Some("continue") => AcceptanceResult::Continue,
        // The plain-text fallback carries no structured payload, so a legacy
        // `ACCEPTANCE: GATED`/`ACCEPTANCE: BLOCKED` line is always bare.
        Some("gated") => AcceptanceResult::BareBlocker {
            rejection: BlockerRejection::Missing,
        },
        Some("fail") => AcceptanceResult::Fail {
            findings: legacy_findings(fallback_findings),
        },
        // No canonical verdict anywhere in the output: explicit protocol
        // failure, never an implicit CONTINUE.
        _ => AcceptanceResult::MissingVerdict,
    }
}

fn acceptance_result_from_json(verdict: JsonVerdict) -> AcceptanceResult {
    match verdict.kind {
        "pass" => AcceptanceResult::Pass,
        // A structured finding that fails validation is a protocol error. It is
        // never converted into a normalized path-only repair instruction.
        "fail" => match parse_finding_entries(&verdict.findings) {
            Ok(findings) => AcceptanceResult::Fail { findings },
            Err(rejection) => AcceptanceResult::MalformedFinding { rejection },
        },
        "continue" => AcceptanceResult::Continue,
        // Compatibility verdict: only a validated structured payload becomes a
        // stall. Everything else is a bounded protocol error.
        "gated" => match validate_acceptance_blocker(verdict.blocker.as_ref()) {
            Ok(blocker) => AcceptanceResult::Stalled { blocker },
            Err(rejection) => AcceptanceResult::BareBlocker { rejection },
        },
        _ => AcceptanceResult::MissingVerdict,
    }
}

/// Strip markdown decorations from a string.
/// Removes bold (**), italic (*), underline (_), heading prefixes (#),
/// blockquote prefixes (>), and bullet prefixes (-) that LLMs commonly
/// add around verdict markers.
///
/// This is a defensive measure — the canonical contract forbids these
/// wrappers, but the parser tolerates them to prevent a missing-verdict
/// protocol failure when an agent drifts.
pub(crate) fn strip_markdown_decorations(text: &str) -> String {
    let mut s = text.to_string();
    // Remove bold pairs first to avoid leaving stray * characters
    s = s.replace("**", "");
    // Remove remaining italic/underline characters
    s = s.replace(['*', '_'], "");
    // Trim whitespace, then strip leading markdown block-level prefixes
    let trimmed = s.trim();
    // Strip leading heading markers (e.g., "## ", "### ")
    let trimmed = trimmed.trim_start_matches('#');
    // Strip leading blockquote markers (e.g., "> ")
    let trimmed = trimmed.trim_start_matches('>');
    // Strip leading bullet markers (e.g., "- ")
    let trimmed = trimmed.trim_start_matches('-');
    trimmed.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pass() {
        let output = "ACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn acceptance_append_prompt_text_is_not_parsed_as_output_verdict() {
        let prompt = crate::agent::append_optional_prompt(
            "generated acceptance prompt".to_string(),
            Some("optional guidance mentioning ACCEPTANCE: FAIL and {change_id}"),
        );
        assert!(prompt.ends_with("optional guidance mentioning ACCEPTANCE: FAIL and {change_id}"));
        assert_eq!(
            parse_acceptance_output("ACCEPTANCE: PASS\n"),
            AcceptanceResult::Pass
        );
    }

    #[test]
    fn test_parse_pass_with_extra_output() {
        let output = "Some debug output\nACCEPTANCE: PASS\nMore output\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_fail_with_findings() {
        let output = "ACCEPTANCE: FAIL\nFINDINGS:\n- Issue 1\n- Issue 2\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 2);
                assert_eq!(findings[0], "Issue 1");
                assert_eq!(findings[1], "Issue 2");
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_fail_with_no_findings() {
        let output = "ACCEPTANCE: FAIL\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 0);
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_fail_with_multiline_findings() {
        let output = r#"ACCEPTANCE: FAIL
FINDINGS:
- Task 1.3 is not completed
- Missing unit tests for new feature
- Code does not handle error case X
"#;
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 3);
                assert_eq!(findings[0], "Task 1.3 is not completed");
                assert_eq!(findings[1], "Missing unit tests for new feature");
                assert_eq!(findings[2], "Code does not handle error case X");
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_no_status() {
        let output = "Some random output\n";
        // When no explicit marker is present, this is a missing-verdict
        // protocol failure — not an implicit CONTINUE.
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict
        );
    }

    #[test]
    fn test_parse_no_marker_is_missing_verdict() {
        // Empty output is a missing-verdict protocol failure
        assert_eq!(
            parse_acceptance_output(""),
            AcceptanceResult::MissingVerdict
        );

        // Output with no acceptance marker is a missing-verdict protocol failure
        let output = "Some debug output\nNo marker here\n";
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict
        );

        // Output with findings but no marker is a missing-verdict protocol failure
        let output = "FINDINGS:\n- Issue 1\n- Issue 2\n";
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict
        );
    }

    #[test]
    fn test_parse_status_only_waiting_output_is_missing_verdict_not_continue() {
        // Regression: an acceptance agent that starts verification, reports it
        // is waiting for a completion notification, and exits without a
        // canonical verdict must be classified as a missing-verdict protocol
        // failure — never as an intentional CONTINUE.
        let output = "Started long-running verification job.\n\
                      Monitoring verification; I will evaluate the evidence and \
                      emit the final verdict once the completion notification arrives.\n";
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict
        );
        assert_ne!(
            parse_acceptance_output(output),
            AcceptanceResult::Continue,
            "status-only exit must remain distinguishable from explicit CONTINUE"
        );
    }

    /// Parser routing matrix: each canonical verdict keeps its own meaning and
    /// only verdict-free output becomes the missing-verdict protocol failure.
    /// The parser never sees command failure — a non-zero exit is classified by
    /// the runtime before parsing — so nothing here can turn into a protocol
    /// retry by accident.
    #[test]
    fn acceptance_parser_routing_matrix_is_stable() {
        let cases: [(&str, AcceptanceResult); 10] = [
            ("{\"acceptance\":\"pass\"}\n", AcceptanceResult::Pass),
            ("ACCEPTANCE: PASS\n", AcceptanceResult::Pass),
            (
                "{\"acceptance\":\"fail\",\"findings\":[\"src/a.rs:1 issue\"]}\n",
                AcceptanceResult::Fail {
                    findings: vec!["src/a.rs:1 issue".to_string().into()],
                },
            ),
            (
                "ACCEPTANCE: FAIL\nFINDINGS:\n- src/a.rs:1 issue\n",
                AcceptanceResult::Fail {
                    findings: vec!["src/a.rs:1 issue".to_string().into()],
                },
            ),
            (
                "{\"acceptance\":\"continue\"}\n",
                AcceptanceResult::Continue,
            ),
            ("ACCEPTANCE: CONTINUE\n", AcceptanceResult::Continue),
            (
                "{\"acceptance\":\"gated\"}\n",
                AcceptanceResult::BareBlocker {
                    rejection: BlockerRejection::Missing,
                },
            ),
            (
                "ACCEPTANCE: BLOCKED\n",
                AcceptanceResult::BareBlocker {
                    rejection: BlockerRejection::Missing,
                },
            ),
            (
                "Monitoring verification; waiting for the job to finish\n",
                AcceptanceResult::MissingVerdict,
            ),
            (
                "ACCEPTANCE: PASSAll criteria verified\n",
                AcceptanceResult::MissingVerdict,
            ),
        ];

        for (output, expected) in cases {
            assert_eq!(
                parse_acceptance_output(output),
                expected,
                "routing drifted for output: {output:?}"
            );
        }
    }

    #[test]
    fn test_explicit_continue_remains_distinct_from_missing_verdict() {
        // Canonical CONTINUE — via JSON and via legacy marker — must keep its
        // intentional-continuation semantics.
        assert_eq!(
            parse_acceptance_output("{\"acceptance\":\"continue\"}\n"),
            AcceptanceResult::Continue
        );
        assert_eq!(
            parse_acceptance_output("ACCEPTANCE: CONTINUE\n"),
            AcceptanceResult::Continue
        );
        // Whereas verdict-free output is a protocol failure.
        assert_eq!(
            parse_acceptance_output("waiting for checks to finish\n"),
            AcceptanceResult::MissingVerdict
        );
    }

    #[test]
    fn test_parse_findings_with_trailing_content() {
        let output = r#"ACCEPTANCE: FAIL
FINDINGS:
- Issue 1
- Issue 2

Additional output here
"#;
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 2);
                assert_eq!(findings[0], "Issue 1");
                assert_eq!(findings[1], "Issue 2");
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_continue() {
        let output = "ACCEPTANCE: CONTINUE\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_continue_with_extra_output() {
        let output = "Some debug output\nACCEPTANCE: CONTINUE\nMore output\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_pass_with_bold_decoration() {
        let output = "**ACCEPTANCE: PASS**\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_pass_with_bold_decoration_and_extra_output() {
        let output = "Some debug output\n**ACCEPTANCE: PASS**\nMore output\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_fail_with_bold_decoration() {
        let output = "**ACCEPTANCE: FAIL**\nFINDINGS:\n- Issue 1\n- Issue 2\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 2);
                assert_eq!(findings[0], "Issue 1");
                assert_eq!(findings[1], "Issue 2");
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_continue_with_bold_decoration() {
        let output = "**ACCEPTANCE: CONTINUE**\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_pass_with_italic_decoration() {
        let output = "*ACCEPTANCE: PASS*\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_pass_with_mixed_decorations() {
        let output = "**_ACCEPTANCE: PASS_**\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_strip_markdown_decorations() {
        assert_eq!(
            strip_markdown_decorations("**ACCEPTANCE: PASS**"),
            "ACCEPTANCE: PASS"
        );
        assert_eq!(
            strip_markdown_decorations("*ACCEPTANCE: PASS*"),
            "ACCEPTANCE: PASS"
        );
        assert_eq!(
            strip_markdown_decorations("_ACCEPTANCE: PASS_"),
            "ACCEPTANCE: PASS"
        );
        assert_eq!(
            strip_markdown_decorations("**_ACCEPTANCE: PASS_**"),
            "ACCEPTANCE: PASS"
        );
        assert_eq!(
            strip_markdown_decorations("ACCEPTANCE: PASS"),
            "ACCEPTANCE: PASS"
        );
    }

    #[test]
    fn test_parse_ignores_acceptance_in_code_blocks() {
        // Code block with FAIL should be ignored, actual PASS should be detected
        let output = r#"
Example output:
```
ACCEPTANCE: FAIL
FINDINGS:
- Issue 1
```

Actual result:
ACCEPTANCE: PASS
"#;
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_ignores_multiple_code_blocks() {
        // Multiple code blocks with different statuses should be ignored
        let output = r#"
First example:
```
ACCEPTANCE: FAIL
```

Second example:
```
ACCEPTANCE: CONTINUE
```

Actual result:
ACCEPTANCE: PASS
"#;
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_code_block_with_language_specifier() {
        // Code blocks with language specifier should also be ignored
        let output = r#"
Example:
```bash
ACCEPTANCE: FAIL
```

Result:
ACCEPTANCE: PASS
"#;
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_ignores_tilde_fenced_verdicts() {
        let output =
            "~~~json\n{\"acceptance\":\"pass\"}\n~~~\nACCEPTANCE: FAIL\nFINDINGS:\n- real failure";

        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::Fail {
                findings: vec!["real failure".to_string().into()]
            }
        );
    }

    #[test]
    fn test_parse_does_not_close_fence_with_info_string() {
        let output = "```text\n```json\n{\"acceptance\":\"pass\"}\nACCEPTANCE: FAIL";

        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict
        );
    }

    #[test]
    fn verdict_stream_detector_ignores_fenced_verdicts() {
        let mut detector = VerdictStreamDetector::default();

        assert_eq!(detector.detect("~~~json"), None);
        assert_eq!(detector.detect(r#"{\"acceptance\":\"pass\"}"#), None);
        assert_eq!(detector.detect("~~~"), None);
        assert_eq!(detector.detect("ACCEPTANCE: FAIL"), Some("fail"));

        let mut detector = VerdictStreamDetector::default();
        assert_eq!(detector.detect("```json"), None);
        assert_eq!(detector.detect(r#"{"acceptance":"pass"}"#), None);
        assert_eq!(detector.detect("```"), None);
    }

    #[test]
    fn test_parse_unclosed_code_block() {
        // If code block is not closed, everything after opening should be skipped
        let output = r#"
Example:
```
ACCEPTANCE: FAIL
ACCEPTANCE: PASS
"#;
        // Both are inside unclosed code block, so no canonical verdict exists.
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict
        );
    }

    // Trailing-text concatenation is NOT canonical. The runtime parser
    // rejects malformed verdicts and falls through to the missing-verdict
    // protocol failure instead of locking in a bogus PASS.

    #[test]
    fn test_parse_pass_with_trailing_text_is_not_canonical() {
        // Regression: real log produced "ACCEPTANCE: PASSAll acceptance criteria verified:"
        // which previously satisfied PASS via starts_with. Strict canonical
        // matching rejects it.
        let output = "ACCEPTANCE: PASSAll acceptance criteria verified:\n";
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict
        );
    }

    #[test]
    fn test_parse_pass_with_trailing_heading_is_not_canonical() {
        // Real log: "ACCEPTANCE: PASS## Acceptance Review Summary"
        let output = "ACCEPTANCE: PASS## Acceptance Review Summary\n";
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict
        );
    }

    #[test]
    fn test_parse_pass_with_trailing_text_in_context_is_not_canonical() {
        let output = r#"Some prior output
ACCEPTANCE: PASSAll acceptance criteria verified:

1. Git working tree is clean
"#;
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict
        );
    }

    #[test]
    fn test_parse_pass_with_trailing_text_falls_through_to_canonical_pass() {
        // If a malformed verdict appears first but a canonical standalone
        // verdict follows on a later line, the canonical line wins.
        let output = "ACCEPTANCE: PASSAll bad form\nACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_fail_with_trailing_text_is_not_canonical() {
        let output = "ACCEPTANCE: FAILSome additional context\nFINDINGS:\n- Issue 1\n";
        // Trailing-text FAIL is malformed; falls through to MissingVerdict.
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict
        );
    }

    #[test]
    fn test_parse_continue_with_trailing_text_is_not_canonical() {
        let output = "ACCEPTANCE: CONTINUENeeds further investigation\n";
        // Malformed verdict — falls through to the missing-verdict protocol
        // failure rather than being read as an intentional CONTINUE.
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict
        );
    }

    #[test]
    fn test_parse_blocked_with_trailing_text_is_not_canonical() {
        let output = "ACCEPTANCE: BLOCKEDWaiting for dependency\n";
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict
        );
    }

    #[test]
    fn test_parse_passed_word_boundary_is_not_canonical() {
        // "ACCEPTANCE: PASSED" must not match canonical PASS.
        let output = "ACCEPTANCE: PASSED\n";
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict
        );
    }

    #[test]
    fn test_canonical_verdict_kind_strict_match() {
        assert_eq!(canonical_verdict_kind("ACCEPTANCE: PASS"), Some("pass"));
        assert_eq!(canonical_verdict_kind("**ACCEPTANCE: PASS**"), Some("pass"));
        assert_eq!(canonical_verdict_kind("## ACCEPTANCE: PASS"), Some("pass"));
        assert_eq!(canonical_verdict_kind("> ACCEPTANCE: FAIL"), Some("fail"));
        assert_eq!(canonical_verdict_kind("ACCEPTANCE: PASSAll bad"), None);
        assert_eq!(canonical_verdict_kind("ACCEPTANCE: PASS## heading"), None);
        assert_eq!(canonical_verdict_kind("ACCEPTANCE: PASSED"), None);
        assert_eq!(canonical_verdict_kind("not a verdict"), None);
    }

    #[test]
    fn test_parse_blocked() {
        let output = "ACCEPTANCE: BLOCKED\n";
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::BareBlocker {
                rejection: BlockerRejection::Missing
            }
        );
    }

    #[test]
    fn test_parse_blocked_with_extra_output() {
        let output = "Some debug output\nACCEPTANCE: BLOCKED\nMore output\n";
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::BareBlocker {
                rejection: BlockerRejection::Missing
            }
        );
    }

    #[test]
    fn test_parse_blocked_with_bold_decoration() {
        let output = "**ACCEPTANCE: BLOCKED**\n";
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::BareBlocker {
                rejection: BlockerRejection::Missing
            }
        );
    }

    // Characterization tests: document the exact contract that
    // src/orchestration/acceptance.rs relies on after the refactor.

    #[test]
    fn test_parse_fail_findings_excludes_preamble() {
        // parse_acceptance_output for FAIL extracts only items from the
        // FINDINGS section — preamble lines before the marker are NOT included.
        let output =
            "preamble line\nACCEPTANCE: FAIL\nFINDINGS:\n- Finding 1\n- Finding 2\npostamble";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings, vec!["Finding 1", "Finding 2"]);
                assert!(!findings.iter().any(|f| f.contains("preamble")));
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_fail_findings_from_findings_section_only() {
        // Findings come exclusively from lines prefixed with "- " after
        // "FINDINGS:". This is the single authoritative source used in
        // AcceptanceResult::Fail after the refactor.
        let output =
            "ACCEPTANCE: FAIL\nFINDINGS:\n- src/foo.rs:10 missing test\n- src/bar.rs:5 dead code\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 2);
                assert_eq!(findings[0], "src/foo.rs:10 missing test");
                assert_eq!(findings[1], "src/bar.rs:5 dead code");
            }
            _ => panic!("Expected Fail"),
        }
    }

    // --- Markdown drift tolerance tests ---
    // These tests verify that the parser defensively handles common LLM
    // formatting drift (headings, blockquotes, bullets) even though the
    // canonical contract forbids these wrappers.

    #[test]
    fn test_parse_pass_with_heading_prefix() {
        let output = "## ACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_pass_with_heading_h3_prefix() {
        let output = "### ACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_fail_with_heading_prefix() {
        let output = "## ACCEPTANCE: FAIL\nFINDINGS:\n- Issue 1\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 1);
                assert_eq!(findings[0], "Issue 1");
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_pass_with_blockquote_prefix() {
        let output = "> ACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_fail_with_blockquote_prefix() {
        let output = "> ACCEPTANCE: FAIL\nFINDINGS:\n- Issue 1\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 1);
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_pass_with_bullet_prefix() {
        let output = "- ACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_continue_with_heading_prefix() {
        let output = "## ACCEPTANCE: CONTINUE\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Continue);
    }

    #[test]
    fn test_parse_blocked_with_heading_prefix() {
        let output = "## ACCEPTANCE: BLOCKED\n";
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::BareBlocker {
                rejection: BlockerRejection::Missing
            }
        );
    }

    #[test]
    fn test_parse_pass_with_heading_and_bold() {
        // Combined drift: heading + bold
        let output = "## **ACCEPTANCE: PASS**\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_strip_markdown_decorations_heading() {
        assert_eq!(
            strip_markdown_decorations("## ACCEPTANCE: PASS"),
            "ACCEPTANCE: PASS"
        );
    }

    #[test]
    fn test_strip_markdown_decorations_blockquote() {
        assert_eq!(
            strip_markdown_decorations("> ACCEPTANCE: PASS"),
            "ACCEPTANCE: PASS"
        );
    }

    #[test]
    fn test_strip_markdown_decorations_bullet() {
        assert_eq!(
            strip_markdown_decorations("- ACCEPTANCE: PASS"),
            "ACCEPTANCE: PASS"
        );
    }

    #[test]
    fn test_strip_markdown_decorations_heading_and_bold() {
        assert_eq!(
            strip_markdown_decorations("## **ACCEPTANCE: PASS**"),
            "ACCEPTANCE: PASS"
        );
    }

    #[test]
    fn test_parse_fail_empty_findings_when_no_section() {
        // When ACCEPTANCE: FAIL appears without a FINDINGS section,
        // parse_acceptance_output returns an empty findings vec.
        let output = "ACCEPTANCE: FAIL\nSome explanation without a FINDINGS: header\n";
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert!(findings.is_empty());
            }
            _ => panic!("Expected Fail"),
        }
    }

    // --- Marker contract consistency tests ---
    // These tests verify that the marker detection used in orchestration/acceptance.rs
    // streaming code is consistent with the parser contract. The streaming code uses
    // strip_markdown_decorations + starts_with to detect markers for the grace period,
    // and the parser uses the same function. Both must agree.

    #[test]
    fn test_marker_detection_consistency_with_parser() {
        // Streaming verdict detection (src/orchestration/acceptance.rs and
        // src/parallel/executor.rs) and the parser MUST agree on the canonical
        // contract: standalone exact match after stripping tolerated markdown
        // decorations. This test pins both surfaces to canonical_verdict_kind.
        let drift_cases: &[(&str, &str)] = &[
            ("ACCEPTANCE: PASS", "pass"),
            ("**ACCEPTANCE: PASS**", "pass"),
            ("## ACCEPTANCE: PASS", "pass"),
            ("> ACCEPTANCE: PASS", "pass"),
            ("- ACCEPTANCE: PASS", "pass"),
            ("### **ACCEPTANCE: FAIL**", "fail"),
            ("ACCEPTANCE: CONTINUE", "continue"),
            ("ACCEPTANCE: GATED", "gated"),
            // Legacy compatibility during migration.
            ("ACCEPTANCE: BLOCKED", "gated"),
            ("## ACCEPTANCE: BLOCKED", "gated"),
            ("> ACCEPTANCE: FAIL", "fail"),
        ];

        for (case, expected_kind) in drift_cases {
            assert_eq!(
                canonical_verdict_kind(case),
                Some(*expected_kind),
                "canonical_verdict_kind must detect '{}' as kind '{}'",
                case,
                expected_kind
            );

            let full_output = format!("{}\n", case);
            let result = parse_acceptance_output(&full_output);
            let result_kind = match &result {
                AcceptanceResult::Pass => "pass",
                AcceptanceResult::Fail { .. } => "fail",
                AcceptanceResult::Continue => "continue",
                AcceptanceResult::BareBlocker { .. } => "gated",
                AcceptanceResult::Stalled { .. } => "stalled",
                AcceptanceResult::MalformedFinding { .. } => "malformed-finding",
                AcceptanceResult::MissingVerdict => "missing-verdict",
            };
            assert_eq!(
                result_kind, *expected_kind,
                "parse_acceptance_output returned '{}' but expected '{}' for input '{}'",
                result_kind, expected_kind, case
            );
        }
    }

    #[test]
    fn test_marker_detection_rejects_trailing_text_uniformly() {
        // Both surfaces (parser + streaming detection) must reject trailing-text
        // verdicts so they cannot accidentally finalize an acceptance run early.
        let malformed: &[&str] = &[
            "ACCEPTANCE: PASSAll checks completed",
            "ACCEPTANCE: PASS## Acceptance Review Summary",
            "ACCEPTANCE: PASSED",
            "ACCEPTANCE: FAILSome explanation",
            "ACCEPTANCE: CONTINUEMore work",
            "ACCEPTANCE: BLOCKEDWaiting",
        ];

        for case in malformed {
            assert!(
                canonical_verdict_kind(case).is_none(),
                "canonical_verdict_kind must NOT detect malformed verdict '{}'",
                case
            );
        }
    }

    #[test]
    fn test_code_fence_markers_rejected_by_both_parser_and_detection() {
        // Markers inside code fences must NOT be detected by either the parser
        // or the streaming marker detection (which operates line-by-line and
        // does not track code fence state, but the parser does).
        let output = "```\nACCEPTANCE: PASS\n```\n";
        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::MissingVerdict,
            "Parser must not match markers inside code fences"
        );
    }

    // --- JSON-primary verdict contract tests ---

    fn verdict_kind(line: &str) -> Option<&'static str> {
        parse_json_verdict(line).map(|verdict| verdict.kind)
    }

    #[test]
    fn test_parse_json_verdict_pass() {
        let verdict = parse_json_verdict(r#"{"acceptance":"pass"}"#).expect("strict JSON verdict");
        assert_eq!(verdict.kind, "pass");
        assert!(verdict.findings.is_empty());
        assert_eq!(verdict.blocker, None);
    }

    #[test]
    fn test_parse_json_verdict_fail_with_findings() {
        let line = r#"{"acceptance":"fail","findings":["src/a.rs:1 bad","src/b.rs:2 worse"]}"#;
        let verdict = parse_json_verdict(line).expect("strict JSON verdict");
        assert_eq!(verdict.kind, "fail");
        assert_eq!(verdict.findings, vec!["src/a.rs:1 bad", "src/b.rs:2 worse"]);
    }

    #[test]
    fn test_parse_json_verdict_continue_gated_and_legacy_blocked() {
        assert_eq!(
            verdict_kind(r#"{"acceptance":"continue"}"#),
            Some("continue")
        );
        assert_eq!(verdict_kind(r#"{"acceptance":"gated"}"#), Some("gated"));
        assert_eq!(verdict_kind(r#"{"acceptance":"blocked"}"#), Some("gated"));
    }

    #[test]
    fn test_parse_json_verdict_case_insensitive_value() {
        assert_eq!(verdict_kind(r#"{"acceptance":"PASS"}"#), Some("pass"));
        assert_eq!(verdict_kind(r#"{"acceptance":"Fail"}"#), Some("fail"));
    }

    #[test]
    fn test_parse_json_verdict_rejects_non_object_and_unknown_kind() {
        assert_eq!(verdict_kind("pass"), None);
        assert_eq!(verdict_kind(r#"["pass"]"#), None);
        assert_eq!(verdict_kind(r#"{"acceptance":"maybe"}"#), None);
        assert_eq!(verdict_kind(r#"{"other":"pass"}"#), None);
        assert_eq!(verdict_kind("not json"), None);
        assert_eq!(verdict_kind(""), None);
    }

    #[test]
    fn test_parse_acceptance_output_json_pass_single_line() {
        // Primary contract: the final machine-readable verdict is a strict JSON
        // object emitted as its own stdout line.
        let output = r#"{"acceptance":"pass"}
"#;
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_acceptance_output_json_fail_findings_preferred_over_text_section() {
        // JSON findings array is the source of truth when JSON verdict wins.
        let output = r#"preamble
{"acceptance":"fail","findings":["x","y"]}
"#;
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings, vec!["x", "y"]);
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_acceptance_output_json_beats_text_fallback_regardless_of_order() {
        // JSON-primary: when both a text marker and a JSON verdict appear, the
        // JSON verdict wins even if the text marker came first.
        let output = "ACCEPTANCE: CONTINUE\n{\"acceptance\":\"pass\"}\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_acceptance_output_text_fallback_when_no_json() {
        // Backward compatibility: absent a JSON verdict, the legacy standalone
        // text marker remains accepted as canonical.
        let output = "ACCEPTANCE: PASS\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_acceptance_output_json_inside_agent_assistant_event() {
        // Some agent JSONL formats wrap final text in an assistant event.
        // The parser must unwrap the text payload and still find the JSON verdict.
        let event = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"{\"acceptance\":\"pass\"}"}]}}"#;
        let output = format!("{}\n", event);
        assert_eq!(parse_acceptance_output(&output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_parse_acceptance_output_ignores_fenced_verdict_inside_agent_event() {
        let event = serde_json::json!({
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{
                    "type": "text",
                    "text": "~~~json\n{\"acceptance\":\"pass\"}\n~~~\nACCEPTANCE: FAIL\nFINDINGS:\n- real"
                }]
            }
        })
        .to_string();

        assert_eq!(
            parse_acceptance_output(&event),
            AcceptanceResult::Fail {
                findings: vec!["real".to_string().into()]
            }
        );
    }

    #[test]
    fn legacy_fail_ignores_findings_before_verdict_and_inside_fences() {
        let output = "FINDINGS:\n- stale\n```text\nFINDINGS:\n- injected\n```\nACCEPTANCE: FAIL\nFINDINGS:\n- canonical";

        assert_eq!(
            parse_acceptance_output(output),
            AcceptanceResult::Fail {
                findings: vec!["canonical".to_string().into()]
            }
        );
    }

    #[test]
    fn test_parse_acceptance_output_json_inside_agent_result_event() {
        // Some agent JSONL formats carry the assistant's final text in `result`.
        // Parser must unwrap and accept the JSON verdict.
        let event = r#"{"type":"result","subtype":"success","result":"{\"acceptance\":\"fail\",\"findings\":[\"a\"]}","is_error":false}"#;
        let output = format!("{}\n", event);
        match parse_acceptance_output(&output) {
            AcceptanceResult::Fail { findings } => {
                assert_eq!(findings, vec!["a".to_string()]);
            }
            other => panic!("expected Fail, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_acceptance_output_json_inside_codex_item_completed_event() {
        let event = r#"{"type":"item.completed","item":{"id":"item_0","type":"agent_message","text":"{\"acceptance\":\"pass\"}"}}"#;
        let output = format!("{}\n", event);
        assert_eq!(parse_acceptance_output(&output), AcceptanceResult::Pass);
    }

    #[test]
    fn test_detect_verdict_in_line_json_direct() {
        assert_eq!(
            detect_verdict_in_line(r#"{"acceptance":"pass"}"#),
            Some("pass")
        );
        assert_eq!(
            detect_verdict_in_line(r#"{"acceptance":"gated"}"#),
            Some("gated")
        );
        assert_eq!(
            detect_verdict_in_line(r#"{"acceptance":"blocked"}"#),
            Some("gated")
        );
    }

    #[test]
    fn test_detect_verdict_in_line_json_inside_assistant_event() {
        let event = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"{\"acceptance\":\"pass\"}"}]}}"#;
        assert_eq!(detect_verdict_in_line(event), Some("pass"));
    }

    #[test]
    fn test_detect_verdict_in_line_text_fallback() {
        assert_eq!(detect_verdict_in_line("ACCEPTANCE: PASS"), Some("pass"));
        assert_eq!(detect_verdict_in_line("**ACCEPTANCE: FAIL**"), Some("fail"));
        assert_eq!(
            detect_verdict_in_line("ACCEPTANCE: PASSAll checks done"),
            None,
            "trailing-text PASS must not satisfy even the text fallback"
        );
    }

    #[test]
    fn test_detect_verdict_in_line_text_inside_assistant_event() {
        // When the agent produces the legacy text marker but wrapped in a
        // stream-json assistant event, the detector MUST still recognize the
        // fallback so acceptance does not stall waiting for JSON.
        let event = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"ACCEPTANCE: PASS"}]}}"#;
        assert_eq!(detect_verdict_in_line(event), Some("pass"));
    }

    #[test]
    fn test_detect_verdict_in_line_unrelated_events_return_none() {
        assert_eq!(
            detect_verdict_in_line(r#"{"type":"system","subtype":"init"}"#),
            None
        );
        assert_eq!(detect_verdict_in_line("plain log line"), None);
        assert_eq!(detect_verdict_in_line(""), None);
    }

    // --- Structured stalled-blocker contract tests ---

    fn structured_gated(blocker: &str) -> String {
        format!("{{\"acceptance\":\"gated\",\"blocker\":{blocker}}}\n")
    }

    const VALID_BLOCKER: &str = r#"{"category":"credential","evidence":["STAGING_API_KEY unset in the verification environment"],"next_action":"provision STAGING_API_KEY then retry acceptance","resumable":true}"#;

    #[test]
    fn structured_blocker_becomes_validated_stall_with_explicit_category() {
        match parse_acceptance_output(&structured_gated(VALID_BLOCKER)) {
            AcceptanceResult::Stalled { blocker } => {
                assert_eq!(blocker.category, "credential");
                assert_eq!(
                    blocker.evidence,
                    ["STAGING_API_KEY unset in the verification environment"]
                );
                assert_eq!(
                    blocker.next_action,
                    "provision STAGING_API_KEY then retry acceptance"
                );
                assert!(blocker.resumable);
                assert_eq!(blocker.prerequisite_owner, None);
                assert!(blocker.evidence_ids.is_empty());
            }
            other => panic!("expected validated stall, got {other:?}"),
        }
    }

    #[test]
    fn structured_blocker_preserves_optional_owner_and_evidence_ids() {
        let blocker = r#"{"category":"external_approval","evidence":["change board ticket CB-42 is awaiting sign-off"],"next_action":"await CB-42 approval then retry acceptance","resumable":true,"prerequisite_owner":"release-management","evidence_ids":["CB-42",""," "]}"#;
        match parse_acceptance_output(&structured_gated(blocker)) {
            AcceptanceResult::Stalled { blocker } => {
                assert_eq!(blocker.category, "external_approval");
                assert_eq!(
                    blocker.prerequisite_owner.as_deref(),
                    Some("release-management")
                );
                assert_eq!(blocker.evidence_ids, ["CB-42"]);
            }
            other => panic!("expected validated stall, got {other:?}"),
        }
    }

    #[test]
    fn every_supported_category_is_accepted_verbatim() {
        for category in SUPPORTED_BLOCKER_CATEGORIES {
            let blocker = format!(
                r#"{{"category":"{category}","evidence":["concrete evidence"],"next_action":"resolve then retry","resumable":false}}"#
            );
            match parse_acceptance_output(&structured_gated(&blocker)) {
                AcceptanceResult::Stalled { blocker } => {
                    assert_eq!(&blocker.category, category);
                    assert!(!blocker.resumable);
                }
                other => panic!("category {category} must validate, got {other:?}"),
            }
        }
    }

    /// Table-driven rejection matrix: every incomplete or malformed blocker
    /// payload falls back to the bounded protocol-error path, never a stall.
    #[test]
    fn invalid_blocker_payloads_fall_back_to_bare_protocol_error() {
        let cases: &[(&str, BlockerRejection)] = &[
            (r#"null"#, BlockerRejection::Missing),
            (r#""credential""#, BlockerRejection::NotAnObject),
            (r#"["credential"]"#, BlockerRejection::NotAnObject),
            (
                r#"{"evidence":["e"],"next_action":"a","resumable":true}"#,
                BlockerRejection::MissingCategory,
            ),
            (
                r#"{"category":"   ","evidence":["e"],"next_action":"a","resumable":true}"#,
                BlockerRejection::MissingCategory,
            ),
            (
                r#"{"category":"flaky_test","evidence":["e"],"next_action":"a","resumable":true}"#,
                BlockerRejection::UnsupportedCategory("flaky_test".to_string()),
            ),
            (
                r#"{"category":"credential","evidence":[],"next_action":"a","resumable":true}"#,
                BlockerRejection::EmptyEvidence,
            ),
            (
                r#"{"category":"credential","evidence":["  "],"next_action":"a","resumable":true}"#,
                BlockerRejection::EmptyEvidence,
            ),
            (
                r#"{"category":"credential","next_action":"a","resumable":true}"#,
                BlockerRejection::EmptyEvidence,
            ),
            (
                r#"{"category":"credential","evidence":["e"],"resumable":true}"#,
                BlockerRejection::MissingNextAction,
            ),
            (
                r#"{"category":"credential","evidence":["e"],"next_action":"  ","resumable":true}"#,
                BlockerRejection::MissingNextAction,
            ),
            (
                r#"{"category":"credential","evidence":["e"],"next_action":"a"}"#,
                BlockerRejection::MissingResumable,
            ),
            (
                r#"{"category":"credential","evidence":["e"],"next_action":"a","resumable":"yes"}"#,
                BlockerRejection::MissingResumable,
            ),
        ];

        for (payload, expected) in cases {
            assert_eq!(
                parse_acceptance_output(&structured_gated(payload)),
                AcceptanceResult::BareBlocker {
                    rejection: expected.clone()
                },
                "payload must not become a stall: {payload}"
            );
        }
    }

    /// Prose containing credential/token/auth words must never be promoted to a
    /// category. Only an explicit supported `category` field can do that.
    #[test]
    fn credential_prose_never_infers_a_blocker_category() {
        let outputs = [
            "{\"acceptance\":\"gated\",\"findings\":[\"missing credential token for auth\"]}\n"
                .to_string(),
            structured_gated(
                r#"{"evidence":["missing credential token for auth"],"next_action":"provision it","resumable":true}"#,
            ),
            "ACCEPTANCE: GATED\nThe deploy credential token could not be read (auth failure)\n"
                .to_string(),
        ];
        for output in outputs {
            match parse_acceptance_output(&output) {
                AcceptanceResult::BareBlocker { .. } => {}
                other => panic!("prose must not create a category, got {other:?}"),
            }
        }
    }

    /// Bare and legacy compatibility inputs share one protocol-error path.
    #[test]
    fn bare_and_legacy_blocker_inputs_share_the_protocol_error_path() {
        let bare = AcceptanceResult::BareBlocker {
            rejection: BlockerRejection::Missing,
        };
        for output in [
            "{\"acceptance\":\"gated\"}\n",
            "{\"acceptance\":\"blocked\"}\n",
            "ACCEPTANCE: GATED\n",
            "ACCEPTANCE: BLOCKED\n",
            "**ACCEPTANCE: GATED**\n",
        ] {
            assert_eq!(
                parse_acceptance_output(output),
                bare,
                "drifted for {output:?}"
            );
        }
    }

    #[test]
    fn blocker_rejection_reasons_are_operator_readable() {
        assert!(BlockerRejection::Missing
            .reason()
            .contains("no structured blocker"));
        assert!(BlockerRejection::UnsupportedCategory("nope".to_string())
            .reason()
            .contains("credential"));
        assert!(BlockerRejection::EmptyEvidence
            .reason()
            .contains("evidence"));
        assert!(BlockerRejection::MissingNextAction
            .reason()
            .contains("next_action"));
        assert!(BlockerRejection::MissingResumable
            .reason()
            .contains("resumable"));
        assert!(BlockerRejection::NotAnObject
            .reason()
            .contains("JSON object"));
        assert!(BlockerRejection::MissingCategory
            .reason()
            .contains("category"));
    }

    #[test]
    fn structured_blocker_survives_agent_event_wrapping() {
        let text = format!("{{\"acceptance\":\"gated\",\"blocker\":{VALID_BLOCKER}}}");
        let event = serde_json::json!({
            "type": "assistant",
            "message": {"role": "assistant", "content": [{"type": "text", "text": text}]}
        })
        .to_string();
        assert!(matches!(
            parse_acceptance_output(&event),
            AcceptanceResult::Stalled { .. }
        ));
    }

    /// Regression for the malformed trailing-text real-log case
    /// (`ACCEPTANCE: PASS# ...`): with the legacy text-only contract, the
    /// parser returned CONTINUE and the acceptance loop retried. Under the new
    /// JSON-primary contract the agent emits a strict JSON verdict on a later
    /// line, and that JSON verdict MUST win regardless of the earlier drift.
    #[test]
    fn test_regression_malformed_text_then_json_pass() {
        let output = "ACCEPTANCE: PASS# Acceptance Review Summary\n{\"acceptance\":\"pass\"}\n";
        assert_eq!(parse_acceptance_output(output), AcceptanceResult::Pass);
    }
    // --- Structured repository finding contract ---

    fn secret_value_finding_json() -> String {
        serde_json::json!({
            "acceptance": "fail",
            "findings": [{
                "id": "acceptance-secret-value-scan",
                "severity": "minor",
                "summary": "Challenge and proof leakage is not tested by value",
                "evidence": ["tests/support/relay.ts exposes counts but not issued values"],
                "required_changes": [{
                    "file": "tests/support/relay.ts",
                    "description": "Expose issued challenge and presented proof values to tests"
                }],
                "verification": [{
                    "file": "runtime/recovery.integration.test.ts",
                    "description": "Assert recorded values are absent from serialized audit output"
                }]
            }]
        })
        .to_string()
    }

    fn fail_findings(output: &str) -> Vec<AcceptanceFinding> {
        match parse_acceptance_output(output) {
            AcceptanceResult::Fail { findings } => findings,
            other => panic!("expected Fail, got {other:?}"),
        }
    }

    fn rejection(output: &str) -> FindingRejection {
        match parse_acceptance_output(output) {
            AcceptanceResult::MalformedFinding { rejection } => rejection,
            other => panic!("expected MalformedFinding, got {other:?}"),
        }
    }

    #[test]
    fn acceptance_structured_finding_reaches_callers_without_lossy_conversion() {
        let findings = fail_findings(&secret_value_finding_json());
        assert_eq!(findings.len(), 1);
        let structured = findings[0]
            .structured_payload()
            .expect("structured payload must survive parsing");

        assert_eq!(structured.id, "acceptance-secret-value-scan");
        assert_eq!(structured.severity, FindingSeverity::Minor);
        assert_eq!(
            structured.summary,
            "Challenge and proof leakage is not tested by value"
        );
        assert_eq!(
            structured.evidence,
            ["tests/support/relay.ts exposes counts but not issued values"]
        );
        assert_eq!(structured.required_files(), ["tests/support/relay.ts"]);
        assert_eq!(
            structured.verification_files(),
            ["runtime/recovery.integration.test.ts"]
        );

        // The single-line rendering keeps every actionable field, so no surface
        // that only carries text can silently reduce this to a path hint.
        let text = findings[0].text();
        assert!(text.contains("acceptance-secret-value-scan"), "{text}");
        assert!(
            text.contains("exposes counts but not issued values"),
            "{text}"
        );
        assert!(text.contains("tests/support/relay.ts"), "{text}");
        assert!(
            text.contains("runtime/recovery.integration.test.ts"),
            "{text}"
        );
    }

    #[test]
    fn acceptance_structured_finding_accepts_both_severities_and_both_block_pass() {
        for severity in ["major", "minor"] {
            let output = serde_json::json!({
                "acceptance": "fail",
                "findings": [{
                    "id": format!("id-{severity}"),
                    "severity": severity,
                    "summary": "s",
                    "evidence": ["e"],
                    "required_changes": [{"file": "src/a.rs", "description": "d"}],
                    "verification": [{"file": "tests/a.rs", "description": "d"}]
                }]
            })
            .to_string();
            let result = parse_acceptance_output(&output);
            assert!(
                matches!(result, AcceptanceResult::Fail { .. }),
                "{severity} must block PASS, got {result:?}"
            );
        }
    }

    #[test]
    fn acceptance_structured_finding_rejection_matrix_is_bounded_protocol_handling() {
        let base = serde_json::json!({
            "id": "stable-id",
            "severity": "major",
            "summary": "s",
            "evidence": ["e"],
            "required_changes": [{"file": "src/a.rs", "description": "d"}],
            "verification": [{"file": "tests/a.rs", "description": "d"}]
        });
        let mutate = |key: &str, value: serde_json::Value| {
            let mut object = base.as_object().unwrap().clone();
            if value.is_null() {
                object.remove(key);
            } else {
                object.insert(key.to_string(), value);
            }
            serde_json::json!({"acceptance": "fail", "findings": [object]}).to_string()
        };

        let cases: Vec<(String, FindingRejection)> = vec![
            (
                mutate("id", serde_json::Value::Null),
                FindingRejection::MissingId,
            ),
            (
                mutate("id", serde_json::json!("   ")),
                FindingRejection::MissingId,
            ),
            (
                mutate("severity", serde_json::json!("blocker")),
                FindingRejection::UnsupportedSeverity("blocker".to_string()),
            ),
            (
                mutate("summary", serde_json::Value::Null),
                FindingRejection::MissingSummary,
            ),
            (
                mutate("evidence", serde_json::json!([])),
                FindingRejection::EmptyEvidence,
            ),
            (
                mutate("required_changes", serde_json::Value::Null),
                FindingRejection::EmptyRequiredChanges,
            ),
            (
                mutate("verification", serde_json::json!([])),
                FindingRejection::EmptyVerification,
            ),
            (
                mutate(
                    "required_changes",
                    serde_json::json!([{"file": "../../etc/passwd", "description": "d"}]),
                ),
                FindingRejection::InvalidPath {
                    field: "required_changes",
                    path: "../../etc/passwd".to_string(),
                },
            ),
            (
                mutate(
                    "verification",
                    serde_json::json!([{"file": "/abs/path.rs", "description": "d"}]),
                ),
                FindingRejection::InvalidPath {
                    field: "verification",
                    path: "/abs/path.rs".to_string(),
                },
            ),
            (
                mutate(
                    "verification",
                    serde_json::json!([{"file": "tests/a.rs", "description": "  "}]),
                ),
                FindingRejection::MissingDescription {
                    field: "verification",
                    path: "tests/a.rs".to_string(),
                },
            ),
        ];

        for (output, expected) in cases {
            assert_eq!(rejection(&output), expected, "routing drifted for {output}");
        }
    }

    #[test]
    fn acceptance_duplicate_structured_ids_are_a_protocol_error() {
        let finding = serde_json::json!({
            "id": "same-id",
            "severity": "major",
            "summary": "s",
            "evidence": ["e"],
            "required_changes": [{"file": "src/a.rs", "description": "d"}],
            "verification": [{"file": "tests/a.rs", "description": "d"}]
        });
        let output =
            serde_json::json!({"acceptance": "fail", "findings": [finding.clone(), finding]})
                .to_string();
        assert_eq!(
            rejection(&output),
            FindingRejection::DuplicateId("same-id".to_string())
        );
    }

    #[test]
    fn acceptance_non_object_non_string_finding_entry_is_a_protocol_error() {
        let output = r#"{"acceptance":"fail","findings":[42]}"#;
        assert_eq!(rejection(output), FindingRejection::NotStringOrObject);
    }

    #[test]
    fn acceptance_malformed_structured_finding_never_becomes_path_only_work() {
        // Regression shape: an object naming a file but omitting verification
        // must not degrade into "fix src/a.rs" style repair instructions.
        let output = serde_json::json!({
            "acceptance": "fail",
            "findings": [{
                "id": "partial",
                "severity": "major",
                "summary": "s",
                "evidence": ["e"],
                "required_changes": [{"file": "src/a.rs", "description": "d"}]
            }]
        })
        .to_string();
        let result = parse_acceptance_output(&output);
        assert!(
            matches!(result, AcceptanceResult::MalformedFinding { .. }),
            "got {result:?}"
        );
        if let AcceptanceResult::Fail { findings } = result {
            panic!("malformed structured finding degraded into {findings:?}");
        }
    }

    #[test]
    fn acceptance_mixed_legacy_and_structured_findings_both_survive() {
        let output = serde_json::json!({
            "acceptance": "fail",
            "findings": [
                "src/legacy.rs:10 missing regression coverage",
                {
                    "id": "structured-id",
                    "severity": "major",
                    "summary": "s",
                    "evidence": ["e"],
                    "required_changes": [{"file": "src/a.rs", "description": "d"}],
                    "verification": [{"file": "tests/a.rs", "description": "d"}]
                }
            ]
        })
        .to_string();

        let findings = fail_findings(&output);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].id(), None);
        assert_eq!(
            findings[0].text(),
            "src/legacy.rs:10 missing regression coverage"
        );
        assert_eq!(findings[1].id(), Some("structured-id"));
    }

    #[test]
    fn acceptance_legacy_verdict_syntax_is_unchanged() {
        // Both legacy shapes keep their complete original text and gain no
        // structured payload.
        for output in [
            "ACCEPTANCE: FAIL\nFINDINGS:\n- src/a.rs:1 issue\n",
            r#"{"acceptance":"fail","findings":["src/a.rs:1 issue"]}"#,
        ] {
            let findings = fail_findings(output);
            assert_eq!(findings.len(), 1, "{output}");
            assert_eq!(findings[0].text(), "src/a.rs:1 issue", "{output}");
            assert!(findings[0].structured_payload().is_none(), "{output}");
            assert!(!findings[0].declares_paths(), "{output}");
        }
    }

    #[test]
    fn acceptance_repository_path_normalization_rejects_escapes() {
        assert_eq!(
            normalize_repository_path("./src//a.rs"),
            Some("src/a.rs".to_string())
        );
        assert_eq!(
            normalize_repository_path("src\\a.rs"),
            Some("src/a.rs".to_string())
        );
        for escape in [
            "",
            "  ",
            "/etc/passwd",
            "~/secrets",
            "C:/x",
            "../up",
            "a/../../b",
        ] {
            assert_eq!(normalize_repository_path(escape), None, "{escape}");
        }
    }
}

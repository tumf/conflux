//! Shared acceptance operations for CLI and TUI modes.
//!
//! Provides acceptance test execution after apply and before archive.

#![allow(dead_code)]

use crate::agent::AgentRunner;
use crate::error::{OrchestratorError, Result};
use crate::history::{AcceptanceAttempt, OutputCollector};
use crate::openspec::Change;
use tracing::{info, warn};

use super::output::OutputHandler;

const ACCEPTANCE_OUTPUT_FALLBACK: &str = "No acceptance output captured";
pub const MAX_ACCEPTANCE_RETRY_CYCLES: u32 = 10;

/// First history/finding line recorded when an acceptance command completes
/// without emitting any canonical verdict. Deliberately distinct from the
/// explicit-CONTINUE marker ("Investigation incomplete - continue later") so
/// missing-verdict attempts never count toward the consecutive-CONTINUE retry
/// budget.
pub const MISSING_VERDICT_DIAGNOSTIC: &str = "Missing acceptance verdict: acceptance command \
     exited without emitting a canonical verdict (protocol failure; status-only or waiting \
     output is not a verdict)";

/// Maximum number of acceptance protocol retries permitted after the initial
/// protocol-failing attempt. The initial invocation plus these retries gives
/// three opportunities to satisfy the verdict protocol.
///
/// This budget is deliberately separate from the configured explicit-`CONTINUE`
/// budget (`acceptance_max_continues`): a completed-but-verdictless command, or
/// a compatibility `gated` token with no validated blocker payload, is a
/// protocol failure rather than an intentional continuation request.
pub const MAX_MISSING_VERDICT_RETRIES: u32 = 2;

/// Alias documenting that bare-blocker input reuses the same fixed bound.
pub const MAX_ACCEPTANCE_PROTOCOL_RETRIES: u32 = MAX_MISSING_VERDICT_RETRIES;

/// First history/finding line recorded when acceptance emits a `gated` (or
/// legacy `blocked`) compatibility token without a validated structured blocker.
pub const BARE_BLOCKER_DIAGNOSTIC: &str = "Bare acceptance blocker: acceptance emitted a gated \
     compatibility token without a validated structured blocker payload (protocol failure; a \
     stalled hold requires an explicit supported category, concrete evidence, next action, and \
     resumability)";

/// Which acceptance protocol contract the command failed to satisfy.
///
/// Both kinds share the same fixed retry bound and both re-invoke acceptance
/// only — neither reruns apply, consumes explicit-`CONTINUE` budget, nor
/// persists stalled state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptanceProtocolError {
    /// The command exited without emitting any canonical verdict.
    MissingVerdict,
    /// The command emitted `gated`/legacy `blocked` without a validated
    /// structured blocker payload.
    BareBlocker,
    /// The command emitted a FAIL verdict whose structured finding payload did
    /// not validate.
    MalformedFinding,
}

impl AcceptanceProtocolError {
    /// Short label used in operator-facing diagnostics.
    pub fn label(self) -> &'static str {
        match self {
            AcceptanceProtocolError::MissingVerdict => "missing-verdict",
            AcceptanceProtocolError::BareBlocker => "bare-blocker",
            AcceptanceProtocolError::MalformedFinding => "malformed-finding",
        }
    }
}

/// First history/finding line recorded when acceptance emits a FAIL whose
/// structured finding payload does not validate.
pub const MALFORMED_FINDING_DIAGNOSTIC: &str =
    "Malformed acceptance finding: acceptance emitted a \
     FAIL verdict with a structured finding that is missing a stable id, severity, summary, \
     evidence, required_changes, or verification (protocol failure; runtime will not reduce it to \
     a path-only repair instruction)";

/// Marks an acceptance invocation as a protocol retry so the prompt builder can
/// inject the corrective continuation context for the specific contract that
/// was violated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptanceProtocolRetry {
    /// Which protocol contract the previous attempt failed to satisfy.
    pub kind: AcceptanceProtocolError,
    /// 1-based retry index (the first retry after the initial attempt is 1).
    pub attempt: u32,
    /// Maximum number of retries permitted after the initial attempt.
    pub max: u32,
}

/// Routing decision for a completed acceptance command that violated one of the
/// verdict protocol contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingVerdictRetryDecision {
    /// Budget remains: re-invoke the normal configured acceptance command with
    /// the continuation context described by [`AcceptanceProtocolRetry`].
    Retry(AcceptanceProtocolRetry),
    /// Budget exhausted: route to the terminal protocol failure.
    Exhausted {
        kind: AcceptanceProtocolError,
        attempts: u32,
        max: u32,
    },
}

/// Consecutive protocol-failure accounting for one contract during a single
/// active orchestration run.
///
/// Per `openspec/CONSTITUTION.md` this is active-run memory only. It is never
/// persisted, so a process restart simply re-runs acceptance for an
/// applied-but-unarchived workspace instead of resuming a retry sequence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProtocolRetryCounter {
    consecutive: u32,
}

impl ProtocolRetryCounter {
    /// Number of consecutive protocol failures observed so far.
    pub fn consecutive(&self) -> u32 {
        self.consecutive
    }

    /// Reset after any canonical verdict
    /// (PASS/FAIL/CONTINUE/validated-stall/permission-stall).
    pub fn reset(&mut self) {
        self.consecutive = 0;
    }

    /// Record one protocol failure of `kind` and return the routing decision.
    pub fn record(&mut self, kind: AcceptanceProtocolError) -> MissingVerdictRetryDecision {
        self.consecutive = self.consecutive.saturating_add(1);
        if self.consecutive <= MAX_ACCEPTANCE_PROTOCOL_RETRIES {
            MissingVerdictRetryDecision::Retry(AcceptanceProtocolRetry {
                kind,
                attempt: self.consecutive,
                max: MAX_ACCEPTANCE_PROTOCOL_RETRIES,
            })
        } else {
            MissingVerdictRetryDecision::Exhausted {
                kind,
                attempts: self.consecutive,
                max: MAX_ACCEPTANCE_PROTOCOL_RETRIES,
            }
        }
    }
}

fn bounded_missing_verdict_evidence(findings: &[String]) -> String {
    let evidence = findings
        .iter()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ");
    if evidence.is_empty() {
        "no acceptance output captured".to_string()
    } else {
        evidence
    }
}

/// Non-terminal progress message for a protocol retry.
///
/// Deliberately worded as progress, not error: the change is still acceptance
/// work in progress while retry budget remains.
pub fn missing_verdict_retry_progress(
    retry: AcceptanceProtocolRetry,
    findings: &[String],
) -> String {
    let cause = match retry.kind {
        AcceptanceProtocolError::MissingVerdict => "without a canonical verdict",
        AcceptanceProtocolError::BareBlocker => "with a gated token but no validated blocker",
        AcceptanceProtocolError::MalformedFinding => {
            "with a FAIL verdict whose structured finding did not validate"
        }
    };
    format!(
        "Acceptance completed {cause}; retrying acceptance \
         (protocol retry {}/{}). Evidence: {}",
        retry.attempt,
        retry.max,
        bounded_missing_verdict_evidence(findings)
    )
}

/// Terminal diagnostic emitted once the protocol retry budget is spent.
pub fn missing_verdict_exhausted_error(attempts: u32, max: u32, findings: &[String]) -> String {
    protocol_exhausted_error(
        AcceptanceProtocolError::MissingVerdict,
        attempts,
        max,
        findings,
    )
}

/// Terminal diagnostic for either protocol contract.
pub fn protocol_exhausted_error(
    kind: AcceptanceProtocolError,
    attempts: u32,
    max: u32,
    findings: &[String],
) -> String {
    let cause = match kind {
        AcceptanceProtocolError::MissingVerdict => {
            "Acceptance completed without a canonical verdict (missing-verdict protocol failure); \
             status-only or waiting output is not a verdict."
        }
        AcceptanceProtocolError::BareBlocker => {
            "Acceptance emitted a gated compatibility token without a validated structured blocker \
             (bare-blocker protocol failure); a stalled hold requires an explicit supported \
             category, concrete evidence, next action, and resumability."
        }
        AcceptanceProtocolError::MalformedFinding => {
            "Acceptance emitted a FAIL verdict whose structured finding did not validate \
             (malformed-finding protocol failure); a structured finding requires a stable id, a \
             major/minor severity, a summary, concrete evidence, and repository-relative \
             required_changes and verification entries."
        }
    };
    format!(
        "{cause} Exhausted {attempts} consecutive attempts after {max} protocol retries. \
         Evidence: {}",
        bounded_missing_verdict_evidence(findings)
    )
}

/// Next step after an acceptance command violated a verdict protocol contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingVerdictRetryStep {
    /// Budget remains: re-invoke the normal configured acceptance command with
    /// `retry` as the continuation marker. `progress` is the non-terminal
    /// operator-visible message.
    Retry {
        retry: AcceptanceProtocolRetry,
        progress: String,
    },
    /// Budget exhausted: route to the terminal protocol failure.
    Exhausted { error: String },
}

/// Mode-independent driver for acceptance protocol-retry sequences.
///
/// Serial and parallel orchestration share this driver so equivalent
/// observations produce equivalent routing. Each acceptance invocation reads its
/// continuation marker from [`Self::take_protocol_retry`] and reports the result
/// back through [`Self::observe_missing_verdict`],
/// [`Self::observe_bare_blocker`], or [`Self::observe_canonical_verdict`].
///
/// The two protocol contracts keep strictly independent counters: a bare
/// blocker never consumes missing-verdict budget and vice versa. Only a
/// canonical verdict resets either counter, so a run cannot alternate between
/// the two failure modes to buy unbounded retries.
///
/// All state is active-run memory. Nothing is written outside the worktree, so a
/// restarted process re-runs acceptance from workspace file/git state rather
/// than resuming a protocol-retry sequence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AcceptanceProtocolDriver {
    missing_verdict: ProtocolRetryCounter,
    bare_blocker: ProtocolRetryCounter,
    malformed_finding: ProtocolRetryCounter,
    pending: Option<AcceptanceProtocolRetry>,
}

impl AcceptanceProtocolDriver {
    /// Consume the continuation marker for the acceptance invocation that is
    /// about to start. `None` for an ordinary (non-retry) invocation.
    pub fn take_protocol_retry(&mut self) -> Option<AcceptanceProtocolRetry> {
        self.pending.take()
    }

    /// Consecutive missing verdicts observed so far.
    pub fn consecutive_missing_verdicts(&self) -> u32 {
        self.missing_verdict.consecutive()
    }

    /// Consecutive bare/invalid blocker verdicts observed so far.
    pub fn consecutive_bare_blockers(&self) -> u32 {
        self.bare_blocker.consecutive()
    }

    /// Record any canonical verdict
    /// (PASS/FAIL/CONTINUE/validated-stall/permission-stall).
    /// Canonical routing is unchanged; only the protocol sequences reset.
    pub fn observe_canonical_verdict(&mut self) {
        self.missing_verdict.reset();
        self.bare_blocker.reset();
        self.malformed_finding.reset();
        self.pending = None;
    }

    /// Consecutive malformed structured findings observed so far.
    pub fn consecutive_malformed_findings(&self) -> u32 {
        self.malformed_finding.consecutive()
    }

    /// Record a FAIL verdict whose structured finding payload did not validate.
    ///
    /// No repair work is dispatched: the runtime asks for a corrected verdict
    /// rather than guessing what the reviewer meant.
    pub fn observe_malformed_finding(
        &mut self,
        rejection: &crate::acceptance::FindingRejection,
    ) -> MissingVerdictRetryStep {
        let evidence = vec![MALFORMED_FINDING_DIAGNOSTIC.to_string(), rejection.reason()];
        let decision = self
            .malformed_finding
            .record(AcceptanceProtocolError::MalformedFinding);
        self.step_from(decision, &evidence)
    }

    /// Record a completed acceptance command that emitted no canonical verdict
    /// and return the resulting non-terminal or terminal step.
    pub fn observe_missing_verdict(&mut self, findings: &[String]) -> MissingVerdictRetryStep {
        let decision = self
            .missing_verdict
            .record(AcceptanceProtocolError::MissingVerdict);
        self.step_from(decision, findings)
    }

    /// Record a `gated`/legacy `blocked` verdict whose blocker payload did not
    /// validate, and return the resulting non-terminal or terminal step.
    ///
    /// No blocker category is inferred and no stalled state is created.
    pub fn observe_bare_blocker(
        &mut self,
        rejection: &crate::acceptance::BlockerRejection,
    ) -> MissingVerdictRetryStep {
        let evidence = vec![BARE_BLOCKER_DIAGNOSTIC.to_string(), rejection.reason()];
        let decision = self
            .bare_blocker
            .record(AcceptanceProtocolError::BareBlocker);
        self.step_from(decision, &evidence)
    }

    fn step_from(
        &mut self,
        decision: MissingVerdictRetryDecision,
        findings: &[String],
    ) -> MissingVerdictRetryStep {
        match decision {
            MissingVerdictRetryDecision::Retry(retry) => {
                self.pending = Some(retry);
                MissingVerdictRetryStep::Retry {
                    retry,
                    progress: missing_verdict_retry_progress(retry, findings),
                }
            }
            MissingVerdictRetryDecision::Exhausted {
                kind,
                attempts,
                max,
            } => {
                self.pending = None;
                MissingVerdictRetryStep::Exhausted {
                    error: protocol_exhausted_error(kind, attempts, max, findings),
                }
            }
        }
    }
}

/// Shared, mode-independent routing for an acceptance result that carries (or
/// claims to carry) an external blocker.
///
/// This is the single decision API serial and parallel orchestration use so
/// equivalent observations produce equivalent retry, stall, and terminal
/// protocol-error outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceBlockerDecision {
    /// Bare or invalid blocker input with budget remaining: re-invoke
    /// acceptance only, with corrective context. Apply is never rerun and no
    /// stalled state is created.
    ProtocolRetry {
        retry: AcceptanceProtocolRetry,
        progress: String,
    },
    /// Bare or invalid blocker input with budget spent: terminal acceptance
    /// protocol error requiring explicit retry.
    ProtocolExhausted { error: String },
    /// Validated structured external blocker: persist a revision-bound runtime
    /// stall and display `stalled`.
    Stall {
        blocker: crate::acceptance::AcceptanceBlocker,
    },
}

/// Route an acceptance result through the shared blocker/protocol policy.
///
/// Returns `None` for results this policy does not own (PASS, FAIL, CONTINUE,
/// command failure, cancellation, missing verdict), leaving their existing
/// routing untouched.
pub fn decide_acceptance_blocker(
    driver: &mut AcceptanceProtocolDriver,
    result: &AcceptanceResult,
) -> Option<AcceptanceBlockerDecision> {
    match result {
        AcceptanceResult::BareBlocker { rejection } => {
            Some(match driver.observe_bare_blocker(rejection) {
                MissingVerdictRetryStep::Retry { retry, progress } => {
                    AcceptanceBlockerDecision::ProtocolRetry { retry, progress }
                }
                MissingVerdictRetryStep::Exhausted { error } => {
                    AcceptanceBlockerDecision::ProtocolExhausted { error }
                }
            })
        }
        AcceptanceResult::MalformedFinding { rejection } => {
            Some(match driver.observe_malformed_finding(rejection) {
                MissingVerdictRetryStep::Retry { retry, progress } => {
                    AcceptanceBlockerDecision::ProtocolRetry { retry, progress }
                }
                MissingVerdictRetryStep::Exhausted { error } => {
                    AcceptanceBlockerDecision::ProtocolExhausted { error }
                }
            })
        }
        AcceptanceResult::Stalled { blocker } => {
            // A validated stall is a canonical verdict: it resets every protocol
            // sequence and follows the durable stalled-hold path.
            driver.observe_canonical_verdict();
            Some(AcceptanceBlockerDecision::Stall {
                blocker: blocker.clone(),
            })
        }
        _ => None,
    }
}

/// A finding paired with its compact comparison identity.
///
/// `identity` exists purely for retry accounting and reconciliation. `finding`
/// keeps the complete actionable payload, so nothing downstream has to
/// reconstruct evidence, required changes, or verification from the identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedFinding {
    pub identity: String,
    pub text: String,
    pub external: bool,
    pub finding: crate::acceptance::AcceptanceFinding,
}

/// Derive comparison identities for a finding list.
///
/// A structured finding's identity is its reviewer-authored `id`, so changed
/// prose, line numbers, evidence, or cited paths cannot manufacture a new
/// identity. Legacy string findings keep the historical deterministic fallback.
pub fn normalize_findings(
    findings: &[crate::acceptance::AcceptanceFinding],
) -> Vec<NormalizedFinding> {
    let mut normalized = findings
        .iter()
        .filter_map(|finding| {
            let structured = finding.structured_payload()?;
            Some(NormalizedFinding {
                identity: format!("repository|id|{}", structured.id),
                text: finding.text().to_string(),
                external: false,
                finding: finding.clone(),
            })
        })
        .collect::<Vec<_>>();
    normalized.extend(normalize_legacy_findings(
        findings
            .iter()
            .filter(|finding| finding.structured_payload().is_none()),
    ));
    normalized.sort_by(|left, right| left.identity.cmp(&right.identity));
    normalized.dedup_by(|left, right| left.identity == right.identity);
    normalized
}

fn normalize_legacy_findings<'a, I>(findings: I) -> Vec<NormalizedFinding>
where
    I: IntoIterator<Item = &'a crate::acceptance::AcceptanceFinding>,
{
    let texts = findings
        .into_iter()
        .map(|finding| finding.text().to_string())
        .collect::<Vec<_>>();
    normalize_legacy_texts(&texts)
}

/// Legacy fallback identity derivation, retained verbatim for string findings.
pub fn normalize_legacy_texts(findings: &[String]) -> Vec<NormalizedFinding> {
    fn rule_kind(text: &str) -> &'static str {
        if ["test", "coverage", "verification", "evidence"]
            .iter()
            .any(|word| text.contains(word))
        {
            "verification"
        } else if ["spec", "proposal", "requirement"]
            .iter()
            .any(|word| text.contains(word))
        {
            "specification"
        } else if ["task", "checklist", "truthful"]
            .iter()
            .any(|word| text.contains(word))
        {
            "task-truthfulness"
        } else if text.contains("dirty working tree") {
            "workspace-cleanliness"
        } else {
            "implementation"
        }
    }

    let mut normalized = findings
        .iter()
        .filter_map(|finding| {
            let normalized = finding.split_whitespace().collect::<Vec<_>>().join(" ");
            (!normalized.is_empty()).then(|| {
                let lower = normalized.to_ascii_lowercase();
                let finding_code = lower
                    .split_whitespace()
                    .next()
                    .filter(|word| word.starts_with('[') && word.ends_with(']'));
                let path_token = lower
                    .split_whitespace()
                    .find(|word| {
                        word.contains('/') || word.ends_with(".rs") || word.ends_with(".md")
                    })
                    .unwrap_or("");
                let path = path_token
                    .trim_matches(|character: char| {
                        matches!(character, '`' | '(' | ')' | '[' | ']' | ',' | '.' | ';')
                    })
                    .split(':')
                    .next()
                    .unwrap_or("");
                // An explicit non-mockable prerequisite is external only when the
                // finding has no repository target or requested repository repair.
                let external = path.is_empty()
                    && !lower.contains("fix ")
                    && !lower.contains("repair ")
                    && [
                        "external non-mockable",
                        "non-mockable external",
                        "external prerequisite",
                        "external service outage",
                        "missing non-mockable external credential",
                    ]
                    .iter()
                    .any(|needle| lower.contains(needle));
                let scope = if external { "external" } else { "repository" };
                NormalizedFinding {
                    identity: finding_code.map_or_else(
                        || {
                            let location = if path.is_empty() {
                                lower.as_str()
                            } else {
                                path
                            };
                            format!("{scope}|{location}|{}", rule_kind(&lower))
                        },
                        |code| format!("{scope}|code|{code}"),
                    ),
                    finding: crate::acceptance::AcceptanceFinding::legacy(normalized.clone()),
                    text: normalized,
                    external,
                }
            })
        })
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| left.identity.cmp(&right.identity));
    normalized.dedup_by(|left, right| left.identity == right.identity);
    normalized
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceRetryDecision {
    Retry {
        reason: &'static str,
    },
    Stall {
        reason: &'static str,
        external_blockers: Vec<String>,
    },
}

/// Findings that repository work can resolve.
///
/// Structured findings are always repository-scoped: they declare repository
/// paths. Only legacy strings can be classified as external prerequisites.
pub fn repository_findings(
    findings: &[crate::acceptance::AcceptanceFinding],
) -> Vec<crate::acceptance::AcceptanceFinding> {
    findings
        .iter()
        .filter(|finding| {
            finding.structured_payload().is_some()
                || normalize_legacy_texts(&[finding.text().to_string()])
                    .first()
                    .is_some_and(|normalized| !normalized.external)
        })
        .cloned()
        .collect()
}

pub fn semantic_progress_fingerprint(workspace: &std::path::Path) -> std::io::Result<String> {
    fn include(path: &str) -> bool {
        !path.starts_with(".git/")
            && !path.starts_with(".cflx/")
            && !path.contains("/APPLY_BLOCKED/")
            && !path.starts_with("logs/")
            && !path.starts_with("history/")
            && (path.starts_with("src/")
                || path.starts_with("tests/")
                || path.starts_with("config/")
                || path.starts_with("openspec/specs/")
                || path.contains("/specs/")
                || path == ".cflx.jsonc"
                || path.ends_with("/.cflx.jsonc")
                || path.ends_with("Cargo.toml")
                || path.ends_with("tasks.md"))
    }
    fn visit(
        root: &std::path::Path,
        directory: &std::path::Path,
        output: &mut Vec<(String, Vec<u8>)>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, output)?;
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if include(&relative) {
                let mut contents = std::fs::read(path)?;
                if relative.ends_with("tasks.md") {
                    let text = String::from_utf8_lossy(&contents);
                    contents = text
                        .split("\n## Current Acceptance Follow-up")
                        .next()
                        .unwrap_or(&text)
                        .split("\n## Acceptance #")
                        .next()
                        .unwrap_or(&text)
                        .as_bytes()
                        .to_vec();
                }
                output.push((relative, contents));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    visit(workspace, workspace, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let hash = files
        .into_iter()
        .flat_map(|(path, bytes)| path.into_bytes().into_iter().chain(bytes))
        .fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ byte as u64).wrapping_mul(0x100000001b3)
        });
    Ok(format!("{hash:016x}"))
}

/// Stop reason for a repair hold that precedes the next Acceptance invocation.
///
/// Both reasons are temporary, evidenced, resumable holds. Neither proves
/// implementation completion, finding closure, PASS, archive readiness, or
/// merge eligibility, and neither writes anything into the managed worktree.
pub const REMEDIATION_MISMATCH_REASON: &str = "acceptance_remediation_mismatch";
/// Stop reason used when the same finding ID survives its one repair Apply.
pub const REPEATED_FINDING_REASON: &str = "repeated_acceptance_finding";

/// Result of comparing declared finding work against the actual repair delta.
///
/// This is a coverage gate, not semantic acceptance: passing it only authorizes
/// the next Acceptance invocation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemediationCoverage {
    /// Files observed in the repair delta, sorted and deduplicated.
    pub changed_files: Vec<String>,
    /// Declared `required_changes` paths absent from the delta, per finding ID.
    pub missing_required: Vec<(String, String)>,
    /// Declared `verification` paths absent from the delta, per finding ID.
    pub missing_verification: Vec<(String, String)>,
    /// Changed files not declared by any finding.
    pub unrelated_files: Vec<String>,
    /// Findings that declare no path set and therefore keep legacy behavior.
    pub legacy_finding_texts: Vec<String>,
}

impl RemediationCoverage {
    /// Whether every declared implementation and verification path was covered.
    pub fn is_complete(&self) -> bool {
        self.missing_required.is_empty() && self.missing_verification.is_empty()
    }

    /// Uncovered declared paths, formatted for operator diagnostics.
    pub fn uncovered(&self) -> Vec<String> {
        self.missing_required
            .iter()
            .map(|(id, file)| format!("{id}: required_changes {file}"))
            .chain(
                self.missing_verification
                    .iter()
                    .map(|(id, file)| format!("{id}: verification {file}")),
            )
            .collect()
    }
}

/// Normalize one raw changed-path entry from a VCS status/diff listing.
fn normalize_changed_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // `git diff --name-status` renames arrive as `old -> new`; both sides count
    // as touched so a finding satisfied by moving a file is still covered.
    crate::acceptance::normalize_repository_path(trimmed)
}

/// Collect the distinct repository-relative files in a repair delta.
pub fn normalize_changed_files<I, S>(paths: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut files = paths
        .into_iter()
        .flat_map(|path| {
            path.as_ref()
                .split(" -> ")
                .filter_map(normalize_changed_path)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    files
}

/// Validate that a repair delta covers every declared finding expectation.
///
/// Pure over the changed-file list so the decision is unit-testable without a
/// real repository. Coverage never claims the finding is semantically resolved.
pub fn evaluate_remediation_coverage(
    findings: &[crate::acceptance::AcceptanceFinding],
    changed_files: &[String],
) -> RemediationCoverage {
    let changed = normalize_changed_files(changed_files);
    let mut coverage = RemediationCoverage {
        changed_files: changed.clone(),
        ..RemediationCoverage::default()
    };
    let mut declared = std::collections::BTreeSet::new();

    for finding in findings {
        // A finding that declares no path set keeps legacy compatibility
        // behavior: strict coverage has nothing to enforce against.
        if !finding.declares_paths() {
            coverage
                .legacy_finding_texts
                .push(finding.text().to_string());
            continue;
        }
        let Some(structured) = finding.structured_payload() else {
            continue;
        };
        for file in structured.required_files() {
            declared.insert(file.clone());
            if !changed.contains(&file) {
                coverage
                    .missing_required
                    .push((structured.id.clone(), file));
            }
        }
        for file in structured.verification_files() {
            declared.insert(file.clone());
            if !changed.contains(&file) {
                coverage
                    .missing_verification
                    .push((structured.id.clone(), file));
            }
        }
    }

    coverage.unrelated_files = changed
        .into_iter()
        .filter(|file| !declared.contains(file))
        .collect();
    coverage
}

/// Per-finding automatic repair accounting for one active orchestration run.
///
/// Each stable ID gets exactly one automatic repair Apply after its first FAIL
/// observation. Unrelated semantic progress never resets that budget.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FindingRepairLedger {
    /// Identities that have already consumed their one automatic repair Apply.
    repaired: std::collections::BTreeSet<String>,
    /// How many canonical FAIL results reported each identity.
    occurrences: std::collections::BTreeMap<String, u32>,
}

/// What runtime should do after ingesting a canonical FAIL result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingRepairDecision {
    /// Every open finding still has its automatic repair opportunity.
    Repair {
        /// Identities granted (or continuing to hold) a repair opportunity.
        identities: Vec<String>,
    },
    /// At least one identity already consumed its repair opportunity.
    ///
    /// Runtime stops atomically: no partial Apply is dispatched even when other
    /// identities in the same verdict are new.
    Stop {
        reason: &'static str,
        repeated_identities: Vec<String>,
    },
}

impl FindingRepairLedger {
    /// Occurrence count for one identity across this run.
    pub fn occurrences(&self, identity: &str) -> u32 {
        self.occurrences.get(identity).copied().unwrap_or(0)
    }

    /// Every observed identity with its occurrence count, for diagnostics.
    pub fn occurrence_report(&self) -> Vec<(String, u32)> {
        self.occurrences
            .iter()
            .map(|(identity, count)| (identity.clone(), *count))
            .collect()
    }

    /// Whether an identity already consumed its automatic repair opportunity.
    pub fn has_consumed_repair(&self, identity: &str) -> bool {
        self.repaired.contains(identity)
    }

    /// Ingest a canonical FAIL result and decide whether repair may proceed.
    ///
    /// Identities absent from `findings` were closed by Acceptance, so their
    /// accounting is dropped: only Acceptance closes a finding.
    pub fn observe_fail(&mut self, findings: &[NormalizedFinding]) -> FindingRepairDecision {
        let identities = findings
            .iter()
            .filter(|finding| !finding.external)
            .map(|finding| finding.identity.clone())
            .collect::<Vec<_>>();
        let open = identities
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();

        self.occurrences
            .retain(|identity, _| open.contains(identity));
        self.repaired.retain(|identity| open.contains(identity));
        for identity in &identities {
            *self.occurrences.entry(identity.clone()).or_insert(0) += 1;
        }

        let repeated = identities
            .iter()
            .filter(|identity| self.repaired.contains(*identity))
            .cloned()
            .collect::<Vec<_>>();
        if !repeated.is_empty() {
            return FindingRepairDecision::Stop {
                reason: REPEATED_FINDING_REASON,
                repeated_identities: repeated,
            };
        }
        FindingRepairDecision::Repair { identities }
    }

    /// Record that the automatic repair Apply for `identities` was dispatched.
    pub fn record_repair_dispatched(&mut self, identities: &[String]) {
        for identity in identities {
            self.repaired.insert(identity.clone());
        }
    }

    /// Drop all accounting, used when an operator authorizes a new attempt.
    ///
    /// Occurrence diagnostics are the caller's to preserve; this only releases
    /// the automatic budget so an explicit retry can proceed.
    pub fn reset_for_explicit_retry(&mut self) {
        self.repaired.clear();
    }
}

/// Structured, mode-independent diagnostics for a repair stop.
///
/// Serial and parallel build this from the same inputs so equivalent
/// observations produce equivalent operator evidence. It is in-memory state for
/// one process lifetime and controls only stalled presentation, dispatch
/// suppression, and explicit-retry eligibility: it never proves implementation
/// completion, finding closure, Acceptance PASS, archive readiness, or merge
/// eligibility, and it is never written into a managed worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptanceRepairStop {
    pub change_id: String,
    pub reason: &'static str,
    /// Every open finding, complete and unmodified.
    pub findings: Vec<crate::acceptance::AcceptanceFinding>,
    /// Comparison identity plus how many canonical FAILs reported it.
    pub occurrences: Vec<(String, u32)>,
    /// Identities that already consumed their one automatic repair.
    pub repeated_identities: Vec<String>,
    /// Revision recorded when the FAIL was observed.
    pub fail_revision: Option<String>,
    /// Revision produced by the repair Apply.
    pub apply_revision: Option<String>,
    /// Declared implementation and verification paths.
    pub required_files: Vec<String>,
    pub verification_files: Vec<String>,
    /// Coverage result over the actual repair delta.
    pub coverage: RemediationCoverage,
    /// Apply-authored remediation evidence recovered from workspace follow-up.
    pub remediation_evidence: Vec<String>,
    pub resumable: bool,
    pub next_action: String,
}

impl AcceptanceRepairStop {
    /// Machine-readable diagnostics for CLI/TUI/event consumers.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "change_id": self.change_id,
            "stop_reason": self.reason,
            "findings": self.findings.iter().map(|finding| finding.to_json()).collect::<Vec<_>>(),
            "finding_occurrences": self
                .occurrences
                .iter()
                .map(|(identity, count)| serde_json::json!({"identity": identity, "occurrences": count}))
                .collect::<Vec<_>>(),
            "repeated_identities": self.repeated_identities,
            "fail_revision": self.fail_revision,
            "apply_revision": self.apply_revision,
            "required_files": self.required_files,
            "verification_files": self.verification_files,
            "changed_files": self.coverage.changed_files,
            "uncovered_files": self.coverage.uncovered(),
            "coverage_complete": self.coverage.is_complete(),
            "unrelated_files": self.coverage.unrelated_files,
            "legacy_findings_without_declared_paths": self.coverage.legacy_finding_texts,
            "remediation_evidence": self.remediation_evidence,
            "resumable": self.resumable,
            "next_action": self.next_action,
            "proves_completion": false,
            "proves_acceptance_pass": false,
            "proves_archive_readiness": false,
        })
    }

    /// Identities present in the occurrence report, in stable order.
    pub fn occurrence_identities(&self) -> Vec<String> {
        self.occurrences
            .iter()
            .map(|(identity, _)| identity.clone())
            .collect()
    }

    /// One-line operator-facing summary used as the process-result error text.
    pub fn summary(&self) -> String {
        format!(
            "Acceptance stopped automatic repair for {} ({}). {} Diagnostics: {}",
            self.change_id,
            self.reason,
            self.next_action,
            self.to_json()
        )
    }
}

/// What runtime should do before invoking Acceptance again after a repair Apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairGateDecision {
    /// Declared coverage is satisfied (or nothing was declared): Acceptance may
    /// run. This authorizes only the next review, never semantic resolution.
    Proceed,
    /// Coverage is incomplete: hold with evidence and do not spend an Acceptance
    /// invocation.
    Stop(Box<AcceptanceRepairStop>),
}

/// Validate a repair delta against every open structured finding.
///
/// Pure over `changed_files` and `remediation_evidence` so serial and parallel
/// share one decision and it stays unit-testable without a real repository.
pub fn decide_repair_gate(
    change_id: &str,
    findings: &[crate::acceptance::AcceptanceFinding],
    ledger: &FindingRepairLedger,
    fail_revision: Option<&str>,
    apply_revision: Option<&str>,
    changed_files: &[String],
    remediation_evidence: &[String],
) -> RepairGateDecision {
    let coverage = evaluate_remediation_coverage(findings, changed_files);
    if coverage.is_complete() {
        return RepairGateDecision::Proceed;
    }
    let uncovered = coverage.uncovered().join(", ");
    RepairGateDecision::Stop(Box::new(AcceptanceRepairStop {
        change_id: change_id.to_string(),
        reason: REMEDIATION_MISMATCH_REASON,
        findings: findings.to_vec(),
        occurrences: ledger.occurrence_report(),
        repeated_identities: Vec::new(),
        fail_revision: fail_revision.map(str::to_string),
        apply_revision: apply_revision.map(str::to_string),
        required_files: declared_files(findings, true),
        verification_files: declared_files(findings, false),
        coverage,
        remediation_evidence: remediation_evidence.to_vec(),
        resumable: true,
        next_action: format!(
            "Change the declared files still missing from the repair delta ({uncovered}), then \
             retry explicitly. Unrelated or comment-only changes cannot satisfy the finding \
             contract, and coverage alone never proves the finding is resolved."
        ),
    }))
}

/// Build the repeated-finding stop record for an ingested canonical FAIL.
#[allow(clippy::too_many_arguments)]
pub fn repeated_finding_stop(
    change_id: &str,
    findings: &[crate::acceptance::AcceptanceFinding],
    ledger: &FindingRepairLedger,
    repeated_identities: Vec<String>,
    fail_revision: Option<&str>,
    apply_revision: Option<&str>,
    changed_files: &[String],
    remediation_evidence: &[String],
) -> AcceptanceRepairStop {
    AcceptanceRepairStop {
        change_id: change_id.to_string(),
        reason: REPEATED_FINDING_REASON,
        findings: findings.to_vec(),
        occurrences: ledger.occurrence_report(),
        next_action: format!(
            "Finding {} is still open after its one automatic repair. Review the finding and the \
             recorded remediation evidence, then retry explicitly; unrelated changed files do not \
             grant another automatic repair.",
            repeated_identities.join(", ")
        ),
        repeated_identities,
        fail_revision: fail_revision.map(str::to_string),
        apply_revision: apply_revision.map(str::to_string),
        required_files: declared_files(findings, true),
        verification_files: declared_files(findings, false),
        coverage: evaluate_remediation_coverage(findings, changed_files),
        remediation_evidence: remediation_evidence.to_vec(),
        resumable: true,
    }
}

fn declared_files(
    findings: &[crate::acceptance::AcceptanceFinding],
    required: bool,
) -> Vec<String> {
    let mut files = findings
        .iter()
        .filter_map(|finding| finding.structured_payload())
        .flat_map(|finding| {
            if required {
                finding.required_files()
            } else {
                finding.verification_files()
            }
        })
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    files
}

/// Collect the repair delta and Apply-authored remediation evidence for a
/// change, so both execution modes feed [`decide_repair_gate`] identically.
///
/// Missing git or follow-up state degrades to empty inputs rather than an
/// inferred pass: an empty delta simply fails coverage for a declared finding.
pub async fn collect_repair_gate_inputs(
    workspace: &std::path::Path,
    change_id: &str,
    fail_revision: Option<&str>,
) -> (Vec<String>, Option<String>, Vec<String>) {
    let changed_files = match fail_revision {
        Some(revision) => crate::vcs::git::commands::get_changed_files_since(workspace, revision)
            .await
            .unwrap_or_default(),
        None => Vec::new(),
    };
    let apply_revision = crate::vcs::git::commands::get_current_commit(workspace)
        .await
        .ok();
    let remediation_evidence =
        crate::task_parser::resolve_acceptance_follow_up_tasks_path(change_id, workspace)
            .ok()
            .and_then(|path| crate::task_parser::read_acceptance_follow_up_evidence(&path).ok())
            .unwrap_or_default();
    (changed_files, apply_revision, remediation_evidence)
}

pub fn decide_acceptance_retry(
    previous_identities: &[String],
    previous_fingerprint: Option<&str>,
    findings: &[NormalizedFinding],
    semantic_fingerprint: &str,
    cycle_count: u32,
) -> AcceptanceRetryDecision {
    let identities = findings
        .iter()
        .map(|finding| finding.identity.clone())
        .collect::<Vec<_>>();
    let external_blockers = findings
        .iter()
        .filter(|finding| finding.external)
        .map(|finding| finding.identity.clone())
        .collect();
    if cycle_count >= MAX_ACCEPTANCE_RETRY_CYCLES {
        return AcceptanceRetryDecision::Stall {
            reason: "acceptance_cycle_limit_exhausted",
            external_blockers,
        };
    }
    if !findings.is_empty() && findings.iter().all(|finding| finding.external) {
        return AcceptanceRetryDecision::Stall {
            reason: "external_acceptance_blocker",
            external_blockers,
        };
    }
    if previous_identities.is_empty() {
        return AcceptanceRetryDecision::Retry {
            reason: "first_acceptance_failure",
        };
    }
    if previous_identities != identities || previous_fingerprint != Some(semantic_fingerprint) {
        return AcceptanceRetryDecision::Retry {
            reason: "finding_or_semantic_progress_changed",
        };
    }
    AcceptanceRetryDecision::Stall {
        reason: "repeated_acceptance_findings",
        external_blockers,
    }
}

pub fn build_acceptance_tail_findings(
    stdout_tail: Option<String>,
    stderr_tail: Option<String>,
) -> Vec<String> {
    let stdout = stdout_tail.filter(|text| !text.trim().is_empty());
    let stderr = stderr_tail.filter(|text| !text.trim().is_empty());
    let selected = stdout
        .or(stderr)
        .unwrap_or_else(|| ACCEPTANCE_OUTPUT_FALLBACK.to_string());
    let lines = selected
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            // Filter out empty lines, ACCEPTANCE: markers, and FINDINGS: lines
            !trimmed.is_empty()
                && !trimmed.starts_with("ACCEPTANCE:")
                && !trimmed.starts_with("FINDINGS:")
        })
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        vec![ACCEPTANCE_OUTPUT_FALLBACK.to_string()]
    } else {
        lines
    }
}

/// Result of an acceptance operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceResult {
    /// Acceptance passed - can proceed to archive.
    Pass,
    /// Acceptance failed - must return to apply loop.
    Fail {
        findings: Vec<crate::acceptance::AcceptanceFinding>,
    },
    /// Acceptance requires more investigation - retry acceptance.
    Continue,
    /// A FAIL verdict carried a structured `findings` entry that did not
    /// validate. Bounded protocol error: acceptance is re-invoked for a
    /// corrected verdict and no ambiguous repair work is dispatched.
    MalformedFinding {
        rejection: crate::acceptance::FindingRejection,
    },
    /// A `gated`/legacy `blocked` compatibility token arrived without a
    /// validated structured blocker. This is a bounded protocol error: it never
    /// creates a stalled lifecycle transition, blocker category, or durable
    /// record, and it never reruns apply.
    BareBlocker {
        rejection: crate::acceptance::BlockerRejection,
    },
    /// A validated structured external blocker - enter the non-terminal
    /// `stalled` hold and stop the apply loop.
    Stalled {
        blocker: crate::acceptance::AcceptanceBlocker,
    },
    /// Acceptance command execution failed (non-zero exit).
    CommandFailed {
        error: String,
        findings: Vec<String>,
    },
    /// Acceptance detected a repeated unresolved permission/policy blocker.
    PermissionStalled {
        blocker: crate::events::StalledBlocker,
    },
    /// Acceptance command completed without emitting any canonical verdict.
    /// This is a protocol failure (for example a status-only "waiting for
    /// verification" exit) and is intentionally distinct from an explicit
    /// canonical `Continue`; it must never consume the explicit-CONTINUE
    /// retry path.
    MissingVerdict { findings: Vec<String> },
    /// Acceptance was cancelled (e.g., by user or timeout).
    Cancelled,
}

impl AcceptanceResult {
    /// Returns true if acceptance passed.
    pub fn is_pass(&self) -> bool {
        matches!(self, AcceptanceResult::Pass)
    }

    /// Returns true when the acceptance command emitted a canonical verdict
    /// (PASS/FAIL/CONTINUE/validated-stall/permission-stall).
    ///
    /// A missing verdict and a bare blocker are protocol failures, and command
    /// failure and cancellation are not verdicts at all: none of them are
    /// canonical, so none of them reset or consume a protocol retry budget
    /// other than their own.
    pub fn is_canonical_verdict(&self) -> bool {
        matches!(
            self,
            AcceptanceResult::Pass
                | AcceptanceResult::Fail { .. }
                | AcceptanceResult::Continue
                | AcceptanceResult::Stalled { .. }
                | AcceptanceResult::PermissionStalled { .. }
        )
    }
}

/// Run acceptance test for a change with streaming output.
///
/// # Arguments
/// * `change` - The change to test
/// * `agent` - The agent runner for history tracking
/// * `ai_runner` - The AI command runner for command execution
/// * `config` - Orchestrator configuration
/// * `output` - Output handler for streaming command output
/// * `cancel_check` - Function to check if operation should be cancelled
///
/// # Returns
/// * `Ok((AcceptanceResult::Pass, attempt_number))` - Acceptance passed
/// * `Ok((AcceptanceResult::Fail { findings }, attempt_number))` - Acceptance failed with findings
/// * `Ok((AcceptanceResult::CommandFailed { error, findings }, attempt_number))` - Command execution failed
/// * `Ok((AcceptanceResult::Cancelled, attempt_number))` - Operation was cancelled
/// * `Err(e)` - An error occurred
///
/// The attempt_number is the number of the acceptance attempt that was just recorded.
///
/// `protocol_retry` is `Some` only when this invocation continues a previous
/// attempt that exited without a canonical verdict.
#[allow(clippy::too_many_arguments)]
pub async fn acceptance_test_streaming<O, F>(
    change: &Change,
    agent: &mut AgentRunner,
    ai_runner: &crate::ai_command_runner::AiCommandRunner,
    _config: &crate::config::OrchestratorConfig,
    output: &O,
    cancel_check: F,
    protocol_retry: Option<AcceptanceProtocolRetry>,
) -> Result<(AcceptanceResult, u32, String)>
where
    O: OutputHandler,
    F: Fn() -> bool,
{
    use crate::agent::OutputLine;

    info!("Running acceptance test for: {}", change.id);
    output.on_info(&format!("Acceptance test: {}", change.id));

    // Capture current commit hash for diff tracking
    let commit_hash = crate::vcs::git::commands::get_current_commit(".")
        .await
        .ok(); // Allow to fail silently (non-git repos)

    // Get current branch for diff context (first acceptance needs base branch)
    let base_branch = crate::vcs::git::commands::get_current_branch(".")
        .await
        .ok()
        .flatten(); // None if in detached HEAD or non-git repo

    // Execute acceptance command with streaming via AiCommandRunner (real process handle)
    let (mut child, mut output_rx, start_time, command) = agent
        .run_acceptance_streaming_with_runner(
            &change.id,
            ai_runner,
            None,
            base_branch.as_deref(),
            protocol_retry,
        )
        .await?;

    // Log acceptance started with command
    output.on_info(&format!("Acceptance started: {}", change.id));
    output.on_info(&format!(
        "  {}",
        crate::events::command_log_summary(&command)
    ));

    // Create output collector for history and parsing
    let mut output_collector = OutputCollector::new();
    let mut full_stdout = String::new();

    // Grace period after detecting an acceptance marker before terminating the process.
    // This handles the case where the agent process (or its child processes) does not exit
    // promptly after emitting ACCEPTANCE: PASS/FAIL/etc., for example because
    // child processes (MCP servers) keep stdout/stderr pipes open.
    const MARKER_GRACE_PERIOD: std::time::Duration = std::time::Duration::from_secs(30);

    // Stream output until channel closes or acceptance marker detected + grace period
    let mut marker_detected = false;
    let mut verdict_stream_detector = crate::acceptance::VerdictStreamDetector::default();
    let mut marker_deadline: Option<tokio::time::Instant> = None;
    let mut early_terminated = false;

    loop {
        let recv_future = output_rx.recv();

        let line = if let Some(deadline) = marker_deadline {
            // After marker detection, apply a timeout for remaining output
            match tokio::time::timeout_at(deadline, recv_future).await {
                Ok(Some(line)) => line,
                Ok(None) => break, // Channel closed normally
                Err(_) => {
                    // Grace period expired — terminate the process
                    warn!(
                        "Acceptance marker grace period expired for {}, terminating process",
                        change.id
                    );
                    let _ = child.terminate();
                    early_terminated = true;
                    break;
                }
            }
        } else {
            match tokio::time::timeout(std::time::Duration::from_millis(50), recv_future).await {
                Ok(Some(line)) => line,
                Ok(None) => break, // Channel closed normally
                Err(_) => {
                    if cancel_check() {
                        warn!("Acceptance test cancelled while waiting for output");
                        output.on_warn("Acceptance test cancelled");
                        let _ = child.terminate();
                        return Ok((AcceptanceResult::Cancelled, 0, command));
                    }
                    continue;
                }
            }
        };

        // Check for cancellation
        if cancel_check() {
            warn!("Acceptance test cancelled for: {}", change.id);
            output.on_warn("Acceptance test cancelled");
            let _ = child.terminate();
            // Note: For cancellation, we don't record an attempt, so return 0
            return Ok((AcceptanceResult::Cancelled, 0, command));
        }

        match line {
            OutputLine::Stdout(s) => {
                output_collector.add_stdout(&s);
                full_stdout.push_str(&s);
                full_stdout.push('\n');
                output.on_stdout(&s);

                // Detect a canonical verdict in stdout to start the grace
                // period. This prevents indefinite blocking when the agent
                // process does not exit after emitting the verdict. The
                // detector recognises the primary strict JSON verdict (as a
                // standalone line or wrapped in a supported agent JSONL event)
                // and, as fallback, the legacy standalone plain-text
                // marker. Malformed markers with trailing text (for example
                // "ACCEPTANCE: PASSAll ...") do NOT trigger early completion.
                if !marker_detected && verdict_stream_detector.detect(&s).is_some() {
                    marker_detected = true;
                    marker_deadline = Some(tokio::time::Instant::now() + MARKER_GRACE_PERIOD);
                    info!(
                        "Acceptance canonical verdict detected for {}, starting {}s grace period",
                        change.id,
                        MARKER_GRACE_PERIOD.as_secs()
                    );
                }
            }
            OutputLine::Stderr(s) => {
                output_collector.add_stderr(&s);
                output.on_agent_stderr(&s);
            }
        }
    }

    // Child has exited, wait for status. Keep this cancellation-aware because
    // the output channel may close before the process status is reaped.
    let status = loop {
        if cancel_check() {
            warn!(
                "Acceptance test cancelled while waiting for child status for: {}",
                change.id
            );
            output.on_warn("Acceptance test cancelled");
            let _ = child.terminate();
            return Ok((AcceptanceResult::Cancelled, 0, command));
        }

        match tokio::time::timeout(std::time::Duration::from_millis(50), child.wait()).await {
            Ok(status) => {
                break status.map_err(|e| {
                    OrchestratorError::AgentCommand(format!(
                        "Failed to wait for acceptance command for change '{}': {}",
                        change.id, e
                    ))
                })?;
            }
            Err(_) => continue,
        }
    };

    // Record attempt
    let stdout_tail = output_collector.stdout_tail();
    let stderr_tail = output_collector.stderr_tail();

    // Build tail findings for history recording (last N lines, used in AcceptanceAttempt).
    let tail_findings = build_acceptance_tail_findings(stdout_tail.clone(), stderr_tail.clone());

    // A verdict-finalized run is one we terminated after observing the
    // canonical standalone verdict. The non-zero exit from termination is
    // expected — the verdict drives the final result.
    let verdict_finalized_run = early_terminated && marker_detected;

    // Check if command failed (skip when verdict already finalized).
    if !status.success() && !verdict_finalized_run {
        let error_msg = format!(
            "Acceptance command failed with exit code: {:?}",
            status.code()
        );
        let attempt_number = agent.next_acceptance_attempt_number(&change.id);
        let attempt = AcceptanceAttempt {
            attempt: attempt_number,
            passed: false,
            duration: start_time.elapsed(),
            findings: Some(crate::acceptance::legacy_findings(tail_findings.clone())),
            exit_code: status.code(),
            stdout_tail,
            stderr_tail,
            commit_hash: commit_hash.clone(),
        };
        agent.record_acceptance_attempt(&change.id, attempt);
        output.on_error(&error_msg);
        return Ok((
            AcceptanceResult::CommandFailed {
                error: error_msg,
                findings: tail_findings,
            },
            attempt_number,
            command,
        ));
    }

    // Parse acceptance output to determine result
    let parsed_result = crate::acceptance::parse_acceptance_output(&full_stdout);

    let (result, passed) = match parsed_result {
        crate::acceptance::AcceptanceResult::Pass => {
            info!("Acceptance test passed for: {}", change.id);
            output.on_info("Acceptance test: PASS");
            (AcceptanceResult::Pass, true)
        }
        crate::acceptance::AcceptanceResult::Fail {
            findings: parsed_findings,
        } => {
            info!("Acceptance test failed for: {}", change.id);
            output.on_warn("Acceptance test: FAIL");
            let findings = if parsed_findings.is_empty() {
                crate::acceptance::legacy_findings([
                    "Investigate acceptance failure and apply the required fix",
                ])
            } else {
                parsed_findings
            };
            (AcceptanceResult::Fail { findings }, false)
        }
        crate::acceptance::AcceptanceResult::Continue => {
            info!("Acceptance requires continuation for: {}", change.id);
            output.on_info("Acceptance test: CONTINUE");
            (AcceptanceResult::Continue, false)
        }
        crate::acceptance::AcceptanceResult::Stalled { blocker } => {
            info!(
                "Acceptance reported a validated external blocker for: {} (category {})",
                change.id, blocker.category
            );
            output.on_warn(&format!(
                "Acceptance test: STALLED ({}) - {}",
                blocker.category, blocker.next_action
            ));
            (AcceptanceResult::Stalled { blocker }, false)
        }
        crate::acceptance::AcceptanceResult::MalformedFinding { rejection } => {
            warn!(
                "Acceptance emitted a malformed structured finding for: {} ({})",
                change.id,
                rejection.reason()
            );
            output.on_error(&format!(
                "Acceptance test: MALFORMED FINDING (protocol failure — {})",
                rejection.reason()
            ));
            (AcceptanceResult::MalformedFinding { rejection }, false)
        }
        crate::acceptance::AcceptanceResult::BareBlocker { rejection } => {
            warn!(
                "Acceptance emitted a bare blocker compatibility token for: {} ({})",
                change.id,
                rejection.reason()
            );
            output.on_error(&format!(
                "Acceptance test: BARE BLOCKER (protocol failure — {})",
                rejection.reason()
            ));
            (AcceptanceResult::BareBlocker { rejection }, false)
        }
        crate::acceptance::AcceptanceResult::MissingVerdict => {
            warn!(
                "Acceptance completed without a canonical verdict for: {} (missing-verdict protocol failure)",
                change.id
            );
            output.on_error("Acceptance test: MISSING VERDICT (protocol failure — the acceptance command exited without a canonical verdict; status-only or waiting output is not a verdict)");
            (
                AcceptanceResult::MissingVerdict {
                    findings: tail_findings.clone(),
                },
                false,
            )
        }
    };

    let history_findings = match &result {
        AcceptanceResult::Fail { findings } => Some(findings.clone()),
        AcceptanceResult::Continue => Some(crate::acceptance::legacy_findings([
            "Investigation incomplete - continue later",
        ])),
        AcceptanceResult::Stalled { blocker } => {
            let mut evidence = vec![format!(
                "Validated external blocker (category {}): {}",
                blocker.category, blocker.next_action
            )];
            evidence.extend(blocker.evidence.iter().cloned());
            Some(crate::acceptance::legacy_findings(evidence))
        }
        AcceptanceResult::BareBlocker { rejection } => Some(crate::acceptance::legacy_findings([
            BARE_BLOCKER_DIAGNOSTIC.to_string(),
            rejection.reason(),
        ])),
        AcceptanceResult::MalformedFinding { rejection } => {
            Some(crate::acceptance::legacy_findings([
                MALFORMED_FINDING_DIAGNOSTIC.to_string(),
                rejection.reason(),
            ]))
        }
        AcceptanceResult::Pass => None,
        AcceptanceResult::MissingVerdict { findings } => {
            let mut evidence = vec![MISSING_VERDICT_DIAGNOSTIC.to_string()];
            evidence.extend(findings.iter().cloned());
            Some(crate::acceptance::legacy_findings(evidence))
        }
        AcceptanceResult::CommandFailed { .. }
        | AcceptanceResult::PermissionStalled { .. }
        | AcceptanceResult::Cancelled => {
            Some(crate::acceptance::legacy_findings(tail_findings.clone()))
        }
    };
    let attempt_number = agent.next_acceptance_attempt_number(&change.id);
    let attempt = AcceptanceAttempt {
        attempt: attempt_number,
        passed,
        duration: start_time.elapsed(),
        findings: history_findings,
        exit_code: status.code(),
        stdout_tail,
        stderr_tail,
        commit_hash: commit_hash.clone(),
    };
    agent.record_acceptance_attempt(&change.id, attempt);
    match &result {
        AcceptanceResult::Fail { findings } => {
            if !findings.is_empty() {
                agent.record_acceptance_follow_up(&change.id, attempt_number, findings.clone());
            }
        }
        AcceptanceResult::Pass => agent.clear_acceptance_follow_up(&change.id),
        _ => {}
    }
    Ok((result, attempt_number, command))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal validated external blocker, used wherever a canonical stalled
    /// verdict is needed.
    fn sample_blocker() -> crate::acceptance::AcceptanceBlocker {
        crate::acceptance::AcceptanceBlocker {
            category: "pending_verification".to_string(),
            evidence: vec!["managed verification job 42 is still running".to_string()],
            next_action: "wait for job 42 then retry acceptance".to_string(),
            resumable: true,
            prerequisite_owner: None,
            evidence_ids: Vec::new(),
        }
    }

    fn bare_blocker() -> AcceptanceResult {
        AcceptanceResult::BareBlocker {
            rejection: crate::acceptance::BlockerRejection::Missing,
        }
    }

    /// Table-driven contract for the dedicated missing-verdict budget: the
    /// initial attempt plus two retries, then terminal exhaustion.
    #[test]
    fn missing_verdict_budget_allows_two_retries_then_exhausts() {
        let mut state = ProtocolRetryCounter::default();
        let expected = [
            MissingVerdictRetryDecision::Retry(AcceptanceProtocolRetry {
                kind: AcceptanceProtocolError::MissingVerdict,
                attempt: 1,
                max: 2,
            }),
            MissingVerdictRetryDecision::Retry(AcceptanceProtocolRetry {
                kind: AcceptanceProtocolError::MissingVerdict,
                attempt: 2,
                max: 2,
            }),
            MissingVerdictRetryDecision::Exhausted {
                kind: AcceptanceProtocolError::MissingVerdict,
                attempts: 3,
                max: 2,
            },
        ];

        for (index, want) in expected.iter().enumerate() {
            assert_eq!(
                state.record(AcceptanceProtocolError::MissingVerdict),
                *want,
                "consecutive missing verdict #{} must route as {:?}",
                index + 1,
                want
            );
        }
        assert_eq!(MAX_MISSING_VERDICT_RETRIES, 2);
        assert_eq!(
            state.consecutive(),
            3,
            "a fourth protocol retry must never be offered after exhaustion"
        );
        assert!(matches!(
            state.record(AcceptanceProtocolError::MissingVerdict),
            MissingVerdictRetryDecision::Exhausted { .. }
        ));
    }

    #[test]
    fn missing_verdict_canonical_verdict_resets_consecutive_sequence() {
        let mut state = ProtocolRetryCounter::default();
        assert!(matches!(
            state.record(AcceptanceProtocolError::MissingVerdict),
            MissingVerdictRetryDecision::Retry(AcceptanceProtocolRetry { attempt: 1, .. })
        ));
        assert!(matches!(
            state.record(AcceptanceProtocolError::MissingVerdict),
            MissingVerdictRetryDecision::Retry(AcceptanceProtocolRetry { attempt: 2, .. })
        ));

        // PASS/FAIL/CONTINUE/GATED/stalled-hold all reach this reset.
        state.reset();
        assert_eq!(state.consecutive(), 0);

        assert_eq!(
            state.record(AcceptanceProtocolError::MissingVerdict),
            MissingVerdictRetryDecision::Retry(AcceptanceProtocolRetry {
                kind: AcceptanceProtocolError::MissingVerdict,
                attempt: 1,
                max: 2
            }),
            "a later missing verdict must start a fresh protocol-retry sequence"
        );
    }

    /// The protocol budget is fixed and independent from the configured
    /// explicit-`CONTINUE` budget, whatever that budget is set to.
    #[test]
    fn missing_verdict_budget_is_independent_of_configured_continue_count() {
        for configured_continues in [0u32, 1, 5, 50] {
            let mut state = ProtocolRetryCounter::default();
            let mut retries = 0u32;
            loop {
                match state.record(AcceptanceProtocolError::MissingVerdict) {
                    MissingVerdictRetryDecision::Retry(retry) => {
                        assert_eq!(retry.max, MAX_MISSING_VERDICT_RETRIES);
                        retries += 1;
                    }
                    MissingVerdictRetryDecision::Exhausted { attempts, max, .. } => {
                        assert_eq!(attempts, MAX_MISSING_VERDICT_RETRIES + 1);
                        assert_eq!(max, MAX_MISSING_VERDICT_RETRIES);
                        break;
                    }
                }
            }
            assert_eq!(
                retries, MAX_MISSING_VERDICT_RETRIES,
                "configured acceptance_max_continues={configured_continues} must not change the \
                 dedicated missing-verdict budget"
            );
        }
    }

    #[test]
    fn missing_verdict_diagnostics_are_bounded_and_distinguish_progress_from_terminal() {
        let findings = (0..10)
            .map(|index| format!("evidence line {index}"))
            .collect::<Vec<_>>();

        let progress = missing_verdict_retry_progress(
            AcceptanceProtocolRetry {
                kind: AcceptanceProtocolError::MissingVerdict,
                attempt: 1,
                max: 2,
            },
            &findings,
        );
        assert!(progress.contains("protocol retry 1/2"));
        assert!(progress.contains("evidence line 4"));
        assert!(
            !progress.contains("evidence line 5"),
            "evidence must stay bounded to the first five findings, got {progress}"
        );
        assert!(
            !progress.to_ascii_lowercase().contains("protocol failure"),
            "an available retry must not read as a terminal failure: {progress}"
        );

        let terminal = missing_verdict_exhausted_error(3, 2, &findings);
        assert!(terminal.contains("missing-verdict protocol failure"));
        assert!(terminal.contains("Exhausted 3 consecutive attempts after 2 protocol retries"));
        assert!(terminal.contains("evidence line 4"));
        assert!(!terminal.contains("evidence line 5"));

        assert!(
            missing_verdict_exhausted_error(3, 2, &[]).contains("no acceptance output captured"),
            "empty evidence must still produce an actionable diagnostic"
        );
    }

    /// Replay an acceptance verdict sequence through the shared driver exactly
    /// as serial and parallel orchestration do, and report what each invocation
    /// received plus how the sequence terminated.
    fn replay_missing_verdict_sequence(
        sequence: &[AcceptanceResult],
    ) -> (
        Vec<Option<AcceptanceProtocolRetry>>,
        Option<AcceptanceResult>,
        Option<String>,
    ) {
        let mut protocol = AcceptanceProtocolDriver::default();
        let mut received = Vec::new();
        for result in sequence {
            received.push(protocol.take_protocol_retry());
            match result {
                AcceptanceResult::MissingVerdict { findings } => {
                    match protocol.observe_missing_verdict(findings) {
                        MissingVerdictRetryStep::Retry { .. } => continue,
                        MissingVerdictRetryStep::Exhausted { error } => {
                            return (received, None, Some(error))
                        }
                    }
                }
                canonical => {
                    protocol.observe_canonical_verdict();
                    return (received, Some(canonical.clone()), None);
                }
            }
        }
        (received, None, None)
    }

    fn missing_verdict(evidence: &str) -> AcceptanceResult {
        AcceptanceResult::MissingVerdict {
            findings: vec![evidence.to_string()],
        }
    }

    #[test]
    fn missing_verdict_driver_retries_twice_then_accepts_canonical_pass() {
        let (received, canonical, error) = replay_missing_verdict_sequence(&[
            missing_verdict("waiting for verification"),
            missing_verdict("still waiting"),
            AcceptanceResult::Pass,
        ]);

        assert_eq!(received.len(), 3, "acceptance must be invoked three times");
        assert_eq!(
            received,
            vec![
                None,
                Some(AcceptanceProtocolRetry {
                    kind: AcceptanceProtocolError::MissingVerdict,
                    attempt: 1,
                    max: 2
                }),
                Some(AcceptanceProtocolRetry {
                    kind: AcceptanceProtocolError::MissingVerdict,
                    attempt: 2,
                    max: 2
                }),
            ],
            "only the retries may carry a continuation marker"
        );
        assert_eq!(canonical, Some(AcceptanceResult::Pass));
        assert!(
            error.is_none(),
            "a canonical verdict within budget must not produce a terminal error"
        );
    }

    #[test]
    fn missing_verdict_driver_exhausts_after_three_consecutive_missing_verdicts() {
        let (received, canonical, error) = replay_missing_verdict_sequence(&[
            missing_verdict("waiting one"),
            missing_verdict("waiting two"),
            missing_verdict("waiting three"),
            missing_verdict("never reached"),
        ]);

        assert_eq!(
            received.len(),
            3,
            "no fourth protocol retry may start after exhaustion"
        );
        assert!(canonical.is_none());
        let error = error.expect("third consecutive missing verdict must be terminal");
        assert!(error.contains("missing-verdict protocol failure"));
        assert!(error.contains("Exhausted 3 consecutive attempts after 2 protocol retries"));
        assert!(
            error.contains("waiting three"),
            "terminal diagnostic must carry bounded evidence, got {error}"
        );
    }

    /// Every canonical outcome — not just PASS — resets the protocol sequence
    /// and keeps its own routing untouched.
    #[test]
    fn missing_verdict_driver_treats_every_canonical_outcome_as_a_reset() {
        for canonical in [
            AcceptanceResult::Pass,
            AcceptanceResult::Fail {
                findings: vec!["src/lib.rs:1 fix".to_string().into()],
            },
            AcceptanceResult::Continue,
            AcceptanceResult::Stalled {
                blocker: sample_blocker(),
            },
            AcceptanceResult::PermissionStalled {
                blocker: crate::events::StalledBlocker::acceptance_external(
                    "pending_verification",
                    "denied",
                ),
            },
        ] {
            let mut protocol = AcceptanceProtocolDriver::default();
            assert!(matches!(
                protocol.observe_missing_verdict(&["waiting".to_string()]),
                MissingVerdictRetryStep::Retry { .. }
            ));
            assert!(protocol.take_protocol_retry().is_some());

            protocol.observe_canonical_verdict();
            assert_eq!(
                protocol.consecutive_missing_verdicts(),
                0,
                "{canonical:?} must reset the consecutive protocol counter"
            );
            assert!(
                protocol.take_protocol_retry().is_none(),
                "{canonical:?} must clear any pending continuation marker"
            );

            // The next protocol failure starts a fresh full budget.
            assert!(matches!(
                protocol.observe_missing_verdict(&["waiting again".to_string()]),
                MissingVerdictRetryStep::Retry {
                    retry: AcceptanceProtocolRetry { attempt: 1, .. },
                    ..
                }
            ));
        }
    }

    // --- Shared bare-blocker / validated-stall decision API ---

    fn drive_blockers(sequence: &[AcceptanceResult]) -> Vec<AcceptanceBlockerDecision> {
        let mut driver = AcceptanceProtocolDriver::default();
        let mut decisions = Vec::new();
        for result in sequence {
            if let Some(decision) = decide_acceptance_blocker(&mut driver, result) {
                decisions.push(decision);
            } else {
                driver.observe_canonical_verdict();
            }
        }
        decisions
    }

    /// Bare input gets the initial attempt plus exactly two acceptance-only
    /// retries, then a terminal protocol error. Nothing stalls, ever.
    #[test]
    fn bare_blocker_budget_allows_two_retries_then_exhausts() {
        let decisions = drive_blockers(&[bare_blocker(), bare_blocker(), bare_blocker()]);

        assert_eq!(decisions.len(), 3);
        for (index, expected_attempt) in [1u32, 2].iter().enumerate() {
            match &decisions[index] {
                AcceptanceBlockerDecision::ProtocolRetry { retry, progress } => {
                    assert_eq!(retry.kind, AcceptanceProtocolError::BareBlocker);
                    assert_eq!(retry.attempt, *expected_attempt);
                    assert_eq!(retry.max, MAX_ACCEPTANCE_PROTOCOL_RETRIES);
                    assert!(progress.contains("no validated blocker"), "{progress}");
                    assert!(
                        progress.contains("retrying acceptance")
                            && progress.contains(&format!("protocol retry {expected_attempt}/2")),
                        "an available retry must read as progress: {progress}"
                    );
                    assert!(
                        !progress.contains("Exhausted"),
                        "an available retry must be distinguishable from the terminal \
                         diagnostic: {progress}"
                    );
                }
                other => panic!("attempt {expected_attempt} must retry, got {other:?}"),
            }
        }
        match &decisions[2] {
            AcceptanceBlockerDecision::ProtocolExhausted { error } => {
                assert!(error.contains("bare-blocker protocol failure"), "{error}");
                assert!(
                    error.contains("Exhausted 3 consecutive attempts after 2 protocol retries"),
                    "{error}"
                );
            }
            other => panic!("third consecutive bare blocker must be terminal, got {other:?}"),
        }
    }

    /// Every canonical outcome — including a validated stall — resets the
    /// bare-blocker sequence.
    #[test]
    fn canonical_verdict_resets_the_bare_blocker_sequence() {
        for canonical in [
            AcceptanceResult::Pass,
            AcceptanceResult::Fail {
                findings: vec!["src/lib.rs:1 fix".to_string().into()],
            },
            AcceptanceResult::Continue,
            AcceptanceResult::Stalled {
                blocker: sample_blocker(),
            },
        ] {
            let mut driver = AcceptanceProtocolDriver::default();
            for _ in 0..2 {
                assert!(matches!(
                    decide_acceptance_blocker(&mut driver, &bare_blocker()),
                    Some(AcceptanceBlockerDecision::ProtocolRetry { .. })
                ));
            }
            assert_eq!(driver.consecutive_bare_blockers(), 2);

            if decide_acceptance_blocker(&mut driver, &canonical).is_none() {
                driver.observe_canonical_verdict();
            }
            assert_eq!(
                driver.consecutive_bare_blockers(),
                0,
                "{canonical:?} must reset the consecutive bare-blocker counter"
            );
            assert!(
                driver.take_protocol_retry().is_none(),
                "{canonical:?} must clear any pending continuation marker"
            );

            assert!(
                matches!(
                    decide_acceptance_blocker(&mut driver, &bare_blocker()),
                    Some(AcceptanceBlockerDecision::ProtocolRetry {
                        retry: AcceptanceProtocolRetry { attempt: 1, .. },
                        ..
                    })
                ),
                "a later bare blocker must start a fresh full budget"
            );
        }
    }

    /// The two protocol contracts keep strictly independent counters, so a run
    /// cannot alternate between them to buy unbounded retries.
    #[test]
    fn bare_blocker_and_missing_verdict_budgets_are_independent_but_both_bounded() {
        let mut driver = AcceptanceProtocolDriver::default();

        assert!(matches!(
            decide_acceptance_blocker(&mut driver, &bare_blocker()),
            Some(AcceptanceBlockerDecision::ProtocolRetry { .. })
        ));
        assert!(matches!(
            driver.observe_missing_verdict(&["waiting".to_string()]),
            MissingVerdictRetryStep::Retry { .. }
        ));
        assert_eq!(driver.consecutive_bare_blockers(), 1);
        assert_eq!(driver.consecutive_missing_verdicts(), 1);

        // Neither protocol failure resets the other, so alternating still
        // exhausts both budgets rather than renewing them.
        assert!(matches!(
            decide_acceptance_blocker(&mut driver, &bare_blocker()),
            Some(AcceptanceBlockerDecision::ProtocolRetry { .. })
        ));
        assert!(matches!(
            decide_acceptance_blocker(&mut driver, &bare_blocker()),
            Some(AcceptanceBlockerDecision::ProtocolExhausted { .. })
        ));
        assert!(matches!(
            driver.observe_missing_verdict(&["waiting".to_string()]),
            MissingVerdictRetryStep::Retry { .. }
        ));
        assert!(matches!(
            driver.observe_missing_verdict(&["waiting".to_string()]),
            MissingVerdictRetryStep::Exhausted { .. }
        ));
    }

    /// Every structural rejection routes to the same bounded protocol path and
    /// never to a stall, and the diagnostic names the actual defect.
    #[test]
    fn every_invalid_blocker_payload_routes_to_bounded_protocol_retry() {
        use crate::acceptance::BlockerRejection;

        let rejections = [
            BlockerRejection::Missing,
            BlockerRejection::NotAnObject,
            BlockerRejection::MissingCategory,
            BlockerRejection::UnsupportedCategory("flaky_test".to_string()),
            BlockerRejection::EmptyEvidence,
            BlockerRejection::MissingNextAction,
            BlockerRejection::MissingResumable,
        ];

        for rejection in rejections {
            let mut driver = AcceptanceProtocolDriver::default();
            match decide_acceptance_blocker(
                &mut driver,
                &AcceptanceResult::BareBlocker {
                    rejection: rejection.clone(),
                },
            ) {
                Some(AcceptanceBlockerDecision::ProtocolRetry { progress, .. }) => {
                    assert!(
                        progress.contains(&rejection.reason()),
                        "diagnostic must name the defect ({rejection:?}): {progress}"
                    );
                }
                other => panic!("{rejection:?} must retry, got {other:?}"),
            }
        }
    }

    /// A validated blocker becomes a stall with its explicit category intact,
    /// and prose in the evidence never changes that category.
    #[test]
    fn validated_blocker_stalls_with_its_explicit_category_preserved() {
        let blocker = crate::acceptance::AcceptanceBlocker {
            category: "human_decision".to_string(),
            evidence: vec!["the credential token auth story needs an owner decision".to_string()],
            next_action: "owner decides the approach".to_string(),
            resumable: false,
            prerequisite_owner: Some("architecture".to_string()),
            evidence_ids: vec!["adr-7".to_string()],
        };
        let mut driver = AcceptanceProtocolDriver::default();

        match decide_acceptance_blocker(
            &mut driver,
            &AcceptanceResult::Stalled {
                blocker: blocker.clone(),
            },
        ) {
            Some(AcceptanceBlockerDecision::Stall { blocker: stalled }) => {
                assert_eq!(stalled, blocker);
                assert_eq!(
                    stalled.category, "human_decision",
                    "credential/token/auth prose must not override the explicit category"
                );
            }
            other => panic!("a validated blocker must stall, got {other:?}"),
        }
    }

    /// The decision API owns only blocker-bearing results and leaves every other
    /// verdict's routing untouched.
    #[test]
    fn blocker_decision_api_ignores_non_blocker_results() {
        let mut driver = AcceptanceProtocolDriver::default();
        for result in [
            AcceptanceResult::Pass,
            AcceptanceResult::Fail {
                findings: Vec::new(),
            },
            AcceptanceResult::Continue,
            AcceptanceResult::MissingVerdict {
                findings: Vec::new(),
            },
            AcceptanceResult::CommandFailed {
                error: "exit 1".to_string(),
                findings: Vec::new(),
            },
            AcceptanceResult::Cancelled,
        ] {
            assert!(
                decide_acceptance_blocker(&mut driver, &result).is_none(),
                "{result:?} must keep its own routing"
            );
        }
    }

    /// Serial and parallel drive the same API, so identical observation
    /// sequences must produce identical decisions.
    #[test]
    fn bare_blocker_decisions_have_serial_and_parallel_parity() {
        let sequences: [Vec<AcceptanceResult>; 3] = [
            vec![bare_blocker(), bare_blocker(), AcceptanceResult::Pass],
            vec![bare_blocker(), bare_blocker(), bare_blocker()],
            vec![
                bare_blocker(),
                AcceptanceResult::Stalled {
                    blocker: sample_blocker(),
                },
                bare_blocker(),
            ],
        ];

        for sequence in sequences {
            let serial = drive_blockers(&sequence);
            let parallel = drive_blockers(&sequence);
            assert_eq!(
                serial, parallel,
                "equivalent observations must produce equivalent decisions for {sequence:?}"
            );
        }
    }

    /// Routing matrix: only canonical verdicts reset the protocol sequence,
    /// only a missing verdict may consume it, and command failure or
    /// cancellation is never reclassified as a missing-verdict retry.
    #[test]
    fn acceptance_routing_matrix_keeps_missing_verdict_distinct() {
        let cases: [(AcceptanceResult, bool, bool); 8] = [
            (AcceptanceResult::Pass, true, false),
            (
                AcceptanceResult::Fail {
                    findings: vec!["src/lib.rs:1 fix".to_string().into()],
                },
                true,
                false,
            ),
            (AcceptanceResult::Continue, true, false),
            (
                AcceptanceResult::Stalled {
                    blocker: sample_blocker(),
                },
                true,
                false,
            ),
            (
                AcceptanceResult::PermissionStalled {
                    blocker: crate::events::StalledBlocker::acceptance_external(
                        "pending_verification",
                        "denied",
                    ),
                },
                true,
                false,
            ),
            (missing_verdict("waiting"), false, true),
            (
                AcceptanceResult::CommandFailed {
                    error: "exit code 1".to_string(),
                    findings: vec!["boom".to_string()],
                },
                false,
                false,
            ),
            (AcceptanceResult::Cancelled, false, false),
        ];

        for (result, canonical, missing) in cases {
            assert_eq!(
                result.is_canonical_verdict(),
                canonical,
                "{result:?} canonical-verdict classification"
            );
            assert_eq!(
                matches!(result, AcceptanceResult::MissingVerdict { .. }),
                missing,
                "{result:?} must not be confused with a missing verdict"
            );
            assert!(
                !(canonical && missing),
                "{result:?} cannot be both canonical and a protocol failure"
            );
        }

        // Command failure keeps its own routing: it neither resets nor consumes
        // the protocol budget, so a later missing verdict still gets a full one.
        let mut protocol = AcceptanceProtocolDriver::default();
        for result in [
            AcceptanceResult::CommandFailed {
                error: "exit code 1".to_string(),
                findings: Vec::new(),
            },
            AcceptanceResult::Cancelled,
        ] {
            assert!(!result.is_canonical_verdict());
            assert!(protocol.take_protocol_retry().is_none());
            assert_eq!(protocol.consecutive_missing_verdicts(), 0);
        }
        assert!(matches!(
            protocol.observe_missing_verdict(&["waiting".to_string()]),
            MissingVerdictRetryStep::Retry {
                retry: AcceptanceProtocolRetry {
                    kind: AcceptanceProtocolError::MissingVerdict,
                    attempt: 1,
                    max: 2
                },
                ..
            }
        ));
    }

    /// Serial and parallel call the same driver; equivalent observations must
    /// produce equivalent routing.
    #[test]
    fn missing_verdict_driver_has_serial_and_parallel_routing_parity() {
        let sequence = [
            missing_verdict("waiting"),
            missing_verdict("waiting"),
            AcceptanceResult::Fail {
                findings: vec!["src/lib.rs:1 missing coverage".to_string().into()],
            },
        ];

        let serial = replay_missing_verdict_sequence(&sequence);
        let parallel = replay_missing_verdict_sequence(&sequence);
        assert_eq!(serial, parallel);
        assert_eq!(serial.0.len(), 3);
        assert!(matches!(serial.1, Some(AcceptanceResult::Fail { .. })));
        assert!(serial.2.is_none());
    }

    #[test]
    fn semantic_fingerprint_excludes_runtime_bookkeeping() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("src")).unwrap();
        std::fs::write(temp.path().join("src/lib.rs"), "one").unwrap();
        let before = semantic_progress_fingerprint(temp.path()).unwrap();
        std::fs::create_dir_all(temp.path().join(".cflx")).unwrap();
        std::fs::write(temp.path().join(".cflx/runtime.json"), "runtime").unwrap();
        assert_eq!(before, semantic_progress_fingerprint(temp.path()).unwrap());
        std::fs::write(temp.path().join("src/lib.rs"), "two").unwrap();
        assert_ne!(before, semantic_progress_fingerprint(temp.path()).unwrap());
    }

    #[test]
    fn finding_identity_prefers_code_and_uses_structural_fallback() {
        let coded = normalize_findings(&[
            "[MISSING_RETRY_TEST] old evidence at src/run.rs:10".into(),
            "[MISSING_RETRY_TEST] changed summary at tests/run.rs:99".into(),
        ]);
        assert_eq!(coded.len(), 1);
        assert_eq!(coded[0].identity, "repository|code|[missing_retry_test]");

        let changed_detail = normalize_findings(&[
            "Missing retry test at src/run.rs:10 because the branch is uncovered".into(),
            "Regression coverage absent in src/run.rs:77; add a focused test".into(),
        ]);
        assert_eq!(changed_detail.len(), 1);
        assert_eq!(
            changed_detail[0].identity,
            "repository|src/run.rs|verification"
        );

        let distinct = normalize_findings(&[
            "Missing test at src/run.rs:10".into(),
            "Incorrect implementation at src/run.rs:11".into(),
            "Missing test at src/other.rs:10".into(),
        ]);
        assert_eq!(
            distinct.len(),
            3,
            "rule and location must prevent collisions"
        );
    }

    #[test]
    fn retry_decision_normalizes_order_whitespace_duplicates_and_stalls_repeats() {
        let findings = normalize_findings(&[
            " src/lib.rs:10   missing  test ".to_string().into(),
            "src/lib.rs:11 missing test".to_string().into(),
        ]);
        assert_eq!(findings.len(), 1);
        let decision = decide_acceptance_retry(
            &[findings[0].identity.clone()],
            Some("unchanged"),
            &findings,
            "unchanged",
            2,
        );
        assert!(matches!(
            decision,
            AcceptanceRetryDecision::Stall {
                reason: "repeated_acceptance_findings",
                ..
            }
        ));
    }

    #[test]
    fn retry_decision_stalls_external_only_and_allows_progress_changed() {
        let findings = normalize_findings(&["external service outage".to_string().into()]);
        assert!(findings[0].external);
        assert!(matches!(
            decide_acceptance_retry(&[], None, &findings, "one", 1),
            AcceptanceRetryDecision::Stall {
                reason: "external_acceptance_blocker",
                ..
            }
        ));
        assert!(
            matches!(decide_acceptance_retry(&[], None, &findings, "one", MAX_ACCEPTANCE_RETRY_CYCLES), AcceptanceRetryDecision::Stall { reason: "acceptance_cycle_limit_exhausted", external_blockers } if external_blockers.len() == 1)
        );
    }

    #[test]
    fn semantic_fingerprint_tracks_change_specs_and_jsonc_but_excludes_runtime_follow_up() {
        let temp = tempfile::TempDir::new().unwrap();
        let tasks = temp.path().join("openspec/changes/example/tasks.md");
        let spec = temp
            .path()
            .join("openspec/changes/example/specs/runtime/spec.md");
        std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
        std::fs::write(&tasks, "## Implementation Tasks\n- [x] work\n").unwrap();
        std::fs::write(&spec, "requirement one").unwrap();
        std::fs::write(temp.path().join(".cflx.jsonc"), "{ \"mode\": 1 }").unwrap();
        let before = semantic_progress_fingerprint(temp.path()).unwrap();
        std::fs::write(&tasks, "## Implementation Tasks\n- [x] work\n\n## Acceptance #2 Failure Follow-up\n- [ ] runtime finding\n").unwrap();
        assert_eq!(before, semantic_progress_fingerprint(temp.path()).unwrap());
        std::fs::write(&spec, "requirement two").unwrap();
        assert_ne!(before, semantic_progress_fingerprint(temp.path()).unwrap());
        std::fs::write(temp.path().join(".cflx.jsonc"), "{ \"mode\": 2 }").unwrap();
        assert_ne!(before, semantic_progress_fingerprint(temp.path()).unwrap());
    }

    #[test]
    fn generic_credential_and_unavailable_errors_remain_repository_fixable() {
        let findings = normalize_findings(&[
            "missing API key in test fixture".to_string().into(),
            "src/client.rs: rate limit retry missing".to_string().into(),
            "network unreachable: fix retry handling".to_string().into(),
            "dns resolution failed while repairing src/client.rs"
                .to_string()
                .into(),
            "missing non-mockable external credential"
                .to_string()
                .into(),
        ]);
        assert_eq!(
            findings.iter().filter(|finding| finding.external).count(),
            1
        );
        assert!(findings[0].identity.starts_with("external|"));
        assert_eq!(
            repository_findings(&[
                "missing API key in test fixture".to_string().into(),
                "src/client.rs: rate limit retry missing".to_string().into(),
                "network unreachable: fix retry handling".to_string().into(),
                "dns resolution failed while repairing src/client.rs"
                    .to_string()
                    .into(),
                "missing non-mockable external credential"
                    .to_string()
                    .into(),
            ])
            .len(),
            4
        );
    }

    #[test]
    fn alternating_continue_and_fail_keeps_fail_retry_history_deterministic() {
        let findings = normalize_findings(&["src/lib.rs:10 missing regression coverage".into()]);
        let identities = findings
            .iter()
            .map(|finding| finding.identity.clone())
            .collect::<Vec<_>>();

        // CONTINUE never enters the FAIL retry decision; the first later FAIL
        // remains the repair opportunity and the repeated FAIL then stalls.
        assert!(matches!(
            decide_acceptance_retry(&[], None, &findings, "unchanged", 1),
            AcceptanceRetryDecision::Retry {
                reason: "first_acceptance_failure"
            }
        ));
        assert!(matches!(
            decide_acceptance_retry(&identities, Some("unchanged"), &findings, "unchanged", 2),
            AcceptanceRetryDecision::Stall {
                reason: "repeated_acceptance_findings",
                ..
            }
        ));
    }

    #[test]
    fn serial_and_parallel_same_inputs_have_retry_outcome_parity() {
        let findings = normalize_findings(&[
            "src/lib.rs:10 missing regression coverage".into(),
            "external non-mockable prerequisite unavailable".into(),
        ]);
        let previous = findings
            .iter()
            .map(|finding| finding.identity.clone())
            .collect::<Vec<_>>();

        // Both execution modes call this shared pure decision with checkpoint
        // state. Keep an explicit parity fixture for their common boundary.
        let serial = decide_acceptance_retry(&previous, Some("same"), &findings, "same", 2);
        let parallel = decide_acceptance_retry(&previous, Some("same"), &findings, "same", 2);
        assert_eq!(serial, parallel);
        assert!(matches!(
            serial,
            AcceptanceRetryDecision::Stall {
                reason: "repeated_acceptance_findings",
                ref external_blockers
            } if external_blockers.len() == 1
        ));
    }

    #[test]
    fn retry_decision_handles_mixed_and_findingless_failures() {
        let mixed = normalize_findings(&[
            "src/lib.rs:1 fix test".into(),
            "external service outage".into(),
        ]);
        assert!(matches!(
            decide_acceptance_retry(&[], None, &mixed, "one", 1),
            AcceptanceRetryDecision::Retry { .. }
        ));
        assert!(matches!(
            decide_acceptance_retry(&[], None, &[], "one", 1),
            AcceptanceRetryDecision::Retry { .. }
        ));
        assert_eq!(
            repository_findings(&[
                "src/lib.rs:1 fix test".into(),
                "external service outage".into(),
            ]),
            vec!["src/lib.rs:1 fix test"]
        );
    }

    #[test]
    fn test_build_acceptance_tail_findings_prefers_stdout() {
        let findings = build_acceptance_tail_findings(
            Some("stdout line 1\nstdout line 2".to_string()),
            Some("stderr line".to_string()),
        );

        assert_eq!(findings, vec!["stdout line 1", "stdout line 2"]);
    }

    #[test]
    fn test_build_acceptance_tail_findings_falls_back_to_stderr() {
        let findings =
            build_acceptance_tail_findings(Some("  ".to_string()), Some("stderr".to_string()));

        assert_eq!(findings, vec!["stderr"]);
    }

    #[test]
    fn test_build_acceptance_tail_findings_fallback_message() {
        let findings = build_acceptance_tail_findings(None, Some("\n\n".to_string()));

        assert_eq!(findings, vec!["No acceptance output captured"]);
    }

    #[test]
    fn test_acceptance_result_is_pass() {
        assert!(AcceptanceResult::Pass.is_pass());
        assert!(!AcceptanceResult::Fail {
            findings: vec!["error".to_string().into()]
        }
        .is_pass());
        assert!(!AcceptanceResult::CommandFailed {
            error: "test".to_string(),
            findings: vec!["failure".to_string()],
        }
        .is_pass());
        assert!(!AcceptanceResult::PermissionStalled {
            blocker: crate::events::StalledBlocker::acceptance_external(
                "pending_verification",
                "permission denied"
            ),
        }
        .is_pass());
        assert!(!AcceptanceResult::MissingVerdict {
            findings: vec!["status-only output".to_string()],
        }
        .is_pass());
        assert!(!AcceptanceResult::Cancelled.is_pass());
        assert!(!AcceptanceResult::Stalled {
            blocker: sample_blocker()
        }
        .is_pass());
    }

    #[test]
    fn test_build_acceptance_tail_findings_filters_acceptance_marker() {
        let findings = build_acceptance_tail_findings(
            Some("line 1\nACCEPTANCE: FAIL\nline 2".to_string()),
            None,
        );

        assert_eq!(findings, vec!["line 1", "line 2"]);
    }

    #[test]
    fn test_build_acceptance_tail_findings_filters_findings_line() {
        let findings = build_acceptance_tail_findings(
            Some("error 1\nFINDINGS:\n- item 1\n- item 2".to_string()),
            None,
        );

        assert_eq!(findings, vec!["error 1", "- item 1", "- item 2"]);
    }

    #[test]
    fn test_build_acceptance_tail_findings_filters_both_markers() {
        let findings = build_acceptance_tail_findings(
            Some("ACCEPTANCE: FAIL\nFINDINGS:\nactual error\nanother line".to_string()),
            None,
        );

        assert_eq!(findings, vec!["actual error", "another line"]);
    }

    // Characterization tests: document the difference between tail_findings
    // and parse_acceptance_output findings so the refactor is clearly motivated.

    #[test]
    fn test_tail_findings_includes_preamble_parse_does_not() {
        // tail_findings includes all non-marker lines (preamble, postamble, items).
        // parse_acceptance_output findings includes only FINDINGS section items.
        // The refactor unifies the FAIL path to use parse_acceptance_output findings.
        let stdout = "preamble\nACCEPTANCE: FAIL\nFINDINGS:\n- Finding 1\n- Finding 2\npostamble"
            .to_string();

        let tail = build_acceptance_tail_findings(Some(stdout.clone()), None);
        // tail includes preamble, finding items, postamble
        assert!(tail.iter().any(|l| l.contains("preamble")));
        assert!(tail.iter().any(|l| l.contains("postamble")));
        assert!(tail.iter().any(|l| l.contains("Finding 1")));

        // parse_acceptance_output returns only the FINDINGS section items
        match crate::acceptance::parse_acceptance_output(&stdout) {
            crate::acceptance::AcceptanceResult::Fail { findings } => {
                assert_eq!(findings, vec!["Finding 1", "Finding 2"]);
                assert!(!findings.iter().any(|f| f.contains("preamble")));
                assert!(!findings.iter().any(|f| f.contains("postamble")));
            }
            _ => panic!("Expected Fail"),
        }
    }

    #[test]
    fn test_parse_findings_is_preferred_source_for_fail_result() {
        // After the refactor: for AcceptanceResult::Fail, findings come from
        // parse_acceptance_output (FINDINGS section), not from build_acceptance_tail_findings.
        let stdout =
            "ACCEPTANCE: FAIL\nFINDINGS:\n- src/foo.rs:10 issue A\n- src/bar.rs:5 issue B\n"
                .to_string();

        match crate::acceptance::parse_acceptance_output(&stdout) {
            crate::acceptance::AcceptanceResult::Fail { findings } => {
                assert_eq!(findings.len(), 2);
                assert_eq!(findings[0], "src/foo.rs:10 issue A");
                assert_eq!(findings[1], "src/bar.rs:5 issue B");
            }
            _ => panic!("Expected Fail"),
        }
    }
    // --- Remediation-to-diff coverage ---

    use crate::acceptance::AcceptanceFinding;

    fn finding(id: &str, implementation: &str, verification: &str) -> AcceptanceFinding {
        AcceptanceFinding::structured(crate::acceptance::RepositoryFinding {
            id: id.to_string(),
            severity: crate::acceptance::FindingSeverity::Minor,
            summary: "Challenge and proof leakage is not tested by value".to_string(),
            evidence: vec!["relay exposes counts but not issued values".to_string()],
            required_changes: vec![crate::acceptance::FindingFileExpectation {
                file: implementation.to_string(),
                description: "Expose issued challenge and presented proof values".to_string(),
            }],
            verification: vec![crate::acceptance::FindingFileExpectation {
                file: verification.to_string(),
                description: "Assert recorded values are absent from audit output".to_string(),
            }],
        })
    }

    fn secret_value_finding() -> AcceptanceFinding {
        finding(
            "acceptance-secret-value-scan",
            "tests/support/relay.ts",
            "runtime/recovery.integration.test.ts",
        )
    }

    fn changed(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|path| path.to_string()).collect()
    }

    #[test]
    fn complete_coverage_permits_semantic_review_without_claiming_resolution() {
        let findings = [secret_value_finding()];
        let coverage = evaluate_remediation_coverage(
            &findings,
            &changed(&[
                "tests/support/relay.ts",
                "runtime/recovery.integration.test.ts",
            ]),
        );

        assert!(coverage.is_complete());
        assert!(coverage.uncovered().is_empty());
        assert!(coverage.unrelated_files.is_empty());
        assert_eq!(
            decide_repair_gate(
                "change-a",
                &findings,
                &FindingRepairLedger::default(),
                Some("fail-rev"),
                Some("apply-rev"),
                &changed(&[
                    "tests/support/relay.ts",
                    "runtime/recovery.integration.test.ts",
                ]),
                &[],
            ),
            RepairGateDecision::Proceed
        );
    }

    #[test]
    fn missing_implementation_or_verification_path_fails_coverage() {
        let findings = [secret_value_finding()];

        let missing_implementation = evaluate_remediation_coverage(
            &findings,
            &changed(&["runtime/recovery.integration.test.ts"]),
        );
        assert!(!missing_implementation.is_complete());
        assert_eq!(
            missing_implementation.missing_required,
            [(
                "acceptance-secret-value-scan".to_string(),
                "tests/support/relay.ts".to_string()
            )]
        );
        assert!(missing_implementation.missing_verification.is_empty());

        let missing_verification =
            evaluate_remediation_coverage(&findings, &changed(&["tests/support/relay.ts"]));
        assert!(!missing_verification.is_complete());
        assert_eq!(
            missing_verification.missing_verification,
            [(
                "acceptance-secret-value-scan".to_string(),
                "runtime/recovery.integration.test.ts".to_string()
            )]
        );
    }

    #[test]
    fn calibration_only_change_stops_before_acceptance_with_evidence() {
        // Regression shape: Apply touched only a calibration test and a comment.
        let findings = [secret_value_finding()];
        let delta = changed(&["tests/calibration.test.ts", "src/unrelated.rs"]);

        let RepairGateDecision::Stop(stop) = decide_repair_gate(
            "change-a",
            &findings,
            &FindingRepairLedger::default(),
            Some("fail-rev"),
            Some("apply-rev"),
            &delta,
            &["adjusted calibration threshold".to_string()],
        ) else {
            panic!("calibration-only repair must not authorize another acceptance run");
        };

        assert_eq!(stop.reason, REMEDIATION_MISMATCH_REASON);
        assert!(stop.resumable);
        assert_eq!(stop.fail_revision.as_deref(), Some("fail-rev"));
        assert_eq!(stop.apply_revision.as_deref(), Some("apply-rev"));
        assert_eq!(stop.required_files, ["tests/support/relay.ts"]);
        assert_eq!(
            stop.verification_files,
            ["runtime/recovery.integration.test.ts"]
        );
        assert_eq!(
            stop.coverage.unrelated_files,
            ["src/unrelated.rs", "tests/calibration.test.ts"],
            "every changed file is retained as a diagnostic, sorted"
        );
        assert_eq!(stop.coverage.changed_files.len(), delta.len());
        assert_eq!(
            stop.remediation_evidence,
            ["adjusted calibration threshold"]
        );
        // The complete finding is retained for the operator.
        assert_eq!(stop.findings, findings);

        let json = stop.to_json();
        assert_eq!(json["coverage_complete"], false);
        assert_eq!(json["proves_acceptance_pass"], false);
        assert_eq!(json["proves_archive_readiness"], false);
        assert_eq!(json["proves_completion"], false);
    }

    #[test]
    fn coverage_normalizes_renames_and_untracked_paths() {
        let findings = [secret_value_finding()];
        // A rename entry from `--name-status` counts both sides as touched, and
        // `./`-prefixed untracked entries normalize to the same repository path.
        let coverage = evaluate_remediation_coverage(
            &findings,
            &changed(&[
                "tests/support/old-relay.ts -> tests/support/relay.ts",
                "./runtime/recovery.integration.test.ts",
            ]),
        );

        assert!(coverage.is_complete(), "{coverage:?}");
        assert_eq!(
            coverage.unrelated_files,
            ["tests/support/old-relay.ts".to_string()]
        );
    }

    #[test]
    fn legacy_findings_without_declared_paths_keep_compatibility_behavior() {
        let findings = crate::acceptance::legacy_findings(["src/a.rs:10 missing coverage"]);
        let coverage = evaluate_remediation_coverage(&findings, &changed(&["docs/readme.md"]));

        assert!(
            coverage.is_complete(),
            "a legacy finding declares no path set, so strict coverage cannot apply"
        );
        assert_eq!(
            coverage.legacy_finding_texts,
            ["src/a.rs:10 missing coverage"]
        );
        assert_eq!(
            decide_repair_gate(
                "change-a",
                &findings,
                &FindingRepairLedger::default(),
                None,
                None,
                &changed(&["docs/readme.md"]),
                &[],
            ),
            RepairGateDecision::Proceed
        );
    }

    #[test]
    fn empty_repair_delta_cannot_satisfy_a_declared_finding() {
        let findings = [secret_value_finding()];
        assert!(matches!(
            decide_repair_gate(
                "change-a",
                &findings,
                &FindingRepairLedger::default(),
                Some("fail-rev"),
                Some("fail-rev"),
                &[],
                &[],
            ),
            RepairGateDecision::Stop(_)
        ));
    }

    // --- Per-finding automatic repair budget ---

    fn observe(
        ledger: &mut FindingRepairLedger,
        findings: &[AcceptanceFinding],
    ) -> FindingRepairDecision {
        ledger.observe_fail(&normalize_findings(findings))
    }

    #[test]
    fn first_observation_grants_one_repair_opportunity() {
        let mut ledger = FindingRepairLedger::default();
        let findings = [secret_value_finding()];

        let FindingRepairDecision::Repair { identities } = observe(&mut ledger, &findings) else {
            panic!("first observation must allow one repair");
        };
        assert_eq!(identities, ["repository|id|acceptance-secret-value-scan"]);
        assert_eq!(
            ledger.occurrences("repository|id|acceptance-secret-value-scan"),
            1
        );
        assert!(!ledger.has_consumed_repair("repository|id|acceptance-secret-value-scan"));

        ledger.record_repair_dispatched(&identities);
        assert!(ledger.has_consumed_repair("repository|id|acceptance-secret-value-scan"));
    }

    #[test]
    fn repeated_id_stops_before_a_second_repair_regardless_of_progress() {
        let mut ledger = FindingRepairLedger::default();
        let findings = [secret_value_finding()];
        let FindingRepairDecision::Repair { identities } = observe(&mut ledger, &findings) else {
            panic!("first observation must allow one repair");
        };
        ledger.record_repair_dispatched(&identities);

        // The same defect, reported with changed prose, changed evidence, a
        // changed line number, and a different cited path.
        let restated = AcceptanceFinding::structured(crate::acceptance::RepositoryFinding {
            id: "acceptance-secret-value-scan".to_string(),
            severity: crate::acceptance::FindingSeverity::Major,
            summary: "Rewritten summary at a new line".to_string(),
            evidence: vec!["different evidence at src/other.rs:99".to_string()],
            required_changes: vec![crate::acceptance::FindingFileExpectation {
                file: "src/other.rs".to_string(),
                description: "different described change".to_string(),
            }],
            verification: vec![crate::acceptance::FindingFileExpectation {
                file: "tests/other.rs".to_string(),
                description: "different described proof".to_string(),
            }],
        });

        let FindingRepairDecision::Stop {
            reason,
            repeated_identities,
        } = observe(&mut ledger, std::slice::from_ref(&restated))
        else {
            panic!("a repeated ID must stop automatic repair");
        };
        assert_eq!(reason, REPEATED_FINDING_REASON);
        assert_eq!(
            repeated_identities,
            ["repository|id|acceptance-secret-value-scan"]
        );
        assert_eq!(
            ledger.occurrences("repository|id|acceptance-secret-value-scan"),
            2
        );

        // Unrelated semantic progress does not change the decision, and the stop
        // record retains the complete finding.
        let stop = repeated_finding_stop(
            "change-a",
            &[restated],
            &ledger,
            repeated_identities,
            Some("fail-rev"),
            Some("apply-rev"),
            &changed(&["src/other.rs", "tests/other.rs", "docs/notes.md"]),
            &["claimed a repair".to_string()],
        );
        assert_eq!(stop.reason, REPEATED_FINDING_REASON);
        assert!(
            stop.coverage.is_complete(),
            "coverage passing is irrelevant"
        );
        assert_eq!(
            stop.occurrences,
            [("repository|id|acceptance-secret-value-scan".to_string(), 2)]
        );
        assert!(stop.resumable);
        assert_eq!(stop.remediation_evidence, ["claimed a repair"]);
    }

    #[test]
    fn a_new_finding_id_receives_its_own_repair_opportunity() {
        let mut ledger = FindingRepairLedger::default();
        let first = [secret_value_finding()];
        let FindingRepairDecision::Repair { identities } = observe(&mut ledger, &first) else {
            panic!("first observation must allow one repair");
        };
        ledger.record_repair_dispatched(&identities);

        // Acceptance closed the prior ID and reported a genuinely new one.
        let second = [finding(
            "acceptance-new-defect",
            "src/new.rs",
            "tests/new.rs",
        )];
        let FindingRepairDecision::Repair { identities } = observe(&mut ledger, &second) else {
            panic!("a new ID gets its own opportunity");
        };
        assert_eq!(identities, ["repository|id|acceptance-new-defect"]);
        assert_eq!(
            ledger.occurrences("repository|id|acceptance-secret-value-scan"),
            0,
            "an ID acceptance stopped reporting is closed, not carried forward"
        );
    }

    #[test]
    fn mixed_repeated_and_new_ids_stop_atomically_with_every_finding_retained() {
        let mut ledger = FindingRepairLedger::default();
        let repeated = secret_value_finding();
        let FindingRepairDecision::Repair { identities } =
            observe(&mut ledger, std::slice::from_ref(&repeated))
        else {
            panic!("first observation must allow one repair");
        };
        ledger.record_repair_dispatched(&identities);

        let fresh = finding("acceptance-new-defect", "src/new.rs", "tests/new.rs");
        let mixed = [repeated, fresh];
        let FindingRepairDecision::Stop {
            reason,
            repeated_identities,
        } = observe(&mut ledger, &mixed)
        else {
            panic!("a mixed payload containing a repeated ID must stop atomically");
        };
        assert_eq!(reason, REPEATED_FINDING_REASON);
        assert_eq!(
            repeated_identities,
            ["repository|id|acceptance-secret-value-scan"],
            "only the repeated ID is the stop reason"
        );

        let stop = repeated_finding_stop(
            "change-a",
            &mixed,
            &ledger,
            repeated_identities,
            None,
            None,
            &[],
            &[],
        );
        assert_eq!(stop.findings.len(), 2, "diagnostics retain every finding");
        assert_eq!(
            stop.occurrence_identities(),
            [
                "repository|id|acceptance-new-defect".to_string(),
                "repository|id|acceptance-secret-value-scan".to_string(),
            ]
        );
    }

    #[test]
    fn explicit_retry_reset_releases_the_automatic_budget_only() {
        let mut ledger = FindingRepairLedger::default();
        let findings = [secret_value_finding()];
        let FindingRepairDecision::Repair { identities } = observe(&mut ledger, &findings) else {
            panic!("first observation must allow one repair");
        };
        ledger.record_repair_dispatched(&identities);

        ledger.reset_for_explicit_retry();

        assert!(!ledger.has_consumed_repair("repository|id|acceptance-secret-value-scan"));
        assert_eq!(
            ledger.occurrences("repository|id|acceptance-secret-value-scan"),
            1,
            "prior occurrence evidence is preserved for review"
        );
        assert!(matches!(
            observe(&mut ledger, &findings),
            FindingRepairDecision::Repair { .. }
        ));
    }

    #[test]
    fn legacy_findings_use_the_fallback_identity_for_the_repair_budget() {
        let mut ledger = FindingRepairLedger::default();
        let findings = crate::acceptance::legacy_findings(["Missing retry test at src/run.rs:10"]);
        let FindingRepairDecision::Repair { identities } = observe(&mut ledger, &findings) else {
            panic!("first observation must allow one repair");
        };
        assert_eq!(identities, ["repository|src/run.rs|verification"]);
        ledger.record_repair_dispatched(&identities);

        // Same defect, different prose and line number: the fallback identity is
        // stable, so the repeated-finding stop still fires.
        let restated =
            crate::acceptance::legacy_findings(["Regression coverage absent in src/run.rs:99"]);
        assert!(matches!(
            observe(&mut ledger, &restated),
            FindingRepairDecision::Stop { .. }
        ));
    }

    #[test]
    fn structured_finding_identity_is_the_reviewer_id_not_its_prose() {
        let first = normalize_findings(&[secret_value_finding()]);
        let restated = normalize_findings(&[AcceptanceFinding::structured(
            crate::acceptance::RepositoryFinding {
                id: "acceptance-secret-value-scan".to_string(),
                severity: crate::acceptance::FindingSeverity::Major,
                summary: "totally different words".to_string(),
                evidence: vec!["totally different evidence".to_string()],
                required_changes: vec![crate::acceptance::FindingFileExpectation {
                    file: "src/elsewhere.rs".to_string(),
                    description: "d".to_string(),
                }],
                verification: vec![crate::acceptance::FindingFileExpectation {
                    file: "tests/elsewhere.rs".to_string(),
                    description: "d".to_string(),
                }],
            },
        )]);

        assert_eq!(first[0].identity, restated[0].identity);
        assert_ne!(
            first[0].text, restated[0].text,
            "identity is stable while the payload stays free to change"
        );
    }
}

//! Native task progress parsing and runtime-owned task mutation.
//!
//! Markdown checkbox parsing lives here — bullet lists (`- [ ]`) and numbered
//! lists (`1. [ ]`) — and is one implementation behind the shared
//! [`crate::task_file`] contract. Which artifact speaks for a change, and in
//! which format, is decided there; this module never constructs a `tasks.md`
//! path of its own.

use crate::error::{OrchestratorError, Result};
use crate::task_file::{
    self, JsonExternalBlocker, JsonFinding, JsonFollowUp, TaskFile, TaskFileFormat,
};
use crate::tui::log_deduplicator;
use regex::Regex;
use std::fmt::Write as _;

/// End-to-end regression coverage for acceptance follow-up recovery.
///
/// Lives in-crate because `task_parser` is intentionally crate-private (see
/// `src/lib.rs`), so a `tests/` integration binary cannot reach it.
#[cfg(test)]
mod recovery_regression;

use std::path::Path;
use std::sync::OnceLock;
use tracing::debug;

/// Task progress information.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskProgress {
    /// Number of completed tasks.
    pub completed: u32,
    /// Total number of tasks.
    pub total: u32,
}

impl TaskProgress {
    /// Create a new TaskProgress with zero counts.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a TaskProgress with specific counts.
    #[cfg(test)]
    pub fn with_counts(completed: u32, total: u32) -> Self {
        Self { completed, total }
    }
}

/// Get the task checkbox regex pattern.
///
/// Pattern matches both bullet and numbered lists with checkboxes:
/// - `- [ ] Task` (bullet unchecked)
/// - `- [x] Task` (bullet checked)
/// - `* [X] Task` (asterisk checked)
/// - `1. [ ] Task` (numbered unchecked)
/// - `10. [x] Task` (numbered checked)
///
/// Does NOT match:
/// - `  - [ ] Sub-item` (indented sub-bullets)
/// - `Some text [ ]` (inline checkboxes)
/// - `## [x] Header` (markdown headers)
fn task_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        // ^: start of line
        // (?:[-*]|\d+\.): bullet (-/*) or numbered (digits followed by .)
        // \s+: one or more whitespace
        // \[([ xX])\]: checkbox with capture group for status
        Regex::new(r"^(?:[-*]|\d+\.)\s+\[([ xX])\]").expect("Invalid regex pattern")
    })
}

/// Parse task progress from markdown content.
///
/// Parses each line looking for task checkboxes at the start of lines.
/// Checkbox-like text inside fenced blocks is inert: recovered acceptance
/// notes store untrusted payloads in fenced literals and must never change
/// completion accounting.
/// Returns the count of completed and total tasks.
/// When change_id is provided, emits deduplicated debug logs.
pub fn parse_content(content: &str, change_id: Option<&str>) -> TaskProgress {
    let regex = task_regex();
    let mut progress = TaskProgress::new();
    let mut fences = FenceTracker::default();

    for line in content.lines() {
        if fences.observe(line) {
            continue;
        }
        if let Some(captures) = regex.captures(line) {
            progress.total += 1;
            // Capture group 1 contains the checkbox status: ' ', 'x', or 'X'
            if let Some(status) = captures.get(1) {
                let status_char = status.as_str();
                if status_char == "x" || status_char == "X" {
                    progress.completed += 1;
                }
            }
        }
    }

    if let Some(change_id) = change_id {
        if log_deduplicator::should_log_task_progress(change_id, progress.completed, progress.total)
        {
            debug!(
                "Parsed task progress: {}/{} tasks completed",
                progress.completed, progress.total
            );
        }
    }

    progress
}

/// Parse task progress from a task artifact path.
///
/// The format is bound from the basename through [`TaskFile::from_path`], so a
/// `tasks.json` path is never parsed with the Markdown reader. An unrecognized
/// basename is refused rather than guessed.
/// When change_id is provided, emits deduplicated debug logs.
///
/// Test-only: production callers resolve a [`TaskFile`] through
/// [`crate::task_file`] and never bind a format from a path they built.
#[cfg(test)]
pub fn parse_file(path: &Path, change_id: Option<&str>) -> Result<TaskProgress> {
    let file = TaskFile::from_path(path).ok_or_else(|| {
        OrchestratorError::ConfigLoad(format!(
            "Unsupported task artifact {:?}: expected {} or {}",
            path,
            task_file::MARKDOWN_FILE_NAME,
            task_file::JSON_FILE_NAME
        ))
    })?;
    task_file::read_progress(&file, change_id)
}

/// Parse task progress for a change by its ID.
///
/// Test-only: every execution path resolves progress against a managed
/// workspace path through the shared resolver, never against the process's
/// current directory.
#[cfg(test)]
pub fn parse_change(change_id: &str) -> Result<TaskProgress> {
    let resolved = task_file::resolve_active(change_id, None)?.ok_or_else(|| {
        OrchestratorError::ConfigLoad(format!(
            "Tasks file not found for change '{}' in openspec/changes/",
            change_id
        ))
    })?;
    task_file::read_progress(&resolved.file, Some(change_id))
}

/// Parse task progress with worktree priority and base tree fallback.
///
/// Resolution order: worktree active entry, then base active entry.
///
/// # Deprecated
/// Use [`parse_progress_with_fallback`] instead, which provides comprehensive
/// fallback order: worktree → archive → base.
#[deprecated(
    since = "0.3.0",
    note = "Use parse_progress_with_fallback for comprehensive fallback order"
)]
#[allow(dead_code)]
pub fn parse_change_with_worktree_fallback(
    change_id: &str,
    worktree_path: Option<&Path>,
) -> Result<TaskProgress> {
    let resolved = task_file::resolve_active(change_id, worktree_path)?.ok_or_else(|| {
        OrchestratorError::ConfigLoad(format!(
            "Tasks file not found for change '{}' in any active location",
            change_id
        ))
    })?;
    debug!(
        "Reading tasks from {}: {:?}",
        resolved.kind.log_label(),
        resolved.file.path
    );
    task_file::read_progress(&resolved.file, Some(change_id))
}

/// Parse task progress from the archive directory of the base tree.
///
/// # Deprecated
/// Use [`parse_progress_with_fallback`] instead, which provides comprehensive
/// fallback order: worktree → archive → base.
#[deprecated(
    since = "0.3.0",
    note = "Use parse_progress_with_fallback for comprehensive fallback order"
)]
#[allow(dead_code)]
pub fn parse_archived_change(change_id: &str) -> Result<TaskProgress> {
    if let Some(message) = task_file::invalid_archive_layout_error(change_id, None) {
        return Err(OrchestratorError::ConfigLoad(message));
    }

    let resolved = task_file::resolve_archived(change_id, None)?.ok_or_else(|| {
        if task_file::find_archive_entry(change_id, None).is_some() {
            OrchestratorError::ConfigLoad(format!(
                "Archived tasks file not found for change '{}' in {:?}",
                change_id,
                Path::new("openspec/changes/archive")
            ))
        } else {
            OrchestratorError::ConfigLoad(format!(
                "Archived directory not found for change '{}' in openspec/changes/archive/",
                change_id
            ))
        }
    })?;

    debug!(
        "Reading tasks from {}: {:?}",
        resolved.kind.log_label(),
        resolved.file.path
    );
    task_file::read_progress(&resolved.file, Some(change_id))
}

/// Parse task progress with worktree priority for archived changes.
///
/// Resolution order: worktree archive entry, worktree active entry, then base
/// archive entry.
///
/// # Deprecated
/// Use [`parse_progress_with_fallback`] instead, which provides comprehensive
/// fallback order: worktree → archive → base.
#[deprecated(
    since = "0.3.0",
    note = "Use parse_progress_with_fallback for comprehensive fallback order"
)]
#[allow(dead_code)]
pub fn parse_archived_change_with_worktree_fallback(
    change_id: &str,
    worktree_path: Option<&Path>,
) -> Result<TaskProgress> {
    if let Some(wt_path) = worktree_path {
        if let Some(message) = task_file::invalid_archive_layout_error(change_id, Some(wt_path)) {
            return Err(OrchestratorError::ConfigLoad(message));
        }
    }
    if let Some(message) = task_file::invalid_archive_layout_error(change_id, None) {
        return Err(OrchestratorError::ConfigLoad(message));
    }

    let resolved = task_file::resolve_archived(change_id, worktree_path)?.ok_or_else(|| {
        OrchestratorError::ConfigLoad(format!(
            "Archived directory not found for change '{}' in openspec/changes/archive/",
            change_id
        ))
    })?;

    debug!(
        "Reading archived tasks from {}: {:?}",
        resolved.kind.log_label(),
        resolved.file.path
    );
    task_file::read_progress(&resolved.file, Some(change_id))
}

/// Parse task progress with comprehensive fallback order: worktree → archive → base.
///
/// This is the unified progress retrieval helper used by both TUI and Web components.
///
/// Resolution order:
/// 1. Try worktree active location: worktree_path/openspec/changes/{change_id}/tasks.md
/// 2. Try worktree archive location: worktree_path/openspec/changes/archive/{change_id}/tasks.md (or date-prefixed)
/// 3. Try base tree archive location: openspec/changes/archive/{change_id}/tasks.md (or date-prefixed)
/// 4. Try base tree active location: openspec/changes/{change_id}/tasks.md
///
/// Note:
/// This helper is for task *progress reads* across worktree/base-tree boundaries.
/// Rejecting recovery task *writes* use `orchestration::rejection` with a stricter
/// workspace-local canonical order (active change dir first, then archived change dir)
/// and intentionally do not fall back to base-tree paths.
///
/// # Arguments
/// * `change_id` - The ID of the change to retrieve progress for
/// * `worktree_path` - Optional path to the worktree (for uncommitted changes)
///
/// # Returns
/// Task progress with completed and total counts, or an error if not found in any location.
///
/// # Example
/// ```ignore
/// use std::path::Path;
/// use conflux::task_parser;
///
/// // Without worktree (base tree only)
/// let progress = task_parser::parse_progress_with_fallback("my-change", None)?;
///
/// // With worktree (prioritizes worktree)
/// let wt_path = Path::new("/path/to/worktree");
/// let progress = task_parser::parse_progress_with_fallback("my-change", Some(wt_path))?;
/// # Ok::<(), conflux::error::OrchestratorError>(())
/// ```
pub fn parse_progress_with_fallback(
    change_id: &str,
    worktree_path: Option<&Path>,
) -> Result<TaskProgress> {
    if let Some(wt_path) = worktree_path {
        if let Some(message) = task_file::invalid_archive_layout_error(change_id, Some(wt_path)) {
            return Err(OrchestratorError::ConfigLoad(message));
        }
    }
    if let Some(message) = task_file::invalid_archive_layout_error(change_id, None) {
        return Err(OrchestratorError::ConfigLoad(message));
    }

    if let Some(resolved) = task_file::resolve_progress(change_id, worktree_path)? {
        debug!(
            "Reading progress from {}: {:?}",
            resolved.kind.log_label(),
            resolved.file.path
        );
        return task_file::read_progress(&resolved.file, Some(change_id));
    }

    Err(OrchestratorError::ConfigLoad(format!(
        "Tasks file not found for change '{}' in any location (worktree, archive, or base tree)",
        change_id
    )))
}

const ACCEPTANCE_FOLLOW_UP_HEADING: &str = "## Current Acceptance Follow-up";

/// Runtime-owned metadata line carrying the immutable structured finding
/// payload for one follow-up item.
///
/// Persisting the payload (not just its checkbox text) is what lets an
/// interrupted FAIL-to-Apply handoff recover the actionable repair target from
/// workspace-local evidence instead of inferring one.
const FINDING_PAYLOAD_PREFIX: &str = "  finding: ";

fn normalize_acceptance_findings(
    findings: &[crate::acceptance::AcceptanceFinding],
) -> Vec<crate::orchestration::acceptance::NormalizedFinding> {
    let mut normalized = crate::orchestration::acceptance::normalize_findings(findings);
    if normalized.is_empty() {
        normalized = crate::orchestration::acceptance::normalize_findings(&[
            crate::acceptance::AcceptanceFinding::legacy(
                "Investigate acceptance failure and apply the required fix",
            ),
        ]);
    }
    normalized
}

/// One follow-up item already recorded in the workspace.
///
/// `remediation_claimed` is what a checked box means: Apply recorded a repair
/// claim. It is never closure — only a later canonical Acceptance result closes
/// or reopens an item.
#[derive(Debug, PartialEq, Eq)]
struct ExistingAcceptanceFinding {
    finding: crate::orchestration::acceptance::NormalizedFinding,
    remediation_claimed: bool,
    evidence: Vec<String>,
}

fn existing_acceptance_findings(section: &str) -> Vec<ExistingAcceptanceFinding> {
    let mut findings: Vec<ExistingAcceptanceFinding> = Vec::new();
    for line in section.lines() {
        if let Some((remediation_claimed, text)) = ["- [ ] ", "- [x] ", "- [X] "]
            .iter()
            .enumerate()
            .find_map(|(index, prefix)| line.strip_prefix(prefix).map(|text| (index > 0, text)))
        {
            if let Some(finding) = crate::orchestration::acceptance::normalize_findings(&[
                crate::acceptance::AcceptanceFinding::legacy(text),
            ])
            .into_iter()
            .next()
            {
                findings.push(ExistingAcceptanceFinding {
                    finding,
                    remediation_claimed,
                    evidence: Vec::new(),
                });
            }
        } else if let Some(payload) = line.strip_prefix(FINDING_PAYLOAD_PREFIX) {
            // Runtime-owned payload: restores the immutable structured finding,
            // including its stable ID, over the checkbox text Apply can see.
            if let Some(existing) = findings.last_mut() {
                if let Some(finding) = parse_finding_payload(payload) {
                    existing.finding =
                        crate::orchestration::acceptance::normalize_findings(&[finding])
                            .into_iter()
                            .next()
                            .unwrap_or_else(|| existing.finding.clone());
                }
            }
        } else if let Some(evidence) = line.trim().strip_prefix("evidence: ") {
            if let Some(finding) = findings.last_mut() {
                finding.evidence.push(evidence.to_string());
            }
        }
    }
    findings
}

/// Parse one persisted `  finding: {...}` payload back into a shared finding.
///
/// Invalid metadata is ignored rather than trusted: a corrupted payload must not
/// be able to fabricate a finding ID or a repair target.
fn parse_finding_payload(payload: &str) -> Option<crate::acceptance::AcceptanceFinding> {
    let value: serde_json::Value = serde_json::from_str(payload.trim()).ok()?;
    crate::acceptance::validate_repository_finding(&value)
        .ok()
        .map(crate::acceptance::AcceptanceFinding::structured)
}

fn reconcile_apply_progress(
    mut runtime_findings: Vec<crate::orchestration::acceptance::NormalizedFinding>,
    existing_findings: &[ExistingAcceptanceFinding],
) -> (
    Vec<crate::orchestration::acceptance::NormalizedFinding>,
    Vec<String>,
    std::collections::HashMap<String, Vec<String>>,
) {
    let mut claimed_identities = Vec::new();
    let mut evidence_by_identity = std::collections::HashMap::new();
    for candidate in &mut runtime_findings {
        if let Some(existing) = existing_findings
            .iter()
            .find(|existing| existing.finding.identity == candidate.identity)
        {
            // Runtime owns the actionable detail for structured findings: the
            // reviewer's payload wins over whatever text is on the checkbox line
            // so Apply cannot rewrite a finding into something easier.
            if candidate.finding.structured_payload().is_none() {
                candidate.text.clone_from(&existing.finding.text);
                candidate.finding =
                    crate::acceptance::AcceptanceFinding::legacy(existing.finding.text.clone());
            }
            if existing.remediation_claimed {
                claimed_identities.push(candidate.identity.clone());
            }
            if !existing.evidence.is_empty() {
                evidence_by_identity.insert(candidate.identity.clone(), existing.evidence.clone());
            }
        }
    }
    (runtime_findings, claimed_identities, evidence_by_identity)
}

fn render_acceptance_follow_up_section(
    attempt: u32,
    findings: &[crate::orchestration::acceptance::NormalizedFinding],
    claimed_identities: &[String],
    evidence_by_identity: &std::collections::HashMap<String, Vec<String>>,
) -> String {
    let mut section = format!("- attempt: {attempt}\n");
    for finding in findings.iter().filter(|finding| !finding.external) {
        let checked = if claimed_identities.contains(&finding.identity) {
            "x"
        } else {
            " "
        };
        let _ = writeln!(&mut section, "- [{checked}] {}", finding.text);
        if let Some(structured) = finding.finding.structured_payload() {
            let _ = writeln!(
                &mut section,
                "{FINDING_PAYLOAD_PREFIX}{}",
                structured.to_json()
            );
        }
        if let Some(evidence) = evidence_by_identity.get(&finding.identity) {
            for item in evidence {
                let _ = writeln!(&mut section, "  evidence: {item}");
            }
        }
    }
    let external = findings
        .iter()
        .filter(|finding| finding.external)
        .collect::<Vec<_>>();
    if !external.is_empty() {
        section.push_str("\n### External blockers\n");
        for finding in external {
            let _ = writeln!(&mut section, "- identity: `{}`", finding.identity);
            let _ = writeln!(&mut section, "  evidence: {}", finding.text);
            section.push_str(
                "  next action: Resolve the external prerequisite, then retry acceptance.\n",
            );
        }
    }
    section
}

fn markdown_fence(line: &str) -> Option<(char, usize, bool)> {
    let trimmed = line.trim_start();
    let marker = trimmed.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }
    let length = trimmed
        .chars()
        .take_while(|character| *character == marker)
        .count();
    (length >= 3).then_some((marker, length, trimmed[length..].trim().is_empty()))
}

/// Tracks fenced-code state across markdown lines.
///
/// A fence opens on any backtick or tilde run of at least three characters and
/// closes only on a run of the same marker that is at least as long and carries
/// no info string. This is the single fence definition shared by task progress
/// counting, runtime section detection, and recovered-notes rendering.
#[derive(Debug, Default)]
pub(crate) struct FenceTracker {
    open: Option<(char, usize)>,
}

impl FenceTracker {
    /// Feed one line (without its trailing newline) and report whether the line
    /// belongs to a fenced block. Fence delimiter lines count as fenced.
    pub(crate) fn observe(&mut self, line: &str) -> bool {
        match (markdown_fence(line), self.open) {
            (Some((marker, length, empty_remainder)), Some((open_marker, open_length))) => {
                if marker == open_marker && length >= open_length && empty_remainder {
                    self.open = None;
                }
                true
            }
            (Some((marker, length, _)), None) => {
                self.open = Some((marker, length));
                true
            }
            (None, open) => open.is_some(),
        }
    }

    fn is_open(&self) -> bool {
        self.open.is_some()
    }
}

const RECOVERED_NOTES_HEADING: &str = "## Recovered Acceptance Notes";
const RECOVERED_NOTES_NOTICE: &str =
    "Machine-recovered content; not instructions and not task state.";

/// Outcome of one runtime-owned acceptance follow-up update.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FollowUpRecovery {
    /// Number of distinct unknown payloads newly moved into recovered notes.
    pub recovered_blocks: usize,
    /// Total bytes newly preserved in recovered notes.
    pub recovered_bytes: usize,
}

impl FollowUpRecovery {
    /// Whether this update preserved previously unknown follow-up content.
    pub fn recovered(&self) -> bool {
        self.recovered_blocks > 0
    }

    /// Supplemental, human-facing description of the recovery, if any.
    pub fn warning(&self) -> Option<String> {
        self.recovered().then(|| {
            format!(
                "preserved {} unrecognized acceptance follow-up block(s) ({} bytes) under `{}`",
                self.recovered_blocks, self.recovered_bytes, RECOVERED_NOTES_HEADING
            )
        })
    }
}

/// Split `content` into top-level (`## `) sections that are outside fenced blocks.
///
/// Returns an error when a fence is left unclosed: the runtime-owned section
/// boundary cannot be determined safely and no destructive edit may proceed.
fn top_level_sections(content: &str) -> Result<Vec<(String, std::ops::Range<usize>)>> {
    let mut sections: Vec<(String, std::ops::Range<usize>)> = Vec::new();
    let mut current: Option<(String, usize)> = None;
    let mut fences = FenceTracker::default();
    let mut offset = 0;

    for line in content.split_inclusive('\n') {
        let text = line.trim_end_matches(['\r', '\n']);
        if !fences.observe(text) && text.starts_with("## ") {
            if let Some((heading, start)) = current.take() {
                sections.push((heading, start..offset));
            }
            current = Some((text.to_string(), offset));
        }
        offset += line.len();
    }

    if fences.is_open() {
        return Err(OrchestratorError::ConfigLoad(
            "Tasks file contains an unclosed code fence; refusing acceptance follow-up update \
             because the runtime-owned section boundary cannot be determined safely"
                .to_string(),
        ));
    }

    if let Some((heading, start)) = current {
        sections.push((heading, start..content.len()));
    }
    Ok(sections)
}

fn is_acceptance_follow_up_heading(heading: &str) -> bool {
    heading == ACCEPTANCE_FOLLOW_UP_HEADING
        || (heading.starts_with("## Acceptance #") && heading.ends_with(" Failure Follow-up"))
}

fn acceptance_follow_up_ranges(content: &str) -> Result<Vec<std::ops::Range<usize>>> {
    Ok(top_level_sections(content)?
        .into_iter()
        .filter(|(heading, _)| is_acceptance_follow_up_heading(heading))
        .map(|(_, range)| range)
        .collect())
}

/// Whether a follow-up body line is a runtime-owned record.
///
/// Runtime and apply agents have emitted several metadata spellings across
/// versions (with/without leading dash, varying capitalization); all are
/// runtime-owned and are regenerated rather than preserved.
fn is_known_runtime_follow_up_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return true;
    }
    let lowered = trimmed.to_ascii_lowercase();
    if lowered == "### external blockers" {
        return true;
    }
    const KNOWN_PREFIXES: [&str; 12] = [
        "- [ ]",
        "- [x]",
        "- attempt:",
        "attempt:",
        "- identity:",
        "identity:",
        "- evidence:",
        "evidence:",
        "- next action:",
        "next action:",
        "- finding:",
        "finding:",
    ];
    KNOWN_PREFIXES
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
}

/// Extract the unknown bytes of one runtime-owned follow-up section.
///
/// Recognized runtime records are dropped (runtime regenerates them); every
/// other line is preserved with its original bytes. Only trailing newlines are
/// dropped, because the fenced literal supplies its own terminator.
fn unknown_follow_up_payload(section: &str) -> String {
    let mut payload = String::new();
    // Blank lines only belong to the payload when they separate unknown lines,
    // so paragraph breaks inside recovered prose survive while runtime spacing
    // is dropped.
    let mut pending_blank = String::new();
    // Runtime records only exist outside fences: checkbox or metadata text
    // inside a pasted transcript is unknown payload, not a runtime record.
    let mut fences = FenceTracker::default();
    let mut lines = section.split_inclusive('\n');
    let _heading = lines.next();

    for line in lines {
        let text = line.trim_end_matches(['\r', '\n']);
        let fenced = fences.observe(text);
        if !fenced && text.trim().is_empty() {
            pending_blank.push_str(line);
            continue;
        }
        if !fenced && is_known_runtime_follow_up_line(text) {
            pending_blank.clear();
            continue;
        }
        if !payload.is_empty() {
            payload.push_str(&pending_blank);
        }
        pending_blank.clear();
        payload.push_str(line);
    }

    while payload.ends_with('\n') || payload.ends_with('\r') {
        payload.pop();
    }
    payload
}

/// Length of the backtick fence that safely encloses `payload`.
///
/// One character longer than the longest contiguous backtick run inside the
/// payload, and never shorter than three, so embedded fences, headings, and
/// checkbox syntax cannot escape into active Markdown.
fn recovered_fence_length(payload: &str) -> usize {
    let mut longest = 0usize;
    let mut run = 0usize;
    for character in payload.chars() {
        if character == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    longest.saturating_add(1).max(3)
}

/// Read back the payloads already stored in a recovered-notes section.
///
/// Payload bytes are the deduplication identity, so they are compared exactly
/// as they were rendered.
fn recovered_payloads(section: &str) -> Vec<String> {
    let mut payloads = Vec::new();
    let mut open: Option<(char, usize)> = None;
    let mut current = String::new();

    for line in section.split_inclusive('\n') {
        let text = line.trim_end_matches(['\r', '\n']);
        match (markdown_fence(text), open) {
            (Some((marker, length, empty_remainder)), Some((open_marker, open_length))) => {
                if marker == open_marker && length >= open_length && empty_remainder {
                    let mut payload = std::mem::take(&mut current);
                    while payload.ends_with('\n') || payload.ends_with('\r') {
                        payload.pop();
                    }
                    payloads.push(payload);
                    open = None;
                } else {
                    current.push_str(line);
                }
            }
            (Some((marker, length, _)), None) => open = Some((marker, length)),
            (None, Some(_)) => current.push_str(line),
            (None, None) => {}
        }
    }
    payloads
}

fn render_recovered_notes_section(payloads: &[String]) -> String {
    let mut section = format!("{RECOVERED_NOTES_HEADING}\n\n{RECOVERED_NOTES_NOTICE}\n");
    for payload in payloads {
        let fence = "`".repeat(recovered_fence_length(payload));
        let _ = write!(&mut section, "\n{fence}text\n{payload}\n{fence}\n");
    }
    section
}

fn ensure_blank_line_separator(content: &mut String) {
    if content.is_empty() {
        return;
    }
    if !content.ends_with('\n') {
        content.push('\n');
    }
    if !content.ends_with("\n\n") {
        content.push('\n');
    }
}

fn trim_trailing_blank_lines(content: &mut String) {
    while content.ends_with("\n\n\n") {
        content.pop();
    }
}

/// Merge newly recovered payloads into `## Recovered Acceptance Notes`.
///
/// Identical payload bytes are never appended twice, so hydration, retry,
/// restart, and repeated normalization converge on the same file.
fn merge_recovered_notes(content: &mut String, payloads: Vec<String>) -> Result<FollowUpRecovery> {
    let mut recovery = FollowUpRecovery::default();
    if payloads.is_empty() {
        return Ok(recovery);
    }

    let existing_range = top_level_sections(content)?
        .into_iter()
        .find(|(heading, _)| heading == RECOVERED_NOTES_HEADING)
        .map(|(_, range)| range);
    let mut known = existing_range
        .as_ref()
        .map(|range| recovered_payloads(&content[range.clone()]))
        .unwrap_or_default();

    for payload in payloads {
        if known.contains(&payload) {
            continue;
        }
        recovery.recovered_blocks += 1;
        recovery.recovered_bytes += payload.len();
        known.push(payload);
    }
    if !recovery.recovered() {
        return Ok(recovery);
    }

    let rendered = render_recovered_notes_section(&known);
    match existing_range {
        Some(range) => {
            let trailing = if range.end == content.len() { "" } else { "\n" };
            content.replace_range(range, &format!("{rendered}{trailing}"));
        }
        None => {
            ensure_blank_line_separator(content);
            content.push_str(&rendered);
        }
    }
    Ok(recovery)
}

/// Remove every runtime-owned follow-up section, preserving unknown content.
fn strip_and_recover_follow_up_sections(content: &mut String) -> Result<FollowUpRecovery> {
    let ranges = acceptance_follow_up_ranges(content)?;
    let payloads = ranges
        .iter()
        .map(|range| unknown_follow_up_payload(&content[range.clone()]))
        .filter(|payload| !payload.is_empty())
        .collect::<Vec<_>>();

    for range in ranges.into_iter().rev() {
        content.replace_range(range, "");
    }
    trim_trailing_blank_lines(content);
    merge_recovered_notes(content, payloads)
}

fn upsert_acceptance_follow_up_section(
    content: &mut String,
    attempt: u32,
    findings: &[crate::orchestration::acceptance::NormalizedFinding],
    completed_identities: &[String],
    evidence_by_identity: &std::collections::HashMap<String, Vec<String>>,
) -> Result<FollowUpRecovery> {
    let recovery = strip_and_recover_follow_up_sections(content)?;
    ensure_blank_line_separator(content);
    let _ = writeln!(content, "{ACCEPTANCE_FOLLOW_UP_HEADING}");
    content.push_str(&render_acceptance_follow_up_section(
        attempt,
        findings,
        completed_identities,
        evidence_by_identity,
    ));
    Ok(recovery)
}

/// Resolve the workspace-local task artifact Acceptance follow-up mutates.
///
/// Never falls back to the base tree: follow-up state belongs to the resumed
/// workspace only.
pub fn resolve_acceptance_follow_up_tasks_path(
    change_id: &str,
    worktree_path: &Path,
) -> Result<TaskFile> {
    task_file::resolve_mutation(change_id, worktree_path)?
        .map(|resolved| resolved.file)
        .ok_or_else(|| {
            OrchestratorError::ConfigLoad(format!(
                "Acceptance follow-up tasks path not found for change '{}' under worktree '{}'",
                change_id,
                worktree_path.display()
            ))
        })
}

/// Same resolution as [`resolve_acceptance_follow_up_tasks_path`], but a missing
/// artifact is "nothing to clean up" rather than an error.
pub fn resolve_acceptance_follow_up_tasks_path_for_cleanup(
    change_id: &str,
    worktree_path: &Path,
) -> Result<Option<TaskFile>> {
    Ok(task_file::resolve_mutation(change_id, worktree_path)?.map(|resolved| resolved.file))
}

/// Build the replacement `tasks.md` for a latest-FAIL follow-up rewrite.
///
/// Pure: no filesystem access, so recovery, preservation, deduplication, and
/// hard-error boundaries are unit-testable in memory.
fn plan_replace_acceptance_follow_up(
    content: &str,
    attempt: u32,
    findings: &[crate::acceptance::AcceptanceFinding],
) -> Result<(String, FollowUpRecovery)> {
    let mut content = content.to_string();
    let normalized_findings = normalize_acceptance_findings(findings);
    let recovery = upsert_acceptance_follow_up_section(
        &mut content,
        attempt,
        &normalized_findings,
        &[],
        &std::collections::HashMap::new(),
    )?;
    Ok((content, recovery))
}

/// Build the replacement `tasks.md` for apply-side follow-up reconciliation.
fn plan_merge_acceptance_follow_up(
    content: &str,
    attempt: u32,
    findings: &[crate::acceptance::AcceptanceFinding],
) -> Result<(String, FollowUpRecovery)> {
    let mut content = content.to_string();
    let normalized_findings = normalize_acceptance_findings(findings);
    let existing_section = acceptance_follow_up_ranges(&content)?
        .first()
        .map(|range| content[range.clone()].to_string())
        .unwrap_or_default();
    let existing_findings = existing_acceptance_findings(&existing_section);
    let (merged_findings, claimed_identities, evidence_by_identity) =
        reconcile_apply_progress(normalized_findings, &existing_findings);

    let recovery = upsert_acceptance_follow_up_section(
        &mut content,
        attempt,
        &merged_findings,
        &claimed_identities,
        &evidence_by_identity,
    )?;
    Ok((content, recovery))
}

/// Build the replacement `tasks.md` for acceptance PASS cleanup.
fn plan_clear_acceptance_follow_up(content: &str) -> Result<(String, FollowUpRecovery)> {
    let mut content = content.to_string();
    let recovery = strip_and_recover_follow_up_sections(&mut content)?;
    Ok((content, recovery))
}

/// Project normalized findings into the runtime-owned JSON follow-up block.
///
/// Internal findings become virtual progress-gate items; external blockers stay
/// outside the task counts, exactly as their Markdown counterparts do.
fn build_json_follow_up(
    attempt: u32,
    findings: &[crate::orchestration::acceptance::NormalizedFinding],
    claimed_identities: &[String],
    evidence_by_identity: &std::collections::HashMap<String, Vec<String>>,
) -> JsonFollowUp {
    JsonFollowUp {
        attempt,
        findings: findings
            .iter()
            .filter(|finding| !finding.external)
            .map(|finding| JsonFinding {
                identity: finding.identity.clone(),
                text: finding.text.clone(),
                finding: finding.finding.structured_payload().cloned(),
                remediation_claimed: claimed_identities.contains(&finding.identity),
                evidence: evidence_by_identity
                    .get(&finding.identity)
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect(),
        external_blockers: findings
            .iter()
            .filter(|finding| finding.external)
            .map(|finding| JsonExternalBlocker {
                identity: finding.identity.clone(),
                text: finding.text.clone(),
                evidence: vec![finding.text.clone()],
            })
            .collect(),
    }
}

/// Re-read stored JSON findings through the shared reconciliation contract.
fn existing_json_findings(
    document: &task_file::JsonTaskDocument,
) -> Vec<ExistingAcceptanceFinding> {
    document
        .follow_up
        .as_ref()
        .map(|follow_up| {
            follow_up
                .findings
                .iter()
                .map(|finding| ExistingAcceptanceFinding {
                    finding: crate::orchestration::acceptance::NormalizedFinding {
                        identity: finding.identity.clone(),
                        text: finding.text.clone(),
                        external: false,
                        finding: finding.to_acceptance_finding(),
                    },
                    remediation_claimed: finding.remediation_claimed,
                    evidence: finding.evidence.clone(),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn write_json_follow_up(
    tasks_file: &TaskFile,
    follow_up: Option<JsonFollowUp>,
) -> Result<FollowUpRecovery> {
    let original = task_file::read_to_string(tasks_file)?;
    let mut document = task_file::parse_json_document(&original)?;
    document.set_follow_up(follow_up);
    let content = document.to_content()?;
    if content != original {
        task_file::write_atomically(tasks_file, &content)?;
    }
    // Structured storage keeps unknown fields in place, so a JSON update never
    // relocates content into recovered notes.
    Ok(FollowUpRecovery::default())
}

/// Record the latest Acceptance FAIL as the runtime-owned follow-up.
pub fn replace_acceptance_follow_up_from_latest_fail(
    tasks_file: &TaskFile,
    attempt: u32,
    findings: &[crate::acceptance::AcceptanceFinding],
) -> Result<FollowUpRecovery> {
    match tasks_file.format {
        TaskFileFormat::Markdown => {
            let original = task_file::read_to_string(tasks_file)?;
            let (content, recovery) =
                plan_replace_acceptance_follow_up(&original, attempt, findings)?;
            task_file::write_atomically(tasks_file, &content)?;
            Ok(recovery)
        }
        TaskFileFormat::Json => {
            let normalized = normalize_acceptance_findings(findings);
            let follow_up =
                build_json_follow_up(attempt, &normalized, &[], &std::collections::HashMap::new());
            write_json_follow_up(tasks_file, Some(follow_up))
        }
    }
}

/// Reconcile Apply's recorded remediation claims with the latest findings.
pub fn merge_acceptance_follow_up_apply_progress(
    tasks_file: &TaskFile,
    attempt: u32,
    findings: &[crate::acceptance::AcceptanceFinding],
) -> Result<FollowUpRecovery> {
    match tasks_file.format {
        TaskFileFormat::Markdown => {
            let original = task_file::read_to_string(tasks_file)?;
            let (content, recovery) =
                plan_merge_acceptance_follow_up(&original, attempt, findings)?;
            task_file::write_atomically(tasks_file, &content)?;
            Ok(recovery)
        }
        TaskFileFormat::Json => {
            let original = task_file::read_to_string(tasks_file)?;
            let document = task_file::parse_json_document(&original)?;
            let existing = existing_json_findings(&document);
            let (merged, claimed, evidence) =
                reconcile_apply_progress(normalize_acceptance_findings(findings), &existing);
            let follow_up = build_json_follow_up(attempt, &merged, &claimed, &evidence);
            write_json_follow_up(tasks_file, Some(follow_up))
        }
    }
}

/// Read the runtime-owned current follow-up as actionable findings.
///
/// Structured findings are restored from their runtime-owned payload, so an
/// interrupted FAIL-to-Apply handoff recovers stable IDs, evidence, required
/// changes, and verification expectations rather than display text alone. A
/// remediation claim is a claim only; nothing here implies closure or PASS.
pub fn read_acceptance_follow_up(
    tasks_file: &TaskFile,
) -> Result<Option<(u32, Vec<crate::acceptance::AcceptanceFinding>)>> {
    if tasks_file.format == TaskFileFormat::Json {
        let document = task_file::parse_json_document(&task_file::read_to_string(tasks_file)?)?;
        let Some(follow_up) = document.current_follow_up() else {
            return Ok(None);
        };
        let mut findings = follow_up
            .findings
            .iter()
            .map(JsonFinding::to_acceptance_finding)
            .collect::<Vec<_>>();
        findings.extend(
            follow_up
                .external_blockers
                .iter()
                .map(|blocker| crate::acceptance::AcceptanceFinding::legacy(blocker.text.clone())),
        );
        return Ok((!findings.is_empty()).then_some((follow_up.attempt, findings)));
    }

    let tasks_path = tasks_file.path.as_path();
    let content = task_file::read_to_string(tasks_file)?;
    let ranges = acceptance_follow_up_ranges(&content)?;
    let Some(range) = ranges.last() else {
        return Ok(None);
    };
    let section = &content[range.clone()];
    let mut lines = section.lines();
    let heading = lines.next().unwrap_or_default();
    let attempt = if heading == ACCEPTANCE_FOLLOW_UP_HEADING {
        lines
            .next()
            .and_then(|line| line.strip_prefix("- attempt: "))
            .and_then(|value| value.parse::<u32>().ok())
    } else {
        heading
            .strip_prefix("## Acceptance #")
            .and_then(|value| value.strip_suffix(" Failure Follow-up"))
            .and_then(|value| value.parse::<u32>().ok())
    }
    .ok_or_else(|| {
        OrchestratorError::ConfigLoad(format!(
            "Invalid acceptance follow-up heading in {}",
            tasks_path.display()
        ))
    })?;
    let mut findings: Vec<crate::acceptance::AcceptanceFinding> = Vec::new();
    let mut in_external_blockers = false;
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "### External blockers" {
            in_external_blockers = true;
            continue;
        }
        if let Some(finding) = ["- [ ] ", "- [x] ", "- [X] "]
            .iter()
            .find_map(|prefix| line.strip_prefix(prefix))
        {
            findings.push(crate::acceptance::AcceptanceFinding::legacy(finding));
        } else if let Some(payload) = line.strip_prefix(FINDING_PAYLOAD_PREFIX) {
            // The runtime-owned payload is authoritative for the preceding item.
            if let (Some(last), Some(structured)) = (
                findings.len().checked_sub(1),
                parse_finding_payload(payload),
            ) {
                findings[last] = structured;
            }
        } else if in_external_blockers {
            if let Some(evidence) = trimmed.strip_prefix("evidence: ") {
                findings.push(crate::acceptance::AcceptanceFinding::legacy(evidence));
            }
        }
    }
    Ok((!findings.is_empty()).then_some((attempt, findings)))
}

/// Read the Apply-authored remediation evidence lines from the current
/// follow-up.
///
/// Evidence is a remediation *claim*: it never closes a finding and never
/// implies Acceptance PASS.
pub fn read_acceptance_follow_up_evidence(tasks_file: &TaskFile) -> Result<Vec<String>> {
    let content = task_file::read_to_string(tasks_file)?;
    if tasks_file.format == TaskFileFormat::Json {
        let document = task_file::parse_json_document(&content)?;
        let Some(follow_up) = document.current_follow_up() else {
            return Ok(Vec::new());
        };
        return Ok(follow_up
            .findings
            .iter()
            .flat_map(|finding| finding.evidence.iter().cloned())
            .chain(
                follow_up
                    .external_blockers
                    .iter()
                    .flat_map(|blocker| blocker.evidence.iter().cloned()),
            )
            .collect());
    }

    let ranges = acceptance_follow_up_ranges(&content)?;
    let Some(range) = ranges.last() else {
        return Ok(Vec::new());
    };
    Ok(content[range.clone()]
        .lines()
        .filter_map(|line| line.trim().strip_prefix("evidence: "))
        .map(str::to_string)
        .collect())
}

/// Stable identifier of the rejection-recovery task in JSON documents.
const REJECTING_RECOVERY_TASK_ID: &str = "rejecting-recovery";

/// Heading of the rejection-recovery section in Markdown documents.
pub const REJECTING_RECOVERY_HEADING: &str = "## Rejecting Recovery Tasks";

/// The recovery task text a rejecting handoff records for `change_id`.
pub fn rejecting_recovery_task_text(change_id: &str) -> String {
    format!(
        "Capture unresolved blocker details in the task file (do not recreate REJECTED.md) and implement a non-rejection recovery path before rerunning apply for {}",
        change_id
    )
}

/// Insert the rejection-recovery task into an artifact, in its own format.
///
/// Idempotent in both representations: a repeated handoff converges on the same
/// document rather than appending the task again.
pub fn append_rejecting_recovery_task(tasks_file: &TaskFile, change_id: &str) -> Result<()> {
    let original = task_file::read_to_string(tasks_file)?;
    let content = match tasks_file.format {
        TaskFileFormat::Markdown => {
            append_recovery_task_section(&original, &rejecting_recovery_task_text(change_id))
        }
        TaskFileFormat::Json => {
            let mut document = task_file::parse_json_document(&original)?;
            if !document.append_pending_task(
                REJECTING_RECOVERY_TASK_ID,
                &rejecting_recovery_task_text(change_id),
            ) {
                return Ok(());
            }
            document.to_content()?
        }
    };
    if content != original {
        task_file::write_atomically(tasks_file, &content)?;
    }
    Ok(())
}

/// Append the Markdown rejection-recovery section, preserving an existing one.
fn append_recovery_task_section(existing: &str, task_text: &str) -> String {
    let task = format!("- [ ] {task_text}");

    if existing.contains(REJECTING_RECOVERY_HEADING) {
        if existing.contains(&task) {
            return existing.to_string();
        }
        return format!("{}\n{}\n", existing.trim_end(), task);
    }

    format!(
        "{}\n\n{}\n\n{}\n",
        existing.trim_end(),
        REJECTING_RECOVERY_HEADING,
        task
    )
}

/// Remove the runtime-owned follow-up after Acceptance PASS.
pub fn clear_acceptance_follow_up(tasks_file: &TaskFile) -> Result<FollowUpRecovery> {
    match tasks_file.format {
        TaskFileFormat::Markdown => {
            let original = task_file::read_to_string(tasks_file)?;
            let (content, recovery) = plan_clear_acceptance_follow_up(&original)?;
            if content != original {
                task_file::write_atomically(tasks_file, &content)?;
            }
            Ok(recovery)
        }
        TaskFileFormat::Json => write_json_follow_up(tasks_file, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tasks(path: impl AsRef<Path>, content: &str) {
        let path = path.as_ref();
        std::fs::create_dir_all(path.parent().expect("tasks path has a parent")).unwrap();
        std::fs::write(path, content).unwrap();
    }

    /// Bind `<dir>/tasks.md` as an explicit Markdown artifact.
    pub(super) fn markdown_task_file(dir: &Path) -> TaskFile {
        TaskFile::in_entry(dir, TaskFileFormat::Markdown)
    }

    /// Bind `<dir>/tasks.json` as an explicit JSON artifact.
    pub(super) fn json_task_file(dir: &Path) -> TaskFile {
        TaskFile::in_entry(dir, TaskFileFormat::Json)
    }

    fn checked_tasks(count: u32) -> String {
        (1..=count)
            .map(|index| format!("- [x] Task {}\n", index))
            .collect()
    }

    #[test]
    fn stable_finding_identity_is_code_first_and_structural_without_code() {
        let findings = normalize_acceptance_findings(&[
            "[RETRY_MISSING] old evidence at src/run.rs:10".into(),
            "[RETRY_MISSING] changed evidence at tests/run.rs:90".into(),
            "Missing retry test at src/run.rs:11".into(),
            "Different prose: regression coverage absent in src/run.rs:99".into(),
            "Incorrect implementation at src/run.rs:12".into(),
            "Missing retry test at src/other.rs:10".into(),
        ]);
        let identities = findings
            .iter()
            .map(|finding| finding.identity.as_str())
            .collect::<Vec<_>>();
        assert_eq!(findings.len(), 4);
        assert!(identities.contains(&"repository|code|[retry_missing]"));
        assert!(identities.contains(&"repository|src/run.rs|verification"));
        assert!(identities.contains(&"repository|src/run.rs|implementation"));
        assert!(identities.contains(&"repository|src/other.rs|verification"));
    }

    #[test]
    fn apply_reconciliation_is_monotonic_and_preserves_text_and_evidence() {
        let existing = existing_acceptance_findings(
            "- [x] Missing retry test at src/run.rs:10\n  evidence: cargo test retry passes\n- [ ] Broken implementation at src/other.rs:4\n",
        );
        let incoming = normalize_acceptance_findings(&[
            "Regression coverage absent in src/run.rs:99".into(),
            "Incorrect implementation at src/other.rs:40".into(),
        ]);

        let (merged, completed, evidence) = reconcile_apply_progress(incoming, &existing);

        assert!(merged
            .iter()
            .any(|finding| finding.text == "Missing retry test at src/run.rs:10"));
        assert!(merged
            .iter()
            .any(|finding| finding.text == "Broken implementation at src/other.rs:4"));
        assert_eq!(completed, ["repository|src/run.rs|verification"]);
        assert_eq!(
            evidence["repository|src/run.rs|verification"],
            ["cargo test retry passes"]
        );
    }

    // ====================
    // Bullet list format tests
    // ====================

    #[test]
    fn test_bullet_unchecked() {
        let content = "- [ ] Task 1\n- [ ] Task 2";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_bullet_checked_lowercase() {
        let content = "- [x] Task 1\n- [x] Task 2";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.completed, 2);
    }

    #[test]
    fn test_bullet_checked_uppercase() {
        let content = "- [X] Task 1\n- [X] Task 2";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.completed, 2);
    }

    #[test]
    fn test_asterisk_bullets() {
        let content = "* [ ] Task 1\n* [x] Task 2";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.completed, 1);
    }

    #[test]
    fn test_bullet_mixed_status() {
        let content = "- [x] Completed\n- [ ] Pending\n- [X] Also done";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 3);
        assert_eq!(progress.completed, 2);
    }

    // ====================
    // Numbered list format tests
    // ====================

    #[test]
    fn test_numbered_unchecked() {
        let content = "1. [ ] Task 1\n2. [ ] Task 2";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_record_acceptance_follow_up_appends_unchecked_tasks() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(&tasks_path, "## Implementation Tasks\n- [x] done\n").unwrap();

        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            2,
            &[
                "missing repository coverage".to_string().into(),
                "add notification links".to_string().into(),
            ],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("## Current Acceptance Follow-up"));
        assert!(content.contains("- [ ] missing repository coverage"));
        assert!(content.contains("- [ ] add notification links"));

        let progress = task_file::read_progress(&tasks_path, None).unwrap();
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.total, 3);
    }

    #[test]
    fn test_record_acceptance_follow_up_replaces_existing_section() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- [x] stale\n\n## Final Validation\n- [ ] run tests\n",
        )
        .unwrap();

        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            1,
            &["fresh finding".to_string().into()],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("## Current Acceptance Follow-up"));
        assert!(content.contains("- [ ] fresh finding"));
        assert!(content.contains("## Final Validation\n- [ ] run tests"));
        assert!(!content.contains("- [x] stale"));
    }

    #[test]
    fn record_acceptance_follow_up_replaces_previous_attempt() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Acceptance #1 Failure Follow-up\n- [x] stale\n",
        )
        .unwrap();

        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            2,
            &["latest finding".to_string().into()],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(!content.contains("## Acceptance #1 Failure Follow-up"));
        assert_eq!(
            content.matches("## Current Acceptance Follow-up").count(),
            1
        );
        assert!(content.contains("- [ ] latest finding"));
    }

    #[test]
    fn ensure_acceptance_follow_up_restores_deleted_runtime_section() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(&tasks_path, "## Implementation Tasks\n- [x] done\n").unwrap();

        merge_acceptance_follow_up_apply_progress(
            &tasks_path,
            2,
            &[
                "latest finding".to_string().into(),
                "add regression test".to_string().into(),
            ],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("## Current Acceptance Follow-up"));
        assert!(content.contains("- [ ] latest finding"));
        assert!(content.contains("- [ ] add regression test"));
    }

    #[test]
    fn ensure_acceptance_follow_up_restores_deleted_finding_and_preserves_completed_finding() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- [x] add regression test\n",
        )
        .unwrap();

        merge_acceptance_follow_up_apply_progress(
            &tasks_path,
            2,
            &[
                "latest finding".to_string().into(),
                "add regression test".to_string().into(),
            ],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("- [x] add regression test"));
        assert!(content.contains("- [ ] latest finding"));
        let progress = task_file::read_progress(&tasks_path, None).unwrap();
        assert_eq!(progress, TaskProgress::with_counts(2, 3));
    }

    #[test]
    fn apply_progress_preserves_completed_fallback_identity_text_and_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- attempt: 1\n- [x] Missing retry coverage at src/example.rs:10\n  evidence: cargo test retry passes\n- [ ] Incorrect implementation at src/other.rs:4\n",
        )
        .unwrap();

        merge_acceptance_follow_up_apply_progress(
            &tasks_path,
            2,
            &[
                "Regression test absent in src/example.rs:99 with changed detail"
                    .to_string()
                    .into(),
                "Incorrect implementation at src/other.rs:40 with new evidence"
                    .to_string()
                    .into(),
            ],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("- [x] Missing retry coverage at src/example.rs:10"));
        assert!(content.contains("evidence: cargo test retry passes"));
        assert!(content.contains("- [ ] Incorrect implementation at src/other.rs:4"));
        assert!(!content.contains("changed detail"));
        assert_eq!(
            task_file::read_progress(&tasks_path, None).unwrap(),
            TaskProgress::with_counts(2, 3)
        );
    }

    #[test]
    fn ensure_acceptance_follow_up_preserves_completed_finding_by_code() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- [x] [SERIAL_STALLED_MARKER_MISSING] fixed and verified\n",
        )
        .unwrap();

        merge_acceptance_follow_up_apply_progress(
            &tasks_path,
            2,
            &["[SERIAL_STALLED_MARKER_MISSING] detailed original finding"
                .to_string()
                .into()],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("- [x] [SERIAL_STALLED_MARKER_MISSING] fixed and verified"));
        assert_eq!(
            task_file::read_progress(&tasks_path, None).unwrap(),
            TaskProgress::with_counts(2, 2)
        );
    }

    #[test]
    fn record_acceptance_follow_up_reopens_repeated_stable_identity() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- attempt: 1\n- [x] [SERIAL_STALLED_MARKER_MISSING] fixed and verified\n",
        )
        .unwrap();

        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            2,
            &[
                "[SERIAL_STALLED_MARKER_MISSING] still missing at src/run.rs:42"
                    .to_string()
                    .into(),
            ],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content
            .contains("- [ ] [SERIAL_STALLED_MARKER_MISSING] still missing at src/run.rs:42"));
        assert!(!content.contains("fixed and verified"));
    }

    #[test]
    fn acceptance_follow_up_renders_external_blockers_without_checkboxes() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(&tasks_path, "## Implementation Tasks\n- [x] done\n").unwrap();

        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            3,
            &[
                "fix repository regression at src/run.rs:4"
                    .to_string()
                    .into(),
                "external non-mockable prerequisite: vendor approval"
                    .to_string()
                    .into(),
            ],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("- [ ] fix repository regression at src/run.rs:4"));
        assert!(content.contains("### External blockers"));
        assert!(content.contains("evidence: external non-mockable prerequisite: vendor approval"));
        assert!(content.contains("next action: Resolve the external prerequisite"));
        assert!(!content.contains("- [ ] external non-mockable prerequisite"));
        assert_eq!(
            task_file::read_progress(&tasks_path, None).unwrap(),
            TaskProgress::with_counts(1, 2)
        );
    }

    #[test]
    fn read_acceptance_follow_up_restores_mixed_repository_and_external_findings() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(
            &tasks_path,
            "## Current Acceptance Follow-up\n- attempt: 3\n- [x] fix repository regression at src/run.rs:4\n\n### External blockers\n- identity: `external||vendor approval|plain`\n  evidence: external non-mockable prerequisite: vendor approval\n  next action: Resolve the external prerequisite, then retry acceptance.\n",
        )
        .unwrap();

        let follow_up = read_acceptance_follow_up(&tasks_path).unwrap();

        assert_eq!(
            follow_up,
            Some((
                3,
                crate::acceptance::legacy_findings([
                    "fix repository regression at src/run.rs:4",
                    "external non-mockable prerequisite: vendor approval",
                ]),
            ))
        );
    }

    #[test]
    fn acceptance_follow_up_normalizes_multiline_findings() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(&tasks_path, "## Implementation Tasks\n- [x] done\n").unwrap();

        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            2,
            &["finding\n## injected heading\n- [ ] injected task"
                .to_string()
                .into()],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("- [ ] finding ## injected heading - [ ] injected task"));
        assert_eq!(
            content.matches("## Current Acceptance Follow-up").count(),
            1
        );
        assert_eq!(
            task_file::read_progress(&tasks_path, None).unwrap().total,
            2
        );
    }

    #[test]
    fn clear_acceptance_follow_up_ignores_examples_in_code_fences() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(
            &tasks_path,
            "## Notes\n```md\n## Acceptance #9 Failure Follow-up\n- [ ] example\n```\n\n## Current Acceptance Follow-up\n- [x] fixed\n",
        )
        .unwrap();

        clear_acceptance_follow_up(&tasks_path).unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("## Acceptance #9 Failure Follow-up\n- [ ] example"));
        assert!(!content.contains("## Current Acceptance Follow-up"));
    }

    #[test]
    fn clear_acceptance_follow_up_ignores_tilde_fence_examples() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(
            &tasks_path,
            "## Notes\n~~~md\n## Acceptance #9 Failure Follow-up\n- [ ] example\n~~~\n",
        )
        .unwrap();

        clear_acceptance_follow_up(&tasks_path).unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("## Acceptance #9 Failure Follow-up\n- [ ] example"));
    }

    #[test]
    fn clear_acceptance_follow_up_does_not_close_fence_with_info_string() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        let original =
            "## Notes\n```text\n```md\n## Acceptance #9 Failure Follow-up\n- [ ] example\n```\n";
        std::fs::write(&tasks_path, original).unwrap();

        clear_acceptance_follow_up(&tasks_path).unwrap();

        assert_eq!(std::fs::read_to_string(&tasks_path).unwrap(), original);
    }

    #[test]
    fn record_acceptance_follow_up_replaces_legacy_section_with_runtime_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Acceptance #1 Failure Follow-up\n- attempt: 1\n- [x] stale finding\n  evidence: cargo test passed\n",
        )
        .unwrap();

        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            2,
            &["latest finding".to_string().into()],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(!content.contains("## Acceptance #1 Failure Follow-up"));
        assert!(content.contains("## Current Acceptance Follow-up\n- attempt: 2"));
        assert!(content.contains("- [ ] latest finding"));
    }

    #[test]
    fn clear_acceptance_follow_up_recovers_non_runtime_section_content() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        let original = "## Current Acceptance Follow-up\n- [x] fixed\n```md\n## injected\n```\n";
        std::fs::write(&tasks_path, original).unwrap();

        let recovery = clear_acceptance_follow_up(&tasks_path).unwrap();

        assert_eq!(recovery.recovered_blocks, 1);
        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(!content.contains("## Current Acceptance Follow-up"));
        assert!(content.contains(RECOVERED_NOTES_HEADING));
        assert!(content.contains("````text\n```md\n## injected\n```\n````"));
    }

    #[test]
    fn clear_acceptance_follow_up_removes_runtime_sections_only() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- [x] fixed\n\n## Final Validation\nvalidation passed\n",
        )
        .unwrap();

        clear_acceptance_follow_up(&tasks_path).unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(!content.contains("Failure Follow-up"));
        assert!(content.contains("## Implementation Tasks\n- [x] done"));
        assert!(content.contains("## Final Validation\nvalidation passed"));
    }

    #[test]
    fn test_record_acceptance_follow_up_uses_default_finding_for_empty_input() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(&tasks_path, "## Implementation Tasks\n- [x] done\n").unwrap();

        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            3,
            &[" ".to_string().into(), "\t".to_string().into()],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("## Current Acceptance Follow-up"));
        assert!(content.contains("- [ ] Investigate acceptance failure and apply the required fix"));
    }

    #[test]
    fn test_record_acceptance_follow_up_adds_missing_trailing_newline_before_section() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(&tasks_path, "## Implementation Tasks\n- [x] done").unwrap();

        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            4,
            &["fresh finding".to_string().into()],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert_eq!(
            content,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- attempt: 4\n- [ ] fresh finding\n"
        );
    }

    #[test]
    fn test_resolve_acceptance_follow_up_tasks_path_prefers_active_path() {
        let dir = tempfile::tempdir().unwrap();
        let change_id = "change-a";
        let active_dir = dir.path().join("openspec/changes").join(change_id);
        std::fs::create_dir_all(&active_dir).unwrap();
        let active_tasks = active_dir.join("tasks.md");
        std::fs::write(&active_tasks, "- [ ] active task").unwrap();

        let resolved = resolve_acceptance_follow_up_tasks_path(change_id, dir.path()).unwrap();
        assert_eq!(resolved.path, active_tasks);
        assert_eq!(resolved.format, TaskFileFormat::Markdown);
    }

    #[test]
    fn test_resolve_acceptance_follow_up_tasks_path_falls_back_to_archive_path() {
        let dir = tempfile::tempdir().unwrap();
        let change_id = "change-b";
        let archive_dir = dir.path().join("openspec/changes/archive").join(change_id);
        std::fs::create_dir_all(&archive_dir).unwrap();
        let archive_tasks = archive_dir.join("tasks.md");
        std::fs::write(&archive_tasks, "- [ ] archived task").unwrap();

        let resolved = resolve_acceptance_follow_up_tasks_path(change_id, dir.path()).unwrap();
        assert_eq!(resolved.path, archive_tasks);
        assert_eq!(resolved.format, TaskFileFormat::Markdown);
    }

    #[test]
    fn cleanup_resolver_returns_none_when_follow_up_tasks_are_absent() {
        let dir = tempfile::tempdir().unwrap();

        assert_eq!(
            resolve_acceptance_follow_up_tasks_path_for_cleanup("change-c", dir.path()).unwrap(),
            None
        );
    }

    #[test]
    fn cleanup_resolver_rejects_invalid_archive_layout() {
        let dir = tempfile::tempdir().unwrap();
        let nested_tasks = dir
            .path()
            .join("openspec/changes/archive/2026-07-09/change-c/tasks.md");
        write_tasks(&nested_tasks, "- [x] archived\n");

        let error = resolve_acceptance_follow_up_tasks_path_for_cleanup("change-c", dir.path())
            .unwrap_err();

        assert!(error.to_string().contains("Invalid archive layout"));
    }

    #[test]
    fn test_resolve_acceptance_follow_up_tasks_path_errors_when_missing_everywhere() {
        let dir = tempfile::tempdir().unwrap();
        let change_id = "change-c";

        let result = resolve_acceptance_follow_up_tasks_path(change_id, dir.path());
        assert!(result.is_err());
    }

    const JSON_TASKS: &str = concat!(
        "{\n",
        "  \"schema_version\": 1,\n",
        "  \"vendor_extension\": {\"keep\": true},\n",
        "  \"tasks\": [\n",
        "    {\"id\": \"impl\", \"title\": \"Implement\", \"status\": \"completed\", \"section\": \"implementation\"},\n",
        "    {\"id\": \"docs\", \"title\": \"Document\", \"status\": \"pending\", \"section\": \"specification\"}\n",
        "  ]\n",
        "}\n"
    );

    fn structured_json_finding(id: &str, summary: &str) -> crate::acceptance::AcceptanceFinding {
        let payload = serde_json::json!({
            "id": id,
            "severity": "major",
            "summary": summary,
            "evidence": ["observed at src/run.rs:10"],
            "required_changes": [{"file": "src/run.rs", "description": "repair the behavior"}],
            "verification": [{"file": "tests/run.rs", "description": "cover the repair"}],
        });
        crate::acceptance::AcceptanceFinding::structured(
            crate::acceptance::validate_repository_finding(&payload).expect("valid finding"),
        )
    }

    #[test]
    fn json_follow_up_round_trips_through_fail_apply_restart_and_pass() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_file = json_task_file(dir.path());
        write_tasks(&tasks_file, JSON_TASKS);

        // 1. Acceptance FAIL records the runtime-owned findings.
        let findings = vec![
            structured_json_finding("F1", "Regression coverage is missing"),
            crate::acceptance::AcceptanceFinding::legacy(
                "external non-mockable prerequisite: vendor approval",
            ),
        ];
        let recovery =
            replace_acceptance_follow_up_from_latest_fail(&tasks_file, 2, &findings).unwrap();
        assert_eq!(recovery, FollowUpRecovery::default());

        // One completed ordinary task plus one unclaimed virtual finding gate.
        assert_eq!(
            task_file::read_progress(&tasks_file, None).unwrap(),
            TaskProgress::with_counts(1, 3)
        );

        // 2. Apply hydration reads identity and actionable payload back.
        let (attempt, hydrated) = read_acceptance_follow_up(&tasks_file).unwrap().unwrap();
        assert_eq!(attempt, 2);
        assert!(hydrated.iter().any(|finding| finding.id() == Some("F1")));

        // 3. Apply claims a repair with evidence; identity survives the rewrite.
        {
            let content = std::fs::read_to_string(&tasks_file).unwrap();
            let mut document = task_file::parse_json_document(&content).unwrap();
            let mut follow_up = document.follow_up.clone().unwrap();
            follow_up.findings[0].remediation_claimed = true;
            follow_up.findings[0].evidence = vec!["cargo test --lib passes".to_string()];
            document.set_follow_up(Some(follow_up));
            std::fs::write(&tasks_file, document.to_content().unwrap()).unwrap();
        }

        // 4. A restart re-reads the file and reconciles without losing the claim.
        merge_acceptance_follow_up_apply_progress(&tasks_file, attempt, &hydrated).unwrap();
        let reloaded =
            task_file::parse_json_document(&std::fs::read_to_string(&tasks_file).unwrap()).unwrap();
        let stored = reloaded.follow_up.as_ref().unwrap();
        assert_eq!(stored.attempt, 2);
        assert!(stored.findings.iter().any(|finding| {
            finding.remediation_claimed
                && finding.evidence == ["cargo test --lib passes"]
                && finding.finding.as_ref().map(|f| f.id.as_str()) == Some("F1")
        }));
        assert_eq!(stored.external_blockers.len(), 1);
        assert_eq!(
            read_acceptance_follow_up_evidence(&tasks_file).unwrap()[0],
            "cargo test --lib passes"
        );

        // 5. PASS cleanup removes only the runtime-owned block.
        clear_acceptance_follow_up(&tasks_file).unwrap();
        let cleaned = std::fs::read_to_string(&tasks_file).unwrap();
        assert!(!cleaned.contains("acceptance_follow_up"), "{cleaned}");
        assert!(cleaned.contains("vendor_extension"), "{cleaned}");
        assert_eq!(
            task_file::read_progress(&tasks_file, None).unwrap(),
            TaskProgress::with_counts(1, 2)
        );
        assert!(read_acceptance_follow_up(&tasks_file).unwrap().is_none());
    }

    #[test]
    fn json_task_file_errors_fail_closed_instead_of_reporting_zero_of_zero() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_file = json_task_file(dir.path());
        write_tasks(&tasks_file, "{ not json ");

        let error = task_file::read_progress(&tasks_file, None).unwrap_err();
        assert!(error.to_string().contains("tasks.json:"), "{error}");
        assert!(read_acceptance_follow_up(&tasks_file).is_err());
        assert!(clear_acceptance_follow_up(&tasks_file).is_err());
        assert!(
            replace_acceptance_follow_up_from_latest_fail(&tasks_file, 1, &[]).is_err(),
            "mutation must refuse an unreadable artifact"
        );
    }

    #[test]
    fn json_follow_up_resolution_selects_the_json_artifact() {
        let dir = tempfile::tempdir().unwrap();
        let change_id = "json-change";
        let change_dir = dir.path().join("openspec/changes").join(change_id);
        write_tasks(change_dir.join("tasks.json"), JSON_TASKS);

        let resolved = resolve_acceptance_follow_up_tasks_path(change_id, dir.path()).unwrap();
        assert_eq!(resolved.format, TaskFileFormat::Json);
        assert_eq!(resolved.path, change_dir.join("tasks.json"));
    }

    #[test]
    fn parse_file_binds_the_format_from_the_basename() {
        let dir = tempfile::tempdir().unwrap();
        write_tasks(dir.path().join("tasks.json"), JSON_TASKS);
        assert_eq!(
            parse_file(&dir.path().join("tasks.json"), None).unwrap(),
            TaskProgress::with_counts(1, 2)
        );

        let error = parse_file(&dir.path().join("tasks.yaml"), None).unwrap_err();
        assert!(
            error.to_string().contains("Unsupported task artifact"),
            "{error}"
        );
    }

    #[test]
    fn test_numbered_checked() {
        let content = "1. [x] Task 1\n2. [x] Task 2";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.completed, 2);
    }

    #[test]
    fn test_numbered_multi_digit() {
        let content = "1. [x] Task 1\n10. [ ] Task 10\n100. [X] Task 100";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 3);
        assert_eq!(progress.completed, 2);
    }

    #[test]
    fn test_numbered_mixed_status() {
        let content = "1. [x] Done\n2. [ ] Not done\n3. [X] Also done";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 3);
        assert_eq!(progress.completed, 2);
    }

    // ====================
    // Mixed format tests
    // ====================

    #[test]
    fn test_mixed_bullets_and_numbers() {
        let content =
            "- [x] Bullet done\n1. [ ] Number pending\n* [X] Asterisk done\n2. [x] Number done";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 4);
        assert_eq!(progress.completed, 3);
    }

    #[test]
    fn test_mixed_with_sections() {
        let content = r#"# Tasks

## Implementation
- [x] Task 1
- [ ] Task 2

## Testing
1. [x] Test 1
2. [ ] Test 2
"#;
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 4);
        assert_eq!(progress.completed, 2);
    }

    // ====================
    // Edge case tests
    // ====================

    #[test]
    fn test_empty_content() {
        let progress = parse_content("", None);
        assert_eq!(progress.total, 0);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_no_tasks() {
        let content = "# Just a header\nSome text without tasks.\n\n- Regular list item";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 0);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_indented_not_counted() {
        let content =
            "- [x] Parent task\n  - [ ] Sub-task (should not count)\n  - [x] Another sub-task";
        let progress = parse_content(content, None);
        // Only the parent task at the start of line should count
        assert_eq!(progress.total, 1);
        assert_eq!(progress.completed, 1);
    }

    #[test]
    fn test_inline_checkbox_not_counted() {
        let content = "Some text with [ ] inline checkbox\nAnother line [x] here";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 0);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_header_checkbox_not_counted() {
        let content = "## [x] Header with checkbox\n### [ ] Another header";
        let progress = parse_content(content, None);
        assert_eq!(progress.total, 0);
        assert_eq!(progress.completed, 0);
    }

    #[test]
    fn test_real_world_example() {
        let content = r#"# Tasks

## Implementation Tasks

- [x] Create `src/task_parser.rs` module with regex-based task parsing
- [x] Implement `TaskProgress` struct with `completed` and `total` fields
- [ ] Implement `parse_content()` function to parse task markdown content
- [ ] Implement `parse_file()` function to read and parse tasks.md files

## Testing Tasks

1. [ ] Add unit tests for bullet list format
2. [ ] Add unit tests for numbered list format
3. [x] Add unit tests for mixed format

## Validation

- [ ] Run `cargo test` to verify all tests pass
- [ ] Run `cargo clippy` to check for warnings
"#;
        let progress = parse_content(content, None);
        // 4 bullets + 3 numbered + 2 bullets = 9 total
        // 2 checked bullets + 1 checked numbered = 3 completed
        assert_eq!(progress.total, 9);
        assert_eq!(progress.completed, 3);
    }

    // ====================
    // TaskProgress struct tests
    // ====================

    #[test]
    fn test_task_progress_new() {
        let progress = TaskProgress::new();
        assert_eq!(progress.completed, 0);
        assert_eq!(progress.total, 0);
    }

    #[test]
    fn test_task_progress_with_counts() {
        let progress = TaskProgress::with_counts(5, 10);
        assert_eq!(progress.completed, 5);
        assert_eq!(progress.total, 10);
    }

    #[test]
    fn test_task_progress_default() {
        let progress = TaskProgress::default();
        assert_eq!(progress.completed, 0);
        assert_eq!(progress.total, 0);
    }

    // ====================
    // File parsing tests
    // ====================

    #[test]
    fn test_parse_file_not_found() {
        let result = parse_file(Path::new("/nonexistent/path/tasks.md"), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_change_not_found() {
        let result = parse_change("nonexistent-change-id");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_change_with_worktree_fallback_from_worktree() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let worktree_path = temp_dir.path();

        // Create worktree structure
        let change_dir = worktree_path.join("openspec/changes/test-change");
        std::fs::create_dir_all(&change_dir).unwrap();

        // Write tasks.md in worktree
        let tasks_content = "- [x] Task 1\n- [x] Task 2\n- [ ] Task 3";
        std::fs::write(change_dir.join("tasks.md"), tasks_content).unwrap();

        // Parse with worktree
        let result = parse_progress_with_fallback("test-change", Some(worktree_path));
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 2);
        assert_eq!(progress.total, 3);
    }

    // NOTE: These tests change the process cwd; serialize across the whole crate.

    #[test]
    fn test_parse_change_with_worktree_fallback_to_base() {
        use std::env;
        use tempfile::TempDir;

        // Acquire lock to prevent concurrent directory changes
        let _lock = crate::test_support::cwd_lock().lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        // Create base tree structure
        let change_dir = base_path.join("openspec/changes/test-change");
        std::fs::create_dir_all(&change_dir).unwrap();

        // Write tasks.md in base tree
        let tasks_content = "- [x] Task 1\n- [ ] Task 2";
        std::fs::write(change_dir.join("tasks.md"), tasks_content).unwrap();

        // Parse with non-existent worktree (should fallback to base)
        let result = parse_progress_with_fallback("test-change", None);
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.total, 2);

        // Restore directory
        env::set_current_dir(original_dir).unwrap();
    }

    // ====================
    // Archived change parsing tests
    // ====================

    #[test]
    fn test_parse_archived_change_with_worktree_fallback_from_worktree_archive() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let worktree_path = temp_dir.path();

        // Create worktree archive structure
        let archive_dir = worktree_path.join("openspec/changes/archive/test-archived");
        std::fs::create_dir_all(&archive_dir).unwrap();

        // Write tasks.md in worktree archive
        let tasks_content = "- [x] Task 1\n- [x] Task 2\n- [x] Task 3\n- [ ] Task 4";
        std::fs::write(archive_dir.join("tasks.md"), tasks_content).unwrap();

        // Parse with worktree
        let result = parse_progress_with_fallback("test-archived", Some(worktree_path));
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 3);
        assert_eq!(progress.total, 4);
    }

    #[test]
    fn test_parse_archived_change_with_worktree_fallback_from_worktree_active() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let worktree_path = temp_dir.path();

        // Create worktree active change structure (not yet archived in worktree)
        let change_dir = worktree_path.join("openspec/changes/test-prearchive");
        std::fs::create_dir_all(&change_dir).unwrap();

        // Write tasks.md in worktree active location
        let tasks_content = "- [x] Task 1\n- [x] Task 2\n- [ ] Task 3";
        std::fs::write(change_dir.join("tasks.md"), tasks_content).unwrap();

        // Parse with worktree (should find in active location since not in archive yet)
        let result = parse_progress_with_fallback("test-prearchive", Some(worktree_path));
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 2);
        assert_eq!(progress.total, 3);
    }

    #[test]
    fn test_parse_archived_change_with_worktree_fallback_to_base() {
        use std::env;
        use tempfile::TempDir;

        // Acquire lock to prevent concurrent directory changes
        let _lock = crate::test_support::cwd_lock().lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        // Create base tree archive structure
        let archive_dir = base_path.join("openspec/changes/archive/test-base-archive");
        std::fs::create_dir_all(&archive_dir).unwrap();

        // Write tasks.md in base tree archive
        let tasks_content = "- [x] Task 1\n- [ ] Task 2";
        std::fs::write(archive_dir.join("tasks.md"), tasks_content).unwrap();

        // Parse with non-existent worktree (should fallback to base)
        let result = parse_progress_with_fallback("test-base-archive", None);
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.total, 2);

        // Restore directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_parse_archived_change_with_worktree_fallback_priority() {
        use std::env;
        use tempfile::TempDir;

        // Acquire lock to prevent concurrent directory changes
        let _lock = crate::test_support::cwd_lock().lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        // Create both base archive and worktree archive
        let base_archive = base_path.join("openspec/changes/archive/test-priority");
        std::fs::create_dir_all(&base_archive).unwrap();
        std::fs::write(base_archive.join("tasks.md"), "- [ ] Old task").unwrap();

        let worktree_path = base_path.join("worktree");
        let wt_archive = worktree_path.join("openspec/changes/archive/test-priority");
        std::fs::create_dir_all(&wt_archive).unwrap();
        std::fs::write(
            wt_archive.join("tasks.md"),
            "- [x] New task 1\n- [x] New task 2",
        )
        .unwrap();

        // Parse with worktree (should prefer worktree over base)
        let result = parse_progress_with_fallback("test-priority", Some(&worktree_path));
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 2);
        assert_eq!(progress.total, 2);

        // Restore directory
        env::set_current_dir(original_dir).unwrap();
    }

    // ====================
    // Date-prefixed archive tests
    // ====================

    #[test]
    fn test_parse_archived_change_date_prefixed() {
        use std::env;
        use tempfile::TempDir;

        // Acquire lock to prevent concurrent directory changes
        let _lock = crate::test_support::cwd_lock().lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        // Create date-prefixed archive directory
        let archive_dir = base_path.join("openspec/changes/archive/2024-01-15-test-change");
        std::fs::create_dir_all(&archive_dir).unwrap();

        // Write tasks.md in date-prefixed archive
        let tasks_content = "- [x] Task 1\n- [x] Task 2\n- [ ] Task 3";
        std::fs::write(archive_dir.join("tasks.md"), tasks_content).unwrap();

        // Parse should find the date-prefixed directory
        let result = parse_progress_with_fallback("test-change", None);
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 2);
        assert_eq!(progress.total, 3);

        // Restore directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_parse_archived_change_exact_match_preferred() {
        use std::env;
        use tempfile::TempDir;

        // Acquire lock to prevent concurrent directory changes
        let _lock = crate::test_support::cwd_lock().lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        // Create both exact match and date-prefixed archive
        let exact_archive = base_path.join("openspec/changes/archive/test-exact");
        std::fs::create_dir_all(&exact_archive).unwrap();
        std::fs::write(exact_archive.join("tasks.md"), "- [x] Exact task").unwrap();

        let date_archive = base_path.join("openspec/changes/archive/2024-01-15-test-exact");
        std::fs::create_dir_all(&date_archive).unwrap();
        std::fs::write(date_archive.join("tasks.md"), "- [ ] Date task").unwrap();

        // Parse should prefer exact match over date-prefixed
        let result = parse_progress_with_fallback("test-exact", None);
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.total, 1);

        // Restore directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_parse_archived_change_with_worktree_fallback_date_prefixed() {
        use std::env;
        use tempfile::TempDir;

        // Acquire lock to prevent concurrent directory changes
        let _lock = crate::test_support::cwd_lock().lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        // Create date-prefixed archive in base tree
        let base_archive = base_path.join("openspec/changes/archive/2026-01-17-test-date");
        std::fs::create_dir_all(&base_archive).unwrap();
        std::fs::write(
            base_archive.join("tasks.md"),
            "- [x] Task 1\n- [x] Task 2\n- [x] Task 3",
        )
        .unwrap();

        // Parse without worktree (should find date-prefixed archive)
        let result = parse_progress_with_fallback("test-date", None);
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 3);
        assert_eq!(progress.total, 3);

        // Restore directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_find_archive_directory_not_found() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create archive directory but no matching entries
        let archive_dir = base_path.join("openspec/changes/archive");
        std::fs::create_dir_all(&archive_dir).unwrap();

        // Should return None when no match found
        let result = task_file::find_archive_entry("nonexistent", Some(base_path));
        assert!(result.is_none());
    }

    #[test]
    fn test_find_archive_directory_exact_match() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create exact match archive
        let exact_archive = base_path.join("openspec/changes/archive/exact-match");
        std::fs::create_dir_all(&exact_archive).unwrap();

        let result = task_file::find_archive_entry("exact-match", Some(base_path));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), exact_archive);
    }

    #[test]
    fn test_find_archive_directory_date_prefixed() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Create date-prefixed archive
        let date_archive = base_path.join("openspec/changes/archive/2024-01-15-my-feature");
        std::fs::create_dir_all(&date_archive).unwrap();

        let result = task_file::find_archive_entry("my-feature", Some(base_path));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), date_archive);
    }

    #[test]
    fn test_parse_progress_rejects_nested_archive_layout() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let nested_tasks = temp_dir
            .path()
            .join("openspec/changes/archive/2026-07-09/my-feature/tasks.md");
        write_tasks(&nested_tasks, "- [x] archived\n");

        let err = parse_progress_with_fallback("my-feature", Some(temp_dir.path())).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("Invalid archive layout"));
        assert!(message.contains("2026-07-09/my-feature"));
    }

    // ====================
    // Unified fallback helper tests
    // ====================

    #[test]
    fn test_parse_progress_with_fallback_worktree_active() {
        use std::env;
        use tempfile::TempDir;

        // Acquire lock to prevent concurrent directory changes
        let _lock = crate::test_support::cwd_lock().lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        // Create worktree active location
        let worktree_path = base_path.join("worktree");
        let wt_active = worktree_path.join("openspec/changes/test-fallback");
        std::fs::create_dir_all(&wt_active).unwrap();
        std::fs::write(
            wt_active.join("tasks.md"),
            "- [x] Task 1\n- [x] Task 2\n- [ ] Task 3",
        )
        .unwrap();

        // Parse should find worktree active location first
        let result = parse_progress_with_fallback("test-fallback", Some(&worktree_path));
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 2);
        assert_eq!(progress.total, 3);

        // Restore directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_parse_progress_with_fallback_worktree_archive() {
        use std::env;
        use tempfile::TempDir;

        // Acquire lock to prevent concurrent directory changes
        let _lock = crate::test_support::cwd_lock().lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        // Create worktree archive location (no active location)
        let worktree_path = base_path.join("worktree");
        let wt_archive = worktree_path.join("openspec/changes/archive/test-wt-archive");
        std::fs::create_dir_all(&wt_archive).unwrap();
        std::fs::write(
            wt_archive.join("tasks.md"),
            "- [x] Task 1\n- [x] Task 2\n- [x] Task 3\n- [ ] Task 4",
        )
        .unwrap();

        // Parse should find worktree archive location
        let result = parse_progress_with_fallback("test-wt-archive", Some(&worktree_path));
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 3);
        assert_eq!(progress.total, 4);

        // Restore directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_parse_progress_with_fallback_base_archive() {
        use std::env;
        use tempfile::TempDir;

        // Acquire lock to prevent concurrent directory changes
        let _lock = crate::test_support::cwd_lock().lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        // Create base tree archive location (no worktree)
        let base_archive = base_path.join("openspec/changes/archive/test-base-archive");
        std::fs::create_dir_all(&base_archive).unwrap();
        std::fs::write(base_archive.join("tasks.md"), "- [x] Task 1\n- [x] Task 2").unwrap();

        // Parse should find base tree archive location
        let result = parse_progress_with_fallback("test-base-archive", None);
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 2);
        assert_eq!(progress.total, 2);

        // Restore directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_parse_progress_with_fallback_base_active() {
        use std::env;
        use tempfile::TempDir;

        // Acquire lock to prevent concurrent directory changes
        let _lock = crate::test_support::cwd_lock().lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        // Create base tree active location (no archive, no worktree)
        let base_active = base_path.join("openspec/changes/test-base-active");
        std::fs::create_dir_all(&base_active).unwrap();
        std::fs::write(base_active.join("tasks.md"), "- [ ] Task 1").unwrap();

        // Parse should find base tree active location
        let result = parse_progress_with_fallback("test-base-active", None);
        assert!(result.is_ok());
        let progress = result.unwrap();
        assert_eq!(progress.completed, 0);
        assert_eq!(progress.total, 1);

        // Restore directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_parse_progress_with_fallback_priority_order() {
        use std::env;
        use tempfile::TempDir;

        // Acquire lock to prevent concurrent directory changes
        let _lock = crate::test_support::cwd_lock().lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        // Create all locations with different progress values
        let worktree_path = base_path.join("worktree");

        write_tasks(
            worktree_path.join("openspec/changes/test-priority/tasks.md"),
            &checked_tasks(4),
        );
        write_tasks(
            worktree_path.join("openspec/changes/archive/test-priority/tasks.md"),
            &checked_tasks(3),
        );
        write_tasks(
            base_path.join("openspec/changes/archive/test-priority/tasks.md"),
            &checked_tasks(2),
        );
        write_tasks(
            base_path.join("openspec/changes/test-priority/tasks.md"),
            &checked_tasks(1),
        );

        let scenarios = [
            (true, true, true, true, 4),
            (false, true, true, true, 3),
            (false, false, true, true, 2),
            (false, false, false, true, 1),
        ];

        for (worktree_active, worktree_archive, base_archive, base_active, expected_completed) in
            scenarios
        {
            let case_dir = TempDir::new().unwrap();
            let case_base = case_dir.path();
            env::set_current_dir(case_base).unwrap();
            let case_worktree = case_base.join("worktree");

            if worktree_active {
                write_tasks(
                    case_worktree.join("openspec/changes/test-priority/tasks.md"),
                    &checked_tasks(4),
                );
            }
            if worktree_archive {
                write_tasks(
                    case_worktree.join("openspec/changes/archive/test-priority/tasks.md"),
                    &checked_tasks(3),
                );
            }
            if base_archive {
                write_tasks(
                    case_base.join("openspec/changes/archive/test-priority/tasks.md"),
                    &checked_tasks(2),
                );
            }
            if base_active {
                write_tasks(
                    case_base.join("openspec/changes/test-priority/tasks.md"),
                    &checked_tasks(1),
                );
            }

            let progress = parse_progress_with_fallback("test-priority", Some(&case_worktree))
                .expect("progress should resolve from the first available fallback location");
            assert_eq!(progress.completed, expected_completed);
            assert_eq!(progress.total, expected_completed);
        }

        // Restore directory
        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    #[allow(deprecated)]
    fn test_deprecated_parse_change_with_worktree_fallback_preserves_success_and_not_found() {
        use std::env;
        use tempfile::TempDir;

        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        let worktree_path = base_path.join("worktree");
        write_tasks(
            worktree_path.join("openspec/changes/compat-change/tasks.md"),
            "- [x] Worktree\n- [ ] Worktree pending\n",
        );
        write_tasks(
            base_path.join("openspec/changes/compat-change/tasks.md"),
            "- [ ] Base\n",
        );

        let progress = parse_change_with_worktree_fallback("compat-change", Some(&worktree_path))
            .expect("worktree active tasks should be preferred");
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.total, 2);

        let base_progress = parse_change_with_worktree_fallback("compat-change", None)
            .expect("base active tasks should be used without a worktree");
        assert_eq!(base_progress.completed, 0);
        assert_eq!(base_progress.total, 1);

        let missing = parse_change_with_worktree_fallback("missing-change", Some(&worktree_path));
        assert!(missing.is_err());
        assert!(missing
            .unwrap_err()
            .to_string()
            .contains("Tasks file not found"));

        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    #[allow(deprecated)]
    fn test_deprecated_parse_archived_change_preserves_exact_match_and_not_found() {
        use std::env;
        use tempfile::TempDir;

        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        write_tasks(
            base_path.join("openspec/changes/archive/archived-compat/tasks.md"),
            "- [x] Exact archive\n",
        );
        write_tasks(
            base_path.join("openspec/changes/archive/2026-05-13-archived-compat/tasks.md"),
            "- [ ] Date archive\n",
        );

        let progress = parse_archived_change("archived-compat")
            .expect("exact archived tasks should be preferred");
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.total, 1);

        let missing = parse_archived_change("missing-archive");
        assert!(missing.is_err());
        assert!(missing
            .unwrap_err()
            .to_string()
            .contains("Archived directory not found"));

        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    #[allow(deprecated)]
    fn test_deprecated_parse_archived_change_with_worktree_fallback_preserves_order() {
        use std::env;
        use tempfile::TempDir;

        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        let worktree_path = base_path.join("worktree");
        write_tasks(
            worktree_path.join("openspec/changes/archive/compat-archived/tasks.md"),
            &checked_tasks(3),
        );
        write_tasks(
            worktree_path.join("openspec/changes/compat-archived/tasks.md"),
            &checked_tasks(2),
        );
        write_tasks(
            base_path.join("openspec/changes/archive/compat-archived/tasks.md"),
            &checked_tasks(1),
        );

        let progress =
            parse_archived_change_with_worktree_fallback("compat-archived", Some(&worktree_path))
                .expect("worktree archive should be preferred");
        assert_eq!(progress.completed, 3);
        assert_eq!(progress.total, 3);

        let prearchive_dir = TempDir::new().unwrap();
        let prearchive_base = prearchive_dir.path();
        env::set_current_dir(prearchive_base).unwrap();
        let prearchive_worktree = prearchive_base.join("worktree");
        write_tasks(
            prearchive_worktree.join("openspec/changes/compat-archived/tasks.md"),
            &checked_tasks(2),
        );
        write_tasks(
            prearchive_base.join("openspec/changes/archive/compat-archived/tasks.md"),
            &checked_tasks(1),
        );

        let prearchive_progress = parse_archived_change_with_worktree_fallback(
            "compat-archived",
            Some(&prearchive_worktree),
        )
        .expect("worktree active pre-archive tasks should be used before base archive");
        assert_eq!(prearchive_progress.completed, 2);
        assert_eq!(prearchive_progress.total, 2);

        let base_only_dir = TempDir::new().unwrap();
        let base_only = base_only_dir.path();
        env::set_current_dir(base_only).unwrap();
        write_tasks(
            base_only.join("openspec/changes/archive/compat-archived/tasks.md"),
            &checked_tasks(1),
        );

        let base_progress = parse_archived_change_with_worktree_fallback("compat-archived", None)
            .expect("base archive should be used without a worktree");
        assert_eq!(base_progress.completed, 1);
        assert_eq!(base_progress.total, 1);

        let missing = parse_archived_change_with_worktree_fallback("missing-archive", None);
        assert!(missing.is_err());
        assert!(missing
            .unwrap_err()
            .to_string()
            .contains("Archived directory not found"));

        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_parse_progress_with_fallback_not_found() {
        use std::env;
        use tempfile::TempDir;

        // Acquire lock to prevent concurrent directory changes
        let _lock = crate::test_support::cwd_lock().lock().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();

        // Change to temp directory
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(base_path).unwrap();

        // Don't create any locations
        let result = parse_progress_with_fallback("nonexistent", None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not found for change 'nonexistent'"));

        // Restore directory
        env::set_current_dir(original_dir).unwrap();
    }

    // --- Runtime-owned follow-up: immutable identity + Apply remediation claims ---

    fn structured_finding(id: &str) -> crate::acceptance::AcceptanceFinding {
        crate::acceptance::AcceptanceFinding::structured(crate::acceptance::RepositoryFinding {
            id: id.to_string(),
            severity: crate::acceptance::FindingSeverity::Minor,
            summary: "Challenge and proof leakage is not tested by value".to_string(),
            evidence: vec!["tests/support/relay.ts exposes counts but not values".to_string()],
            required_changes: vec![crate::acceptance::FindingFileExpectation {
                file: "tests/support/relay.ts".to_string(),
                description: "Expose issued challenge and presented proof values".to_string(),
            }],
            verification: vec![crate::acceptance::FindingFileExpectation {
                file: "runtime/recovery.integration.test.ts".to_string(),
                description: "Assert recorded values are absent from audit output".to_string(),
            }],
        })
    }

    #[test]
    fn follow_up_persists_structured_payload_for_interruption_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(temp.path());
        write_tasks(&tasks_path, "## Implementation Tasks\n- [x] Task 1\n");

        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            2,
            &[structured_finding("acceptance-secret-value-scan")],
        )
        .unwrap();

        // Restart path: everything Apply needs comes back from workspace-local
        // evidence, not from lost in-memory state.
        let (attempt, recovered) = read_acceptance_follow_up(&tasks_path).unwrap().unwrap();
        assert_eq!(attempt, 2);
        assert_eq!(recovered.len(), 1);
        let structured = recovered[0]
            .structured_payload()
            .expect("structured payload recovered from workspace evidence");
        assert_eq!(structured.id, "acceptance-secret-value-scan");
        assert_eq!(structured.required_files(), ["tests/support/relay.ts"]);
        assert_eq!(
            structured.verification_files(),
            ["runtime/recovery.integration.test.ts"]
        );
        assert_eq!(
            structured.evidence,
            ["tests/support/relay.ts exposes counts but not values"]
        );
    }

    #[test]
    fn apply_checkbox_is_a_remediation_claim_and_cannot_close_a_finding() {
        let temp = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(temp.path());
        write_tasks(&tasks_path, "## Implementation Tasks\n- [x] Task 1\n");
        let finding = structured_finding("acceptance-secret-value-scan");

        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            1,
            std::slice::from_ref(&finding),
        )
        .unwrap();

        // Apply marks the box and adds evidence.
        let claimed = std::fs::read_to_string(&tasks_path).unwrap().replace(
            "- [ ] [acceptance-secret-value-scan]",
            "- [x] [acceptance-secret-value-scan]",
        );
        std::fs::write(
            &tasks_path,
            format!("{claimed}  evidence: exposed issued values\n"),
        )
        .unwrap();

        // A later FAIL reporting the same ID reopens it: the claim never closed
        // it, and the reviewer's payload is restored verbatim.
        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            2,
            std::slice::from_ref(&finding),
        )
        .unwrap();
        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(
            content.contains("- [ ] [acceptance-secret-value-scan]"),
            "{content}"
        );
        let (_, recovered) = read_acceptance_follow_up(&tasks_path).unwrap().unwrap();
        assert_eq!(recovered, vec![finding]);
    }

    #[test]
    fn apply_hydration_preserves_the_claim_but_not_the_authority_to_rewrite_a_finding() {
        let temp = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(temp.path());
        write_tasks(&tasks_path, "## Implementation Tasks\n- [x] Task 1\n");
        let finding = structured_finding("acceptance-secret-value-scan");
        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            1,
            std::slice::from_ref(&finding),
        )
        .unwrap();

        // Apply rewrites the checkbox text to something easier and claims it.
        let tampered = std::fs::read_to_string(&tasks_path)
            .unwrap()
            .lines()
            .map(|line| {
                if line.starts_with("- [ ] [acceptance-secret-value-scan]") {
                    "- [x] tidied up a comment".to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&tasks_path, format!("{tampered}\n  evidence: reworded\n")).unwrap();

        merge_acceptance_follow_up_apply_progress(&tasks_path, 1, std::slice::from_ref(&finding))
            .unwrap();

        let (_, recovered) = read_acceptance_follow_up(&tasks_path).unwrap().unwrap();
        assert_eq!(
            recovered,
            vec![finding],
            "runtime restores the reviewer payload over Apply-authored text"
        );
        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(!content.contains("tidied up a comment"), "{content}");
    }

    #[test]
    fn absent_finding_id_is_removed_only_by_a_new_canonical_result() {
        let temp = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(temp.path());
        write_tasks(&tasks_path, "## Implementation Tasks\n- [x] Task 1\n");
        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            1,
            &[structured_finding("first-id")],
        )
        .unwrap();

        // The next canonical FAIL no longer reports `first-id`: acceptance
        // closed it, so runtime drops it and grants the new ID its own item.
        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            2,
            &[structured_finding("second-id")],
        )
        .unwrap();

        let (_, recovered) = read_acceptance_follow_up(&tasks_path).unwrap().unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].id(), Some("second-id"));
    }

    #[test]
    fn invalid_finding_metadata_is_ignored_rather_than_trusted() {
        let temp = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(temp.path());
        write_tasks(
            &tasks_path,
            "## Implementation Tasks\n- [x] Task 1\n\n\
             ## Current Acceptance Follow-up\n- attempt: 1\n\
             - [ ] legacy style finding at src/a.rs:10\n\
             \x20 finding: {\"id\":\"forged\"}\n",
        );

        let (_, recovered) = read_acceptance_follow_up(&tasks_path).unwrap().unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(
            recovered[0].id(),
            None,
            "a corrupted payload must not fabricate a finding ID"
        );
        assert_eq!(recovered[0].text(), "legacy style finding at src/a.rs:10");
    }

    #[test]
    fn legacy_string_findings_keep_their_existing_follow_up_shape() {
        let temp = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(temp.path());
        write_tasks(&tasks_path, "## Implementation Tasks\n- [x] Task 1\n");
        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            1,
            &crate::acceptance::legacy_findings(["src/a.rs:10 missing regression coverage"]),
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(
            content.contains("- [ ] src/a.rs:10 missing regression coverage"),
            "{content}"
        );
        assert!(
            !content.contains(FINDING_PAYLOAD_PREFIX.trim()),
            "legacy findings declare no structured payload: {content}"
        );
    }

    #[test]
    fn remediation_evidence_is_readable_for_stop_diagnostics() {
        let temp = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(temp.path());
        write_tasks(&tasks_path, "## Implementation Tasks\n- [x] Task 1\n");
        replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            1,
            &[structured_finding("acceptance-secret-value-scan")],
        )
        .unwrap();
        let content = std::fs::read_to_string(&tasks_path).unwrap();
        std::fs::write(
            &tasks_path,
            format!("{content}  evidence: exposed issued values in relay.ts\n"),
        )
        .unwrap();

        assert_eq!(
            read_acceptance_follow_up_evidence(&tasks_path).unwrap(),
            ["exposed issued values in relay.ts"]
        );
    }
}

/// Unit coverage for acceptance follow-up recovery.
///
/// These cases exercise the pure planning functions only: no filesystem, VCS,
/// process, clock, or network boundary participates, so recovery, preservation,
/// deduplication, and hard-error behavior are verified in isolation.
#[cfg(test)]
mod recovery_tests {
    use super::tests::markdown_task_file;
    use super::*;

    const DRIFTED_FOLLOW_UP: &str = concat!(
        "## Implementation Tasks\n",
        "- [x] done\n",
        "\n",
        "## Current Acceptance Follow-up\n",
        "- attempt: 1\n",
        "- [x] [SAME_FINDING] fixed wording\n",
        "  Evidence: ran `cargo test`\n",
        "### Reviewer notes\n",
        "First unknown paragraph.\n",
        "\n",
        "Second paragraph with ``inline`` runs and - [ ] checkbox text.\n",
    );

    const UNKNOWN_PAYLOAD: &str = concat!(
        "### Reviewer notes\n",
        "First unknown paragraph.\n",
        "\n",
        "Second paragraph with ``inline`` runs and - [ ] checkbox text."
    );

    fn replace(content: &str) -> (String, FollowUpRecovery) {
        plan_replace_acceptance_follow_up(content, 2, &["[SAME_FINDING] still broken".into()])
            .expect("replacement plan succeeds")
    }

    #[test]
    fn unknown_follow_up_content_is_recovered_instead_of_terminating_the_workflow() {
        let (content, recovery) = replace(DRIFTED_FOLLOW_UP);

        assert_eq!(recovery.recovered_blocks, 1);
        assert_eq!(recovery.recovered_bytes, UNKNOWN_PAYLOAD.len());
        assert!(recovery.warning().is_some());
        // Unknown bytes are preserved exactly, inside a fence longer than the
        // longest backtick run in the payload.
        assert!(content.contains(&format!(
            "{RECOVERED_NOTES_HEADING}\n\n{RECOVERED_NOTES_NOTICE}\n\n```text\n{UNKNOWN_PAYLOAD}\n```\n"
        )));
        // Runtime content is regenerated, not recovered.
        assert!(content.contains("## Current Acceptance Follow-up\n- attempt: 2\n"));
        assert!(content.contains("- [ ] [SAME_FINDING] still broken"));
        assert!(!content.contains("- attempt: 1"));
        // Ordinary content is untouched and recovered notes precede the follow-up.
        assert!(content.starts_with("## Implementation Tasks\n- [x] done\n"));
        assert!(content.find(RECOVERED_NOTES_HEADING) < content.find(ACCEPTANCE_FOLLOW_UP_HEADING));
    }

    #[test]
    fn runtime_metadata_capitalization_drift_stays_runtime_owned() {
        let drifted = concat!(
            "## Current Acceptance Follow-up\n",
            "- Attempt: 1\n",
            "- [X] fixed\n",
            "  EVIDENCE: cargo test passed\n",
            "### External Blockers\n",
            "- Identity: `external|api`\n",
            "  Next action: Resolve the external prerequisite.\n",
        );

        let (content, recovery) = replace(drifted);

        assert_eq!(recovery, FollowUpRecovery::default());
        assert!(!content.contains(RECOVERED_NOTES_HEADING));
    }

    #[test]
    fn repeated_normalization_and_restart_do_not_duplicate_recovered_notes() {
        let (first, first_recovery) = replace(DRIFTED_FOLLOW_UP);
        assert_eq!(first_recovery.recovered_blocks, 1);

        // Retry replacement, apply-side merge, and a restart that re-reads the
        // file all converge on the same bytes.
        let (second, second_recovery) = replace(&first);
        assert_eq!(second_recovery, FollowUpRecovery::default());
        assert_eq!(second.matches(RECOVERED_NOTES_HEADING).count(), 1);
        assert_eq!(second.matches(UNKNOWN_PAYLOAD).count(), 1);

        let (merged, merge_recovery) =
            plan_merge_acceptance_follow_up(&second, 2, &["[SAME_FINDING] still broken".into()])
                .unwrap();
        assert_eq!(merge_recovery, FollowUpRecovery::default());
        assert_eq!(merged.matches(UNKNOWN_PAYLOAD).count(), 1);

        let (restarted, restart_recovery) = replace(&merged);
        assert_eq!(restart_recovery, FollowUpRecovery::default());
        assert_eq!(restarted, second);
    }

    #[test]
    fn distinct_unknown_payloads_accumulate_as_separate_blocks() {
        let (first, _) = replace(DRIFTED_FOLLOW_UP);
        let with_new_drift = format!("{first}\nA brand new unknown note.\n");

        let (second, recovery) = replace(&with_new_drift);

        assert_eq!(recovery.recovered_blocks, 1);
        assert_eq!(second.matches(RECOVERED_NOTES_HEADING).count(), 1);
        assert!(second.contains(UNKNOWN_PAYLOAD));
        assert!(second.contains("A brand new unknown note."));
    }

    #[test]
    fn pass_cleanup_removes_runtime_section_and_retains_recovered_notes() {
        let (with_notes, _) = replace(DRIFTED_FOLLOW_UP);

        let (cleaned, recovery) = plan_clear_acceptance_follow_up(&with_notes).unwrap();

        assert_eq!(recovery, FollowUpRecovery::default());
        assert!(!cleaned.contains(ACCEPTANCE_FOLLOW_UP_HEADING));
        assert!(cleaned.contains(RECOVERED_NOTES_HEADING));
        assert!(cleaned.contains(UNKNOWN_PAYLOAD));
        assert!(cleaned.starts_with("## Implementation Tasks\n- [x] done\n"));
    }

    #[test]
    fn recovered_fence_is_longer_than_the_longest_backtick_run() {
        let payload_source = concat!(
            "## Current Acceptance Follow-up\n",
            "- attempt: 1\n",
            "`````\n",
            "````\n",
            "not a runtime record\n",
            "````\n",
            "`````\n",
        );

        let (content, recovery) = replace(payload_source);

        assert_eq!(recovery.recovered_blocks, 1);
        assert!(content.contains("``````text\n"));
        assert!(content.contains("`````\n````\nnot a runtime record\n````\n`````\n``````\n"));
        // The recovered literal round-trips: a second pass finds no new payload.
        let (again, again_recovery) = replace(&content);
        assert_eq!(again_recovery, FollowUpRecovery::default());
        assert_eq!(again, content);
    }

    #[test]
    fn recovered_checkbox_text_is_inert_for_task_progress() {
        let (content, _) = replace(DRIFTED_FOLLOW_UP);

        // `- [x] done` plus the regenerated runtime finding; the recovered
        // `- [ ] checkbox text` line inside the fence must not count.
        assert_eq!(
            parse_content(&content, None),
            TaskProgress::with_counts(1, 2)
        );
    }

    #[test]
    fn task_progress_ignores_dynamic_and_tilde_fences() {
        let content = concat!(
            "- [x] real task\n",
            "~~~~\n",
            "- [x] fenced\n",
            "~~~\n",
            "- [ ] still fenced\n",
            "~~~~\n",
            "````md\n",
            "```\n",
            "- [x] fenced\n",
            "```\n",
            "````\n",
            "- [ ] second real task\n",
        );

        assert_eq!(
            parse_content(content, None),
            TaskProgress::with_counts(1, 2)
        );
    }

    #[test]
    fn unclosed_fence_is_a_hard_error_that_changes_nothing() {
        let ambiguous = concat!(
            "## Implementation Tasks\n",
            "- [x] done\n",
            "```md\n",
            "## Current Acceptance Follow-up\n",
            "- [ ] finding\n",
        );

        for error in [
            plan_replace_acceptance_follow_up(ambiguous, 2, &["finding".into()]).unwrap_err(),
            plan_merge_acceptance_follow_up(ambiguous, 2, &["finding".into()]).unwrap_err(),
            plan_clear_acceptance_follow_up(ambiguous).unwrap_err(),
        ] {
            let message = error.to_string();
            assert!(message.contains("unclosed code fence"), "{message}");
            assert!(
                message.contains("boundary cannot be determined safely"),
                "{message}"
            );
        }
    }

    #[test]
    fn unclosed_fence_inside_the_follow_up_section_is_a_hard_error() {
        let ambiguous = concat!(
            "## Current Acceptance Follow-up\n",
            "- attempt: 1\n",
            "- [ ] finding\n",
            "```text\n",
            "unterminated reviewer dump\n",
        );

        let error = plan_clear_acceptance_follow_up(ambiguous).unwrap_err();
        assert!(error.to_string().contains("unclosed code fence"));
    }

    #[test]
    fn follow_up_headings_inside_fences_are_never_treated_as_runtime_sections() {
        let documented = concat!(
            "## Notes\n",
            "```md\n",
            "## Current Acceptance Follow-up\n",
            "- [ ] example\n",
            "```\n",
        );

        let (cleaned, recovery) = plan_clear_acceptance_follow_up(documented).unwrap();

        assert_eq!(recovery, FollowUpRecovery::default());
        assert_eq!(cleaned, documented);
    }

    #[test]
    fn atomic_update_leaves_the_original_file_unchanged_when_staging_fails() {
        // Filesystem-boundary check for the atomic writer: the temporary file
        // cannot be created in a read-only directory, so no partial write or
        // truncation can reach `tasks.md`.
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = markdown_task_file(dir.path());
        std::fs::write(&tasks_path, DRIFTED_FOLLOW_UP).unwrap();

        let mut permissions = std::fs::metadata(dir.path()).unwrap().permissions();
        let original_mode = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let mode = permissions.mode();
                permissions.set_mode(0o500);
                mode
            }
            #[cfg(not(unix))]
            {
                permissions.set_readonly(true);
                0
            }
        };
        std::fs::set_permissions(dir.path(), permissions).unwrap();

        let error = replace_acceptance_follow_up_from_latest_fail(
            &tasks_path,
            2,
            &["still broken".to_string().into()],
        )
        .unwrap_err();

        let mut restore = std::fs::metadata(dir.path()).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            restore.set_mode(original_mode);
        }
        #[cfg(not(unix))]
        {
            restore.set_readonly(false);
        }
        std::fs::set_permissions(dir.path(), restore).unwrap();

        assert!(error
            .to_string()
            .contains("Failed to stage atomic tasks update"));
        assert_eq!(
            std::fs::read_to_string(&tasks_path).unwrap(),
            DRIFTED_FOLLOW_UP
        );
    }
}

//! Native task progress parsing for tasks.md files.
//!
//! This module provides native parsing of task checkboxes in markdown files,
//! supporting both bullet lists (`- [ ]`) and numbered lists (`1. [ ]`).

use crate::archive_layout;
use crate::error::{OrchestratorError, Result};
use crate::tui::log_deduplicator;
use regex::Regex;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
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
/// Returns the count of completed and total tasks.
/// When change_id is provided, emits deduplicated debug logs.
pub fn parse_content(content: &str, change_id: Option<&str>) -> TaskProgress {
    let regex = task_regex();
    let mut progress = TaskProgress::new();

    for line in content.lines() {
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

/// Parse task progress from a file.
///
/// Reads the file content and parses it for task checkboxes.
/// When change_id is provided, emits deduplicated debug logs.
pub fn parse_file(path: &Path, change_id: Option<&str>) -> Result<TaskProgress> {
    let content = read_tasks_file(path)?;
    Ok(parse_content(&content, change_id))
}

fn read_tasks_file(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| {
        OrchestratorError::ConfigLoad(format!("Failed to read tasks file {:?}: {}", path, e))
    })
}

fn write_tasks_file(path: &Path, content: String) -> Result<()> {
    std::fs::write(path, content).map_err(|e| {
        OrchestratorError::ConfigLoad(format!("Failed to write tasks file {:?}: {}", path, e))
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskProgressLocationKind {
    WorktreeActive,
    WorktreeArchive,
    BaseArchive,
    BaseActive,
}

impl TaskProgressLocationKind {
    fn log_label(self) -> &'static str {
        match self {
            Self::WorktreeActive => "worktree active location",
            Self::WorktreeArchive => "worktree archive location",
            Self::BaseArchive => "base tree archive location",
            Self::BaseActive => "base tree active location",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskProgressLocation {
    kind: TaskProgressLocationKind,
    tasks_path: PathBuf,
}

impl TaskProgressLocation {
    fn new(kind: TaskProgressLocationKind, tasks_path: PathBuf) -> Self {
        Self { kind, tasks_path }
    }
}

fn active_tasks_path(root: Option<&Path>, change_id: &str) -> PathBuf {
    root.unwrap_or_else(|| Path::new(""))
        .join("openspec/changes")
        .join(change_id)
        .join("tasks.md")
}

fn archived_tasks_path(change_id: &str, root: Option<&Path>) -> Option<PathBuf> {
    find_archive_directory(change_id, root)
        .map(|archive_path| archive_path.join("tasks.md"))
        .filter(|tasks_path| tasks_path.exists())
}

fn resolve_progress_location(
    change_id: &str,
    worktree_path: Option<&Path>,
) -> Option<TaskProgressLocation> {
    progress_location_candidates(change_id, worktree_path)
        .into_iter()
        .find(|candidate| candidate.tasks_path.exists())
}

fn progress_location_candidates(
    change_id: &str,
    worktree_path: Option<&Path>,
) -> Vec<TaskProgressLocation> {
    let mut candidates = Vec::new();

    if let Some(wt_path) = worktree_path {
        candidates.push(TaskProgressLocation::new(
            TaskProgressLocationKind::WorktreeActive,
            active_tasks_path(Some(wt_path), change_id),
        ));
        if let Some(tasks_path) = archived_tasks_path(change_id, Some(wt_path)) {
            candidates.push(TaskProgressLocation::new(
                TaskProgressLocationKind::WorktreeArchive,
                tasks_path,
            ));
        }
    }

    if let Some(tasks_path) = archived_tasks_path(change_id, None) {
        candidates.push(TaskProgressLocation::new(
            TaskProgressLocationKind::BaseArchive,
            tasks_path,
        ));
    }

    candidates.push(TaskProgressLocation::new(
        TaskProgressLocationKind::BaseActive,
        active_tasks_path(None, change_id),
    ));

    candidates
}

fn resolve_active_progress_location(
    change_id: &str,
    worktree_path: Option<&Path>,
) -> Option<TaskProgressLocation> {
    worktree_path
        .map(|wt_path| {
            TaskProgressLocation::new(
                TaskProgressLocationKind::WorktreeActive,
                active_tasks_path(Some(wt_path), change_id),
            )
        })
        .filter(|candidate| candidate.tasks_path.exists())
        .or_else(|| {
            let candidate = TaskProgressLocation::new(
                TaskProgressLocationKind::BaseActive,
                active_tasks_path(None, change_id),
            );
            candidate.tasks_path.exists().then_some(candidate)
        })
}

fn resolve_archived_progress_location(
    change_id: &str,
    worktree_path: Option<&Path>,
) -> Option<TaskProgressLocation> {
    if let Some(wt_path) = worktree_path {
        if let Some(tasks_path) = archived_tasks_path(change_id, Some(wt_path)) {
            return Some(TaskProgressLocation::new(
                TaskProgressLocationKind::WorktreeArchive,
                tasks_path,
            ));
        }

        let active_candidate = TaskProgressLocation::new(
            TaskProgressLocationKind::WorktreeActive,
            active_tasks_path(Some(wt_path), change_id),
        );
        if active_candidate.tasks_path.exists() {
            return Some(active_candidate);
        }
    }

    archived_tasks_path(change_id, None).map(|tasks_path| {
        TaskProgressLocation::new(TaskProgressLocationKind::BaseArchive, tasks_path)
    })
}

/// Parse task progress for a change by its ID.
///
/// Looks for tasks.md at `openspec/changes/{change_id}/tasks.md`.
pub fn parse_change(change_id: &str) -> Result<TaskProgress> {
    let tasks_path = active_tasks_path(None, change_id);

    if !tasks_path.exists() {
        return Err(OrchestratorError::ConfigLoad(format!(
            "Tasks file not found: {:?}",
            tasks_path
        )));
    }

    parse_file(&tasks_path, Some(change_id))
}

/// Parse task progress with worktree priority and base tree fallback.
///
/// Resolution order:
/// 1. Try worktree_path/openspec/changes/{change_id}/tasks.md (uncommitted)
/// 2. Fallback to openspec/changes/{change_id}/tasks.md (base tree)
///
/// This function is designed for TUI auto-refresh to read the latest progress
/// from worktrees where AI agents update tasks.md before committing.
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
    if let Some(location) = resolve_active_progress_location(change_id, worktree_path) {
        debug!(
            "Reading tasks from {}: {:?}",
            location.kind.log_label(),
            location.tasks_path
        );
        return parse_file(&location.tasks_path, Some(change_id));
    }

    let tasks_path = active_tasks_path(None, change_id);
    Err(OrchestratorError::ConfigLoad(format!(
        "Tasks file not found: {:?}",
        tasks_path
    )))
}

/// Find the archive directory entry for a change.
///
/// Searches for an archive directory matching either:
/// - `{change_id}` - Simple archive
/// - `{date}-{change_id}` - Date-prefixed archive (e.g., `2024-01-15-add-feature`)
///
/// Returns the path to the archive directory if found.
fn archive_root(base_path: Option<&Path>) -> PathBuf {
    match base_path {
        Some(base) => base.join("openspec/changes/archive"),
        None => Path::new("openspec/changes/archive").to_path_buf(),
    }
}

fn invalid_archive_layout_error(change_id: &str, base_path: Option<&Path>) -> Option<String> {
    archive_layout::invalid_layout_error(change_id, &archive_root(base_path)).map(|e| e.message())
}

fn find_archive_directory(change_id: &str, base_path: Option<&Path>) -> Option<std::path::PathBuf> {
    archive_layout::find_valid_archive_entry(change_id, &archive_root(base_path))
}

/// Parse task progress from the archive directory.
///
/// Looks for tasks.md at `openspec/changes/archive/{change_id}/tasks.md` or
/// `openspec/changes/archive/{date}-{change_id}/tasks.md` (date-prefixed format).
/// This is used to retrieve final progress for archived changes when
/// the change is no longer in the active changes directory.
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
    if let Some(message) = invalid_archive_layout_error(change_id, None) {
        return Err(OrchestratorError::ConfigLoad(message));
    }

    let location = resolve_archived_progress_location(change_id, None).ok_or_else(|| {
        let archive_root = Path::new("openspec/changes/archive");
        if find_archive_directory(change_id, None).is_some() {
            OrchestratorError::ConfigLoad(format!(
                "Archived tasks file not found for change '{}' in {:?}",
                change_id, archive_root
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
        location.kind.log_label(),
        location.tasks_path
    );
    parse_file(&location.tasks_path, Some(change_id))
}

/// Parse task progress with worktree priority for archived changes.
///
/// Resolution order when worktree_path is provided:
/// 1. Try worktree_path/openspec/changes/archive/{change_id}/tasks.md or date-prefixed (archived in worktree)
/// 2. Try worktree_path/openspec/changes/{change_id}/tasks.md (not yet archived in worktree)
/// 3. Fallback to openspec/changes/archive/{change_id}/tasks.md or date-prefixed (base tree)
///
/// Resolution order when worktree_path is None:
/// 1. Try openspec/changes/archive/{change_id}/tasks.md or date-prefixed (base tree)
///
/// This function is designed for Archived/Merged changes in TUI auto-refresh to read
/// the latest progress from worktrees where the archive may not yet be committed.
///
/// # Deprecated
/// Use [`parse_progress_with_fallback`] instead, which provides comprehensive
/// fallback order: worktree → archive → base.
#[deprecated(
    since = "0.3.0",
    note = "Use parse_progress_with_fallback for comprehensive fallback order"
)]
#[allow(dead_code)]
#[allow(deprecated)]
pub fn parse_archived_change_with_worktree_fallback(
    change_id: &str,
    worktree_path: Option<&Path>,
) -> Result<TaskProgress> {
    if let Some(wt_path) = worktree_path {
        if let Some(message) = invalid_archive_layout_error(change_id, Some(wt_path)) {
            return Err(OrchestratorError::ConfigLoad(message));
        }
    }
    if let Some(message) = invalid_archive_layout_error(change_id, None) {
        return Err(OrchestratorError::ConfigLoad(message));
    }

    let location =
        resolve_archived_progress_location(change_id, worktree_path).ok_or_else(|| {
            OrchestratorError::ConfigLoad(format!(
                "Archived directory not found for change '{}' in openspec/changes/archive/",
                change_id
            ))
        })?;

    debug!(
        "Reading archived tasks from {}: {:?}",
        location.kind.log_label(),
        location.tasks_path
    );
    parse_file(&location.tasks_path, Some(change_id))
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
        if let Some(message) = invalid_archive_layout_error(change_id, Some(wt_path)) {
            return Err(OrchestratorError::ConfigLoad(message));
        }
    }
    if let Some(message) = invalid_archive_layout_error(change_id, None) {
        return Err(OrchestratorError::ConfigLoad(message));
    }

    if let Some(location) = resolve_progress_location(change_id, worktree_path) {
        debug!(
            "Reading progress from {}: {:?}",
            location.kind.log_label(),
            location.tasks_path
        );
        return parse_file(&location.tasks_path, Some(change_id));
    }

    Err(OrchestratorError::ConfigLoad(format!(
        "Tasks file not found for change '{}' in any location (worktree, archive, or base tree)",
        change_id
    )))
}

const ACCEPTANCE_FOLLOW_UP_HEADING: &str = "## Current Acceptance Follow-up";

fn normalize_acceptance_findings(
    findings: &[String],
) -> Vec<crate::orchestration::acceptance::NormalizedFinding> {
    let mut normalized = crate::orchestration::acceptance::normalize_findings(findings);
    if normalized.is_empty() {
        normalized = crate::orchestration::acceptance::normalize_findings(&[String::from(
            "Investigate acceptance failure and apply the required fix",
        )]);
    }
    normalized
}

fn acceptance_finding_identity(finding: &str) -> &str {
    finding
        .strip_prefix('[')
        .and_then(|rest| rest.find(']').map(|end| &finding[..=end + 1]))
        .unwrap_or(finding)
}

fn render_acceptance_follow_up_section(
    attempt: u32,
    findings: &[crate::orchestration::acceptance::NormalizedFinding],
    completed_identities: &[String],
) -> String {
    let mut section = format!("- attempt: {attempt}\n");
    for finding in findings.iter().filter(|finding| !finding.external) {
        let checked = if completed_identities.contains(&finding.identity) {
            "x"
        } else {
            " "
        };
        let _ = writeln!(&mut section, "- [{checked}] {}", finding.text);
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

fn acceptance_follow_up_ranges(content: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut offset = 0;
    let mut section_start = None;
    let mut code_fence = None;

    for line in content.split_inclusive('\n') {
        let heading = line.trim_end_matches(['\r', '\n']);
        if let Some((marker, length, empty_remainder)) = markdown_fence(heading) {
            match code_fence {
                Some((open_marker, open_length))
                    if marker == open_marker && length >= open_length && empty_remainder =>
                {
                    code_fence = None;
                }
                None => code_fence = Some((marker, length)),
                _ => {}
            }
        } else if code_fence.is_none() && heading.starts_with("## ") {
            if let Some(start) = section_start.take() {
                ranges.push(start..offset);
            }
            if heading == ACCEPTANCE_FOLLOW_UP_HEADING
                || (heading.starts_with("## Acceptance #")
                    && heading.ends_with(" Failure Follow-up"))
            {
                section_start = Some(offset);
            }
        }
        offset += line.len();
    }

    if let Some(start) = section_start {
        ranges.push(start..content.len());
    }
    ranges
}

fn remove_acceptance_follow_up_sections(content: &mut String) -> Result<()> {
    let ranges = acceptance_follow_up_ranges(content);
    for range in &ranges {
        let section = &content[range.clone()];
        let current = section.lines().next() == Some(ACCEPTANCE_FOLLOW_UP_HEADING);
        let mut lines = section.lines();
        let _ = lines.next();
        if lines.any(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with("- [ ] ")
                && !trimmed.starts_with("- [x] ")
                && !trimmed.starts_with("- [X] ")
                && !(current
                    && (trimmed.starts_with("- attempt: ")
                        || trimmed == "### External blockers"
                        || trimmed.starts_with("- identity: ")
                        || trimmed.starts_with("evidence: ")
                        || trimmed.starts_with("next action: ")))
        }) {
            return Err(OrchestratorError::ConfigLoad(
                "Acceptance follow-up contains non-runtime content; refusing destructive update"
                    .to_string(),
            ));
        }
    }
    for range in ranges.into_iter().rev() {
        content.replace_range(range, "");
    }
    while content.ends_with("\n\n\n") {
        content.pop();
    }
    Ok(())
}

fn upsert_acceptance_follow_up_section(
    content: &mut String,
    attempt: u32,
    findings: &[crate::orchestration::acceptance::NormalizedFinding],
    completed_identities: &[String],
) -> Result<()> {
    remove_acceptance_follow_up_sections(content)?;
    if !content.ends_with('\n') {
        content.push('\n');
    }
    if !content.ends_with("\n\n") {
        content.push('\n');
    }
    let _ = writeln!(content, "{ACCEPTANCE_FOLLOW_UP_HEADING}");
    content.push_str(&render_acceptance_follow_up_section(
        attempt,
        findings,
        completed_identities,
    ));
    Ok(())
}

pub fn resolve_acceptance_follow_up_tasks_path(
    change_id: &str,
    worktree_path: &Path,
) -> Result<std::path::PathBuf> {
    let active_path = worktree_path
        .join("openspec")
        .join("changes")
        .join(change_id)
        .join("tasks.md");

    if active_path.exists() {
        return Ok(active_path);
    }

    if let Some(message) = invalid_archive_layout_error(change_id, Some(worktree_path)) {
        return Err(OrchestratorError::ConfigLoad(message));
    }

    if let Some(archive_path) = find_archive_directory(change_id, Some(worktree_path)) {
        let archive_tasks = archive_path.join("tasks.md");
        if archive_tasks.exists() {
            return Ok(archive_tasks);
        }
    }

    Err(OrchestratorError::ConfigLoad(format!(
        "Acceptance follow-up tasks path not found for change '{}' under worktree '{}'",
        change_id,
        worktree_path.display()
    )))
}

pub fn resolve_acceptance_follow_up_tasks_path_for_cleanup(
    change_id: &str,
    worktree_path: &Path,
) -> Result<Option<std::path::PathBuf>> {
    let active_path = worktree_path
        .join("openspec")
        .join("changes")
        .join(change_id)
        .join("tasks.md");
    if active_path.exists() {
        return Ok(Some(active_path));
    }
    if let Some(message) = invalid_archive_layout_error(change_id, Some(worktree_path)) {
        return Err(OrchestratorError::ConfigLoad(message));
    }
    Ok(find_archive_directory(change_id, Some(worktree_path))
        .map(|archive_path| archive_path.join("tasks.md"))
        .filter(|tasks_path| tasks_path.exists()))
}

pub fn record_acceptance_follow_up(
    tasks_path: &Path,
    attempt: u32,
    findings: &[String],
) -> Result<()> {
    let mut content = read_tasks_file(tasks_path)?;
    let normalized_findings = normalize_acceptance_findings(findings);

    upsert_acceptance_follow_up_section(&mut content, attempt, &normalized_findings, &[])?;
    write_tasks_file(tasks_path, content)
}

pub fn ensure_acceptance_follow_up(
    tasks_path: &Path,
    attempt: u32,
    findings: &[String],
) -> Result<()> {
    let mut content = read_tasks_file(tasks_path)?;
    let normalized_findings = normalize_acceptance_findings(findings);
    let existing_section = acceptance_follow_up_ranges(&content)
        .first()
        .map(|range| content[range.clone()].to_string())
        .unwrap_or_default();
    let existing_tasks = existing_section
        .lines()
        .filter_map(|line| {
            ["- [ ] ", "- [x] ", "- [X] "]
                .iter()
                .position(|prefix| line.starts_with(prefix))
                .map(|index| (index > 0, &line[6..]))
        })
        .collect::<Vec<_>>();
    let completed_identities = normalized_findings
        .iter()
        .filter(|candidate| {
            existing_tasks.iter().any(|(completed, finding)| {
                *completed
                    && (candidate.identity.ends_with(&format!(
                        "|code|{}",
                        acceptance_finding_identity(finding).to_ascii_lowercase()
                    )) || candidate.text == *finding)
            })
        })
        .map(|candidate| candidate.identity.clone())
        .collect::<Vec<_>>();

    upsert_acceptance_follow_up_section(
        &mut content,
        attempt,
        &normalized_findings,
        &completed_identities,
    )?;
    write_tasks_file(tasks_path, content)
}

pub fn read_acceptance_follow_up(tasks_path: &Path) -> Result<Option<(u32, Vec<String>)>> {
    let content = read_tasks_file(tasks_path)?;
    let ranges = acceptance_follow_up_ranges(&content);
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
    let mut findings = Vec::new();
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
            findings.push(finding.to_string());
        } else if in_external_blockers {
            if let Some(evidence) = trimmed.strip_prefix("evidence: ") {
                findings.push(evidence.to_string());
            }
        }
    }
    Ok((!findings.is_empty()).then_some((attempt, findings)))
}

pub fn clear_acceptance_follow_up(tasks_path: &Path) -> Result<()> {
    let mut content = read_tasks_file(tasks_path)?;
    remove_acceptance_follow_up_sections(&mut content)?;
    write_tasks_file(tasks_path, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tasks(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().expect("tasks path has a parent")).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn checked_tasks(count: u32) -> String {
        (1..=count)
            .map(|index| format!("- [x] Task {}\n", index))
            .collect()
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
        let tasks_path = dir.path().join("tasks.md");
        std::fs::write(&tasks_path, "## Implementation Tasks\n- [x] done\n").unwrap();

        record_acceptance_follow_up(
            &tasks_path,
            2,
            &[
                "missing repository coverage".to_string(),
                "add notification links".to_string(),
            ],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("## Current Acceptance Follow-up"));
        assert!(content.contains("- [ ] missing repository coverage"));
        assert!(content.contains("- [ ] add notification links"));

        let progress = parse_file(&tasks_path, None).unwrap();
        assert_eq!(progress.completed, 1);
        assert_eq!(progress.total, 3);
    }

    #[test]
    fn test_record_acceptance_follow_up_replaces_existing_section() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = dir.path().join("tasks.md");
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- [x] stale\n\n## Final Validation\n- [ ] run tests\n",
        )
        .unwrap();

        record_acceptance_follow_up(&tasks_path, 1, &["fresh finding".to_string()]).unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("## Current Acceptance Follow-up"));
        assert!(content.contains("- [ ] fresh finding"));
        assert!(content.contains("## Final Validation\n- [ ] run tests"));
        assert!(!content.contains("- [x] stale"));
    }

    #[test]
    fn record_acceptance_follow_up_replaces_previous_attempt() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = dir.path().join("tasks.md");
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Acceptance #1 Failure Follow-up\n- [x] stale\n",
        )
        .unwrap();

        record_acceptance_follow_up(&tasks_path, 2, &["latest finding".to_string()]).unwrap();

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
        let tasks_path = dir.path().join("tasks.md");
        std::fs::write(&tasks_path, "## Implementation Tasks\n- [x] done\n").unwrap();

        ensure_acceptance_follow_up(
            &tasks_path,
            2,
            &[
                "latest finding".to_string(),
                "add regression test".to_string(),
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
        let tasks_path = dir.path().join("tasks.md");
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- [x] add regression test\n",
        )
        .unwrap();

        ensure_acceptance_follow_up(
            &tasks_path,
            2,
            &[
                "latest finding".to_string(),
                "add regression test".to_string(),
            ],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("- [x] add regression test"));
        assert!(content.contains("- [ ] latest finding"));
        let progress = parse_file(&tasks_path, None).unwrap();
        assert_eq!(progress, TaskProgress::with_counts(2, 3));
    }

    #[test]
    fn acceptance_finding_identity_uses_complete_leading_code() {
        assert_eq!(
            acceptance_finding_identity("[SERIAL_STALLED_MARKER_MISSING] details"),
            "[SERIAL_STALLED_MARKER_MISSING]"
        );
        assert_eq!(
            acceptance_finding_identity("plain finding"),
            "plain finding"
        );
    }

    #[test]
    fn ensure_acceptance_follow_up_reopens_reworded_plain_finding_without_stable_code() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = dir.path().join("tasks.md");
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- [x] fixed and verified with regression coverage\n",
        )
        .unwrap();

        ensure_acceptance_follow_up(
            &tasks_path,
            2,
            &["missing repository coverage at src/example.rs:10".to_string()],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("- [ ] missing repository coverage at src/example.rs:10"));
        assert_eq!(
            parse_file(&tasks_path, None).unwrap(),
            TaskProgress::with_counts(1, 2)
        );
    }

    #[test]
    fn ensure_acceptance_follow_up_preserves_completed_finding_by_code() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = dir.path().join("tasks.md");
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- [x] [SERIAL_STALLED_MARKER_MISSING] fixed and verified\n",
        )
        .unwrap();

        ensure_acceptance_follow_up(
            &tasks_path,
            2,
            &["[SERIAL_STALLED_MARKER_MISSING] detailed original finding".to_string()],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("- [x] [SERIAL_STALLED_MARKER_MISSING] detailed original finding"));
        assert_eq!(
            parse_file(&tasks_path, None).unwrap(),
            TaskProgress::with_counts(2, 2)
        );
    }

    #[test]
    fn record_acceptance_follow_up_reopens_repeated_stable_identity() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = dir.path().join("tasks.md");
        std::fs::write(
            &tasks_path,
            "## Implementation Tasks\n- [x] done\n\n## Current Acceptance Follow-up\n- attempt: 1\n- [x] [SERIAL_STALLED_MARKER_MISSING] fixed and verified\n",
        )
        .unwrap();

        record_acceptance_follow_up(
            &tasks_path,
            2,
            &["[SERIAL_STALLED_MARKER_MISSING] still missing at src/run.rs:42".to_string()],
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
        let tasks_path = dir.path().join("tasks.md");
        std::fs::write(&tasks_path, "## Implementation Tasks\n- [x] done\n").unwrap();

        record_acceptance_follow_up(
            &tasks_path,
            3,
            &[
                "fix repository regression at src/run.rs:4".to_string(),
                "external non-mockable prerequisite: vendor approval".to_string(),
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
            parse_file(&tasks_path, None).unwrap(),
            TaskProgress::with_counts(1, 2)
        );
    }

    #[test]
    fn read_acceptance_follow_up_restores_mixed_repository_and_external_findings() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = dir.path().join("tasks.md");
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
                vec![
                    "fix repository regression at src/run.rs:4".to_string(),
                    "external non-mockable prerequisite: vendor approval".to_string(),
                ],
            ))
        );
    }

    #[test]
    fn acceptance_follow_up_normalizes_multiline_findings() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = dir.path().join("tasks.md");
        std::fs::write(&tasks_path, "## Implementation Tasks\n- [x] done\n").unwrap();

        record_acceptance_follow_up(
            &tasks_path,
            2,
            &["finding\n## injected heading\n- [ ] injected task".to_string()],
        )
        .unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("- [ ] finding ## injected heading - [ ] injected task"));
        assert_eq!(
            content.matches("## Current Acceptance Follow-up").count(),
            1
        );
        assert_eq!(parse_file(&tasks_path, None).unwrap().total, 2);
    }

    #[test]
    fn clear_acceptance_follow_up_ignores_examples_in_code_fences() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = dir.path().join("tasks.md");
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
        let tasks_path = dir.path().join("tasks.md");
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
        let tasks_path = dir.path().join("tasks.md");
        let original =
            "## Notes\n```text\n```md\n## Acceptance #9 Failure Follow-up\n- [ ] example\n```\n";
        std::fs::write(&tasks_path, original).unwrap();

        clear_acceptance_follow_up(&tasks_path).unwrap();

        assert_eq!(std::fs::read_to_string(&tasks_path).unwrap(), original);
    }

    #[test]
    fn clear_acceptance_follow_up_rejects_non_runtime_section_content() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = dir.path().join("tasks.md");
        let original = "## Current Acceptance Follow-up\n- [x] fixed\n```md\n## injected\n```\n";
        std::fs::write(&tasks_path, original).unwrap();

        let error = clear_acceptance_follow_up(&tasks_path).unwrap_err();

        assert!(error.to_string().contains("non-runtime content"));
        assert_eq!(std::fs::read_to_string(&tasks_path).unwrap(), original);
    }

    #[test]
    fn clear_acceptance_follow_up_removes_runtime_sections_only() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = dir.path().join("tasks.md");
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
        let tasks_path = dir.path().join("tasks.md");
        std::fs::write(&tasks_path, "## Implementation Tasks\n- [x] done\n").unwrap();

        record_acceptance_follow_up(&tasks_path, 3, &[" ".to_string(), "\t".to_string()]).unwrap();

        let content = std::fs::read_to_string(&tasks_path).unwrap();
        assert!(content.contains("## Current Acceptance Follow-up"));
        assert!(content.contains("- [ ] Investigate acceptance failure and apply the required fix"));
    }

    #[test]
    fn test_record_acceptance_follow_up_adds_missing_trailing_newline_before_section() {
        let dir = tempfile::tempdir().unwrap();
        let tasks_path = dir.path().join("tasks.md");
        std::fs::write(&tasks_path, "## Implementation Tasks\n- [x] done").unwrap();

        record_acceptance_follow_up(&tasks_path, 4, &["fresh finding".to_string()]).unwrap();

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
        assert_eq!(resolved, active_tasks);
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
        assert_eq!(resolved, archive_tasks);
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
        let result = find_archive_directory("nonexistent", Some(base_path));
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

        let result = find_archive_directory("exact-match", Some(base_path));
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

        let result = find_archive_directory("my-feature", Some(base_path));
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
            &worktree_path.join("openspec/changes/test-priority/tasks.md"),
            &checked_tasks(4),
        );
        write_tasks(
            &worktree_path.join("openspec/changes/archive/test-priority/tasks.md"),
            &checked_tasks(3),
        );
        write_tasks(
            &base_path.join("openspec/changes/archive/test-priority/tasks.md"),
            &checked_tasks(2),
        );
        write_tasks(
            &base_path.join("openspec/changes/test-priority/tasks.md"),
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
                    &case_worktree.join("openspec/changes/test-priority/tasks.md"),
                    &checked_tasks(4),
                );
            }
            if worktree_archive {
                write_tasks(
                    &case_worktree.join("openspec/changes/archive/test-priority/tasks.md"),
                    &checked_tasks(3),
                );
            }
            if base_archive {
                write_tasks(
                    &case_base.join("openspec/changes/archive/test-priority/tasks.md"),
                    &checked_tasks(2),
                );
            }
            if base_active {
                write_tasks(
                    &case_base.join("openspec/changes/test-priority/tasks.md"),
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
            &worktree_path.join("openspec/changes/compat-change/tasks.md"),
            "- [x] Worktree\n- [ ] Worktree pending\n",
        );
        write_tasks(
            &base_path.join("openspec/changes/compat-change/tasks.md"),
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
            &base_path.join("openspec/changes/archive/archived-compat/tasks.md"),
            "- [x] Exact archive\n",
        );
        write_tasks(
            &base_path.join("openspec/changes/archive/2026-05-13-archived-compat/tasks.md"),
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
            &worktree_path.join("openspec/changes/archive/compat-archived/tasks.md"),
            &checked_tasks(3),
        );
        write_tasks(
            &worktree_path.join("openspec/changes/compat-archived/tasks.md"),
            &checked_tasks(2),
        );
        write_tasks(
            &base_path.join("openspec/changes/archive/compat-archived/tasks.md"),
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
            &prearchive_worktree.join("openspec/changes/compat-archived/tasks.md"),
            &checked_tasks(2),
        );
        write_tasks(
            &prearchive_base.join("openspec/changes/archive/compat-archived/tasks.md"),
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
            &base_only.join("openspec/changes/archive/compat-archived/tasks.md"),
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
}

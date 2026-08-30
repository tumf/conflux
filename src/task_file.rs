//! Shared task-file identity, resolution, parsing, and mutation.
//!
//! A change entry owns exactly one authoritative task artifact: the historical
//! `tasks.md` or the versioned `tasks.json`. Every workflow phase that reads or
//! mutates task state goes through this module, so progress, validation,
//! acceptance follow-up, archive authorization, and merge authorization can
//! never disagree about which file speaks for a change.
//!
//! Two supported filenames in the entry selected by the caller's resolution mode
//! are ambiguous and fail closed. A lower-priority location never hides an
//! invalid higher-priority artifact, and mutation modes never fall back to the
//! base tree.

use crate::acceptance::{validate_repository_finding, AcceptanceFinding, RepositoryFinding};
use crate::archive_layout;
use crate::error::{OrchestratorError, Result};
use crate::task_parser::TaskProgress;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Historical Markdown task artifact basename.
pub const MARKDOWN_FILE_NAME: &str = "tasks.md";
/// Structured JSON task artifact basename.
pub const JSON_FILE_NAME: &str = "tasks.json";
/// The only `schema_version` this build accepts in `tasks.json`.
pub const JSON_SCHEMA_VERSION: u64 = 1;

/// Which of the two supported representations a task artifact uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskFileFormat {
    /// `tasks.md` — checkbox Markdown.
    Markdown,
    /// `tasks.json` — versioned structured tasks.
    Json,
}

impl TaskFileFormat {
    /// Both supported formats, in the order candidate entries are probed.
    pub const ALL: [TaskFileFormat; 2] = [TaskFileFormat::Markdown, TaskFileFormat::Json];

    /// Repository basename for this format.
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Markdown => MARKDOWN_FILE_NAME,
            Self::Json => JSON_FILE_NAME,
        }
    }

    /// Recognize a supported task artifact basename.
    pub fn from_file_name(name: &str) -> Option<Self> {
        match name {
            MARKDOWN_FILE_NAME => Some(Self::Markdown),
            JSON_FILE_NAME => Some(Self::Json),
            _ => None,
        }
    }

    /// Temporary-file suffix used by the atomic writer.
    fn temp_suffix(self) -> &'static str {
        match self {
            Self::Markdown => ".md.tmp",
            Self::Json => ".json.tmp",
        }
    }
}

/// A resolved task artifact: its concrete path plus the format it is read and
/// written as.
///
/// Callers never re-derive the format from the path, so a mutation can never be
/// applied with the wrong writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskFile {
    /// Filesystem path of the artifact.
    pub path: PathBuf,
    /// Representation used to parse and rewrite it.
    pub format: TaskFileFormat,
}

impl TaskFile {
    /// Bind a path to an explicit format.
    pub fn new(path: PathBuf, format: TaskFileFormat) -> Self {
        Self { path, format }
    }

    /// The artifact `format` would use inside `entry_dir`, without checking
    /// whether it exists.
    pub fn in_entry(entry_dir: &Path, format: TaskFileFormat) -> Self {
        Self::new(entry_dir.join(format.file_name()), format)
    }

    /// Bind an existing path whose basename names a supported format.
    #[cfg(test)]
    pub fn from_path(path: &Path) -> Option<Self> {
        let format = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(TaskFileFormat::from_file_name)?;
        Some(Self::new(path.to_path_buf(), format))
    }

    /// Display form for diagnostics.
    pub fn display(&self) -> std::path::Display<'_> {
        self.path.display()
    }
}

impl AsRef<Path> for TaskFile {
    fn as_ref(&self) -> &Path {
        self.path.as_path()
    }
}

/// Where a resolved artifact was found.
///
/// Kept distinct from the format so a diagnostic can say both "worktree active"
/// and "JSON" without conflating them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskLocationKind {
    /// `<worktree>/openspec/changes/<id>/`.
    WorktreeActive,
    /// `<worktree>/openspec/changes/archive/<entry>/`.
    WorktreeArchive,
    /// `openspec/changes/archive/<entry>/` in the base tree.
    BaseArchive,
    /// `openspec/changes/<id>/` in the base tree.
    BaseActive,
}

impl TaskLocationKind {
    /// Human-facing label used in debug logs.
    pub fn log_label(self) -> &'static str {
        match self {
            Self::WorktreeActive => "worktree active location",
            Self::WorktreeArchive => "worktree archive location",
            Self::BaseArchive => "base tree archive location",
            Self::BaseActive => "base tree active location",
        }
    }
}

/// One resolved artifact together with the location it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTaskFile {
    /// Where the artifact was found.
    pub kind: TaskLocationKind,
    /// The artifact itself.
    pub file: TaskFile,
}

fn ambiguity_error(entry_dir: &Path) -> OrchestratorError {
    OrchestratorError::ConfigLoad(format!(
        "Ambiguous task artifacts in {}: both {} and {} exist. A change entry must contain exactly one; neither wins by precedence.",
        entry_dir.display(),
        MARKDOWN_FILE_NAME,
        JSON_FILE_NAME
    ))
}

/// Select the sole supported task artifact inside one change entry.
///
/// Both filenames present is an ambiguity error rather than a precedence
/// decision, so no phase can silently pick a different source than another.
pub fn find_in_entry(entry_dir: &Path) -> Result<Option<TaskFile>> {
    let present: Vec<TaskFile> = TaskFileFormat::ALL
        .into_iter()
        .map(|format| TaskFile::in_entry(entry_dir, format))
        .filter(|candidate| candidate.path.is_file())
        .collect();

    match present.len() {
        0 => Ok(None),
        1 => Ok(present.into_iter().next()),
        _ => Err(ambiguity_error(entry_dir)),
    }
}

fn active_entry_dir(root: Option<&Path>, change_id: &str) -> PathBuf {
    root.unwrap_or_else(|| Path::new(""))
        .join("openspec/changes")
        .join(change_id)
}

fn archive_root(base_path: Option<&Path>) -> PathBuf {
    match base_path {
        Some(base) => base.join("openspec/changes/archive"),
        None => Path::new("openspec/changes/archive").to_path_buf(),
    }
}

/// Archive-layout error message for `change_id`, when the entry is unusable.
pub fn invalid_archive_layout_error(change_id: &str, base_path: Option<&Path>) -> Option<String> {
    archive_layout::invalid_layout_error(change_id, &archive_root(base_path)).map(|e| e.message())
}

/// The valid archive entry directory for `change_id`, if one exists.
pub fn find_archive_entry(change_id: &str, base_path: Option<&Path>) -> Option<PathBuf> {
    archive_layout::find_valid_archive_entry(change_id, &archive_root(base_path))
}

fn archived_task_file(change_id: &str, base_path: Option<&Path>) -> Result<Option<TaskFile>> {
    match find_archive_entry(change_id, base_path) {
        Some(entry) => find_in_entry(&entry),
        None => Ok(None),
    }
}

fn resolved(kind: TaskLocationKind, file: TaskFile) -> ResolvedTaskFile {
    ResolvedTaskFile { kind, file }
}

/// Comprehensive progress resolution: worktree active, worktree archive, base
/// archive, base active.
pub fn resolve_progress(
    change_id: &str,
    worktree_path: Option<&Path>,
) -> Result<Option<ResolvedTaskFile>> {
    if let Some(wt_path) = worktree_path {
        if let Some(file) = find_in_entry(&active_entry_dir(Some(wt_path), change_id))? {
            return Ok(Some(resolved(TaskLocationKind::WorktreeActive, file)));
        }
        if let Some(file) = archived_task_file(change_id, Some(wt_path))? {
            return Ok(Some(resolved(TaskLocationKind::WorktreeArchive, file)));
        }
    }
    if let Some(file) = archived_task_file(change_id, None)? {
        return Ok(Some(resolved(TaskLocationKind::BaseArchive, file)));
    }
    Ok(find_in_entry(&active_entry_dir(None, change_id))?
        .map(|file| resolved(TaskLocationKind::BaseActive, file)))
}

/// Active-only resolution: worktree active, then base active.
pub fn resolve_active(
    change_id: &str,
    worktree_path: Option<&Path>,
) -> Result<Option<ResolvedTaskFile>> {
    if let Some(wt_path) = worktree_path {
        if let Some(file) = find_in_entry(&active_entry_dir(Some(wt_path), change_id))? {
            return Ok(Some(resolved(TaskLocationKind::WorktreeActive, file)));
        }
    }
    Ok(find_in_entry(&active_entry_dir(None, change_id))?
        .map(|file| resolved(TaskLocationKind::BaseActive, file)))
}

/// Archived resolution: worktree archive, worktree active, then base archive.
pub fn resolve_archived(
    change_id: &str,
    worktree_path: Option<&Path>,
) -> Result<Option<ResolvedTaskFile>> {
    if let Some(wt_path) = worktree_path {
        if let Some(file) = archived_task_file(change_id, Some(wt_path))? {
            return Ok(Some(resolved(TaskLocationKind::WorktreeArchive, file)));
        }
        if let Some(file) = find_in_entry(&active_entry_dir(Some(wt_path), change_id))? {
            return Ok(Some(resolved(TaskLocationKind::WorktreeActive, file)));
        }
    }
    Ok(archived_task_file(change_id, None)?
        .map(|file| resolved(TaskLocationKind::BaseArchive, file)))
}

/// Workspace-local mutation resolution: worktree active, then worktree archive.
///
/// Deliberately has no base-tree fallback: Acceptance follow-up mutation and
/// rejection recovery must only ever write the resumed workspace.
pub fn resolve_mutation(change_id: &str, worktree_path: &Path) -> Result<Option<ResolvedTaskFile>> {
    if let Some(file) = find_in_entry(&active_entry_dir(Some(worktree_path), change_id))? {
        return Ok(Some(resolved(TaskLocationKind::WorktreeActive, file)));
    }
    if let Some(message) = invalid_archive_layout_error(change_id, Some(worktree_path)) {
        return Err(OrchestratorError::ConfigLoad(message));
    }
    Ok(archived_task_file(change_id, Some(worktree_path))?
        .map(|file| resolved(TaskLocationKind::WorktreeArchive, file)))
}

/// Read a task artifact's bytes as UTF-8.
pub fn read_to_string(file: &TaskFile) -> Result<String> {
    std::fs::read_to_string(&file.path).map_err(|e| {
        OrchestratorError::ConfigLoad(format!(
            "Failed to read tasks file {:?}: {}",
            file.path.display(),
            e
        ))
    })
}

/// Replace a task artifact in one atomic step.
///
/// The complete target content is written to a temporary file in the same
/// directory and renamed over the destination. Any failure before the rename
/// leaves the original file byte-for-byte unchanged.
pub fn write_atomically(file: &TaskFile, content: &str) -> Result<()> {
    use std::io::Write as _;

    let path = file.path.as_path();
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let directory = directory.unwrap_or_else(|| Path::new("."));
    let mut temp = tempfile::Builder::new()
        .prefix(".tasks-")
        .suffix(file.format.temp_suffix())
        .tempfile_in(directory)
        .map_err(|e| {
            OrchestratorError::ConfigLoad(format!(
                "Failed to stage atomic tasks update near {:?}: {}",
                path, e
            ))
        })?;
    temp.write_all(content.as_bytes()).map_err(|e| {
        OrchestratorError::ConfigLoad(format!("Failed to write tasks file {:?}: {}", path, e))
    })?;
    temp.as_file().sync_all().map_err(|e| {
        OrchestratorError::ConfigLoad(format!("Failed to flush tasks file {:?}: {}", path, e))
    })?;
    temp.persist(path).map_err(|e| {
        OrchestratorError::ConfigLoad(format!(
            "Failed to atomically replace tasks file {:?}: {}",
            path, e
        ))
    })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// JSON model
// ---------------------------------------------------------------------------

/// Closed status set for an ordinary JSON task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonTaskStatus {
    /// Not started.
    Pending,
    /// Started but unfinished.
    InProgress,
    /// The only status that counts as completed.
    Completed,
}

impl JsonTaskStatus {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }

    /// Whether this status contributes to the completed count.
    pub fn is_completed(self) -> bool {
        matches!(self, Self::Completed)
    }
}

/// Closed section set for an ordinary JSON task.
///
/// Narrative and runtime follow-up content live outside `tasks`, so there is no
/// section value that can smuggle them into progress totals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonTaskSection {
    /// Behavior-bearing implementation work.
    Implementation,
    /// Specification/documentation work.
    Specification,
}

impl JsonTaskSection {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "implementation" => Some(Self::Implementation),
            "specification" => Some(Self::Specification),
            _ => None,
        }
    }
}

/// One ordinary active task projected from `tasks.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonTask {
    /// Unique non-empty identifier.
    pub id: String,
    /// Human-facing task text.
    pub title: String,
    /// Completion status.
    pub status: JsonTaskStatus,
    /// Which active section the task belongs to.
    pub section: JsonTaskSection,
    /// Optional reference into the proposal's `verifications` block.
    pub verification_id: Option<String>,
    /// Optional descriptive verification kind.
    pub verification_kind: Option<String>,
    /// Optional descriptive verification command.
    pub verification_command: Option<String>,
    /// JSON Pointer of this task, for diagnostics.
    pub pointer: String,
}

/// One runtime-owned acceptance finding stored in JSON.
#[derive(Debug, Clone)]
pub struct JsonFinding {
    /// Stable normalized identity.
    pub identity: String,
    /// Actionable finding text.
    pub text: String,
    /// Runtime-owned structured payload, when the reviewer supplied one.
    pub finding: Option<RepositoryFinding>,
    /// Whether Apply claimed a repair. Never closure.
    pub remediation_claimed: bool,
    /// Apply-authored remediation evidence lines.
    pub evidence: Vec<String>,
}

/// One external blocker stored in JSON.
///
/// External blockers are not checkbox tasks and never alter progress counts.
#[derive(Debug, Clone)]
pub struct JsonExternalBlocker {
    /// Stable normalized identity.
    pub identity: String,
    /// Actionable blocker text.
    pub text: String,
    /// Supporting evidence lines.
    pub evidence: Vec<String>,
}

/// The runtime-owned follow-up block of a JSON task document.
#[derive(Debug, Clone)]
pub struct JsonFollowUp {
    /// Acceptance attempt this follow-up belongs to.
    pub attempt: u32,
    /// Internal findings; each is one virtual progress-gate task.
    pub findings: Vec<JsonFinding>,
    /// External blockers; never virtual tasks.
    pub external_blockers: Vec<JsonExternalBlocker>,
}

impl JsonFollowUp {
    /// Whether this block records nothing actionable.
    pub fn is_empty(&self) -> bool {
        self.findings.is_empty() && self.external_blockers.is_empty()
    }
}

/// A validated `tasks.json` document.
///
/// The raw `root` value is retained so Conflux-owned writes rewrite only the
/// fields Conflux owns and every unknown additive field survives untouched.
#[derive(Debug, Clone)]
pub struct JsonTaskDocument {
    root: Value,
    /// Ordinary active tasks, in document order.
    pub tasks: Vec<JsonTask>,
    /// Runtime-owned follow-up block, when present.
    pub follow_up: Option<JsonFollowUp>,
}

/// Object key carrying the runtime-owned acceptance follow-up in `tasks.json`.
///
/// Shared so progress detection can drop runtime bookkeeping without
/// re-spelling the key it is supposed to ignore.
pub const FOLLOW_UP_KEY: &str = "acceptance_follow_up";

fn json_error(pointer: &str, message: impl AsRef<str>) -> OrchestratorError {
    OrchestratorError::ConfigLoad(format!(
        "{}:{}: {}",
        JSON_FILE_NAME,
        if pointer.is_empty() { "/" } else { pointer },
        message.as_ref()
    ))
}

fn require_object<'a>(value: &'a Value, pointer: &str) -> Result<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| json_error(pointer, "expected a JSON object"))
}

fn require_string<'a>(parent: &'a Map<String, Value>, key: &str, pointer: &str) -> Result<&'a str> {
    let child = format!("{pointer}/{key}");
    let value = parent
        .get(key)
        .ok_or_else(|| json_error(&child, "required field is missing"))?;
    let text = value
        .as_str()
        .ok_or_else(|| json_error(&child, "expected a string"))?;
    if text.trim().is_empty() {
        return Err(json_error(&child, "must not be blank"));
    }
    Ok(text)
}

fn optional_string<'a>(
    parent: &'a Map<String, Value>,
    key: &str,
    pointer: &str,
) -> Result<Option<&'a str>> {
    let child = format!("{pointer}/{key}");
    match parent.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let text = value
                .as_str()
                .ok_or_else(|| json_error(&child, "expected a string"))?;
            if text.trim().is_empty() {
                return Err(json_error(&child, "must not be blank"));
            }
            Ok(Some(text))
        }
    }
}

fn optional_string_array(
    parent: &Map<String, Value>,
    key: &str,
    pointer: &str,
) -> Result<Vec<String>> {
    let child = format!("{pointer}/{key}");
    match parent.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                item.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| json_error(&format!("{child}/{index}"), "expected a string"))
            })
            .collect(),
        Some(_) => Err(json_error(&child, "expected an array of strings")),
    }
}

const VERIFICATION_KINDS: [&str; 6] = [
    "unit",
    "integration",
    "e2e",
    "manual",
    "benchmark",
    "not-testable",
];

fn parse_task(value: &Value, pointer: &str, seen_ids: &mut HashSet<String>) -> Result<JsonTask> {
    let object = require_object(value, pointer)?;
    let id = require_string(object, "id", pointer)?.to_string();
    if !seen_ids.insert(id.clone()) {
        return Err(json_error(
            &format!("{pointer}/id"),
            format!("duplicate task id '{id}'"),
        ));
    }
    let title = require_string(object, "title", pointer)?.to_string();

    let status_text = require_string(object, "status", pointer)?;
    let status = JsonTaskStatus::parse(status_text).ok_or_else(|| {
        json_error(
            &format!("{pointer}/status"),
            format!("unknown status '{status_text}' (expected pending, in_progress, or completed)"),
        )
    })?;

    let section_text = require_string(object, "section", pointer)?;
    let section = JsonTaskSection::parse(section_text).ok_or_else(|| {
        json_error(
            &format!("{pointer}/section"),
            format!(
                "unknown section '{section_text}' (expected implementation or specification); \
                 narrative and acceptance follow-up content must not appear in tasks"
            ),
        )
    })?;

    let verification_id = optional_string(object, "verification_id", pointer)?.map(str::to_string);

    let verification_pointer = format!("{pointer}/verification");
    let (verification_kind, verification_command) = match object.get("verification") {
        None | Some(Value::Null) => (None, None),
        Some(verification) => {
            let verification = require_object(verification, &verification_pointer)?;
            let kind =
                optional_string(verification, "kind", &verification_pointer)?.map(str::to_string);
            if let Some(kind) = kind.as_deref() {
                if !VERIFICATION_KINDS.contains(&kind) {
                    return Err(json_error(
                        &format!("{verification_pointer}/kind"),
                        format!(
                            "unknown verification kind '{kind}' (expected one of: {})",
                            VERIFICATION_KINDS.join(", ")
                        ),
                    ));
                }
            }
            let command = optional_string(verification, "command", &verification_pointer)?
                .map(str::to_string);
            (kind, command)
        }
    };

    Ok(JsonTask {
        id,
        title,
        status,
        section,
        verification_id,
        verification_kind,
        verification_command,
        pointer: pointer.to_string(),
    })
}

fn parse_finding(value: &Value, pointer: &str) -> Result<JsonFinding> {
    let object = require_object(value, pointer)?;
    let identity = require_string(object, "identity", pointer)?.to_string();
    let text = require_string(object, "text", pointer)?.to_string();
    let finding_pointer = format!("{pointer}/finding");
    let finding = match object.get("finding") {
        None | Some(Value::Null) => None,
        Some(payload) => Some(validate_repository_finding(payload).map_err(|error| {
            json_error(
                &finding_pointer,
                format!("invalid runtime-owned finding payload: {}", error.reason()),
            )
        })?),
    };
    let remediation_claimed = match object.get("remediation_claimed") {
        None | Some(Value::Null) => false,
        Some(value) => value.as_bool().ok_or_else(|| {
            json_error(
                &format!("{pointer}/remediation_claimed"),
                "expected a boolean",
            )
        })?,
    };
    let evidence = optional_string_array(object, "evidence", pointer)?;
    Ok(JsonFinding {
        identity,
        text,
        finding,
        remediation_claimed,
        evidence,
    })
}

fn parse_external_blocker(value: &Value, pointer: &str) -> Result<JsonExternalBlocker> {
    let object = require_object(value, pointer)?;
    Ok(JsonExternalBlocker {
        identity: require_string(object, "identity", pointer)?.to_string(),
        text: require_string(object, "text", pointer)?.to_string(),
        evidence: optional_string_array(object, "evidence", pointer)?,
    })
}

fn parse_follow_up(value: &Value, pointer: &str) -> Result<JsonFollowUp> {
    let object = require_object(value, pointer)?;
    let attempt_pointer = format!("{pointer}/attempt");
    let attempt = object
        .get("attempt")
        .ok_or_else(|| json_error(&attempt_pointer, "required field is missing"))?
        .as_u64()
        .filter(|attempt| *attempt >= 1 && *attempt <= u32::MAX as u64)
        .ok_or_else(|| json_error(&attempt_pointer, "expected a positive integer"))?
        as u32;

    let findings = match object.get("findings") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(index, item)| parse_finding(item, &format!("{pointer}/findings/{index}")))
            .collect::<Result<Vec<_>>>()?,
        Some(_) => {
            return Err(json_error(
                &format!("{pointer}/findings"),
                "expected an array",
            ))
        }
    };

    let external_blockers = match object.get("external_blockers") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                parse_external_blocker(item, &format!("{pointer}/external_blockers/{index}"))
            })
            .collect::<Result<Vec<_>>>()?,
        Some(_) => {
            return Err(json_error(
                &format!("{pointer}/external_blockers"),
                "expected an array",
            ))
        }
    };

    Ok(JsonFollowUp {
        attempt,
        findings,
        external_blockers,
    })
}

/// Validate the narrative block's shape.
///
/// Narrative content — including Final Validation — is prose. It is checked so a
/// malformed block fails closed, and then deliberately dropped: it can never
/// become a task, a status, or completion evidence.
fn parse_narrative(value: &Value) -> Result<()> {
    let pointer = "/narrative";
    let object = require_object(value, pointer)?;
    for key in ["future_work", "out_of_scope", "notes"] {
        optional_string_array(object, key, pointer)?;
    }
    optional_string(object, "final_validation", pointer)?;
    Ok(())
}

/// Parse and semantically validate a `tasks.json` body.
///
/// Every failure is typed and fails closed: nothing here can yield a `0/0`
/// projection, archive readiness, or merge authorization for an artifact the
/// runtime could not fully understand.
pub fn parse_json_document(content: &str) -> Result<JsonTaskDocument> {
    let root: Value = serde_json::from_str(content)
        .map_err(|error| json_error("", format!("invalid JSON: {error}")))?;
    let object = require_object(&root, "")?;

    let version_pointer = "/schema_version";
    let version = object
        .get("schema_version")
        .ok_or_else(|| json_error(version_pointer, "required field is missing"))?
        .as_u64()
        .ok_or_else(|| json_error(version_pointer, "expected an integer"))?;
    if version != JSON_SCHEMA_VERSION {
        return Err(json_error(
            version_pointer,
            format!(
                "unsupported schema_version {version} (this build supports {JSON_SCHEMA_VERSION})"
            ),
        ));
    }

    let tasks_value = object
        .get("tasks")
        .ok_or_else(|| json_error("/tasks", "required field is missing"))?;
    let items = tasks_value
        .as_array()
        .ok_or_else(|| json_error("/tasks", "expected an array"))?;
    let mut seen_ids = HashSet::new();
    let tasks = items
        .iter()
        .enumerate()
        .map(|(index, item)| parse_task(item, &format!("/tasks/{index}"), &mut seen_ids))
        .collect::<Result<Vec<_>>>()?;

    if let Some(narrative) = object.get("narrative").filter(|value| !value.is_null()) {
        parse_narrative(narrative)?;
    }

    let follow_up = match object.get(FOLLOW_UP_KEY) {
        None | Some(Value::Null) => None,
        Some(value) => Some(parse_follow_up(value, &format!("/{FOLLOW_UP_KEY}"))?),
    };

    Ok(JsonTaskDocument {
        root,
        tasks,
        follow_up,
    })
}

impl JsonTaskDocument {
    /// Progress including runtime-owned virtual finding gates.
    ///
    /// Only `completed` ordinary tasks count. Each internal finding contributes
    /// one to the total and one to completed only when Apply claimed a repair,
    /// so completed implementation tasks plus an unclaimed finding can never
    /// authorize archive or merge.
    pub fn progress(&self) -> TaskProgress {
        let mut progress = TaskProgress {
            completed: self
                .tasks
                .iter()
                .filter(|task| task.status.is_completed())
                .count() as u32,
            total: self.tasks.len() as u32,
        };
        if let Some(follow_up) = &self.follow_up {
            for finding in &follow_up.findings {
                progress.total += 1;
                if finding.remediation_claimed {
                    progress.completed += 1;
                }
            }
        }
        progress
    }

    /// The current follow-up, when it records anything actionable.
    pub fn current_follow_up(&self) -> Option<&JsonFollowUp> {
        self.follow_up
            .as_ref()
            .filter(|follow_up| !follow_up.is_empty())
    }

    /// Serialize the document back to a JSON body with a trailing newline.
    pub fn to_content(&self) -> Result<String> {
        let mut rendered = serde_json::to_string_pretty(&self.root).map_err(|error| {
            OrchestratorError::ConfigLoad(format!("Failed to serialize {JSON_FILE_NAME}: {error}"))
        })?;
        rendered.push('\n');
        Ok(rendered)
    }

    fn root_object_mut(&mut self) -> &mut Map<String, Value> {
        self.root
            .as_object_mut()
            .expect("validated task document root is an object")
    }

    /// Append one pending implementation task, keyed by `id`.
    ///
    /// Returns `false` when a task with that id already exists, so repeated
    /// rejection-recovery insertion converges on the same document instead of
    /// growing it.
    pub fn append_pending_task(&mut self, id: &str, title: &str) -> bool {
        if self.tasks.iter().any(|task| task.id == id) {
            return false;
        }
        let mut task = Map::new();
        task.insert("id".to_string(), Value::from(id));
        task.insert("title".to_string(), Value::from(title));
        task.insert("status".to_string(), Value::from("pending"));
        task.insert("section".to_string(), Value::from("implementation"));
        self.root_object_mut()
            .entry("tasks".to_string())
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .expect("validated task document has a tasks array")
            .push(Value::Object(task));
        self.tasks.push(JsonTask {
            id: id.to_string(),
            title: title.to_string(),
            status: JsonTaskStatus::Pending,
            section: JsonTaskSection::Implementation,
            verification_id: None,
            verification_kind: None,
            verification_command: None,
            pointer: format!("/tasks/{}", self.tasks.len()),
        });
        true
    }

    /// Replace the runtime-owned follow-up block, leaving every other field —
    /// known or unknown — exactly as it was.
    pub fn set_follow_up(&mut self, follow_up: Option<JsonFollowUp>) {
        match &follow_up {
            Some(value) => {
                let rendered = render_follow_up(value);
                self.root_object_mut()
                    .insert(FOLLOW_UP_KEY.to_string(), rendered);
            }
            None => {
                self.root_object_mut().remove(FOLLOW_UP_KEY);
            }
        }
        self.follow_up = follow_up;
    }
}

fn render_follow_up(follow_up: &JsonFollowUp) -> Value {
    let findings = follow_up
        .findings
        .iter()
        .map(|finding| {
            let mut object = Map::new();
            object.insert(
                "identity".to_string(),
                Value::from(finding.identity.clone()),
            );
            object.insert("text".to_string(), Value::from(finding.text.clone()));
            object.insert(
                "finding".to_string(),
                finding
                    .finding
                    .as_ref()
                    .map(RepositoryFinding::to_json)
                    .unwrap_or(Value::Null),
            );
            object.insert(
                "remediation_claimed".to_string(),
                Value::from(finding.remediation_claimed),
            );
            object.insert(
                "evidence".to_string(),
                Value::from(finding.evidence.clone()),
            );
            Value::Object(object)
        })
        .collect::<Vec<_>>();

    let blockers = follow_up
        .external_blockers
        .iter()
        .map(|blocker| {
            let mut object = Map::new();
            object.insert(
                "identity".to_string(),
                Value::from(blocker.identity.clone()),
            );
            object.insert("text".to_string(), Value::from(blocker.text.clone()));
            object.insert(
                "evidence".to_string(),
                Value::from(blocker.evidence.clone()),
            );
            Value::Object(object)
        })
        .collect::<Vec<_>>();

    let mut object = Map::new();
    object.insert("attempt".to_string(), Value::from(follow_up.attempt));
    object.insert("findings".to_string(), Value::Array(findings));
    object.insert("external_blockers".to_string(), Value::Array(blockers));
    Value::Object(object)
}

impl JsonFinding {
    /// Project this stored finding back into the shared acceptance contract.
    ///
    /// The runtime-owned structured payload wins over stored text, so Apply
    /// cannot rewrite a finding into an easier one by editing prose.
    pub fn to_acceptance_finding(&self) -> AcceptanceFinding {
        match &self.finding {
            Some(structured) => AcceptanceFinding::structured(structured.clone()),
            None => AcceptanceFinding::legacy(self.text.clone()),
        }
    }
}

/// Read a task artifact's progress through the shared contract.
pub fn read_progress(file: &TaskFile, change_id: Option<&str>) -> Result<TaskProgress> {
    let content = read_to_string(file)?;
    parse_progress(file.format, &content, change_id)
}

/// Compute progress for already-read content in a known format.
pub fn parse_progress(
    format: TaskFileFormat,
    content: &str,
    change_id: Option<&str>,
) -> Result<TaskProgress> {
    match format {
        TaskFileFormat::Markdown => Ok(crate::task_parser::parse_content(content, change_id)),
        TaskFileFormat::Json => Ok(parse_json_document(content)?.progress()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().expect("parent")).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn minimal_json(status: &str) -> String {
        format!(
            r#"{{"schema_version":1,"tasks":[{{"id":"a","title":"Do the work","status":"{status}","section":"implementation"}}]}}"#
        )
    }

    #[test]
    fn format_round_trips_through_its_basename() {
        for format in TaskFileFormat::ALL {
            assert_eq!(
                TaskFileFormat::from_file_name(format.file_name()),
                Some(format)
            );
        }
        assert_eq!(TaskFileFormat::from_file_name("tasks.yaml"), None);
    }

    #[test]
    fn entry_with_both_filenames_is_ambiguous() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(MARKDOWN_FILE_NAME), "- [x] done\n");
        write(&dir.path().join(JSON_FILE_NAME), &minimal_json("completed"));

        let error = find_in_entry(dir.path()).unwrap_err();
        assert!(
            error.to_string().contains("Ambiguous task artifacts"),
            "{error}"
        );
    }

    #[test]
    fn entry_selects_the_single_present_format() {
        let markdown = tempfile::tempdir().unwrap();
        write(&markdown.path().join(MARKDOWN_FILE_NAME), "- [ ] todo\n");
        assert_eq!(
            find_in_entry(markdown.path()).unwrap().unwrap().format,
            TaskFileFormat::Markdown
        );

        let json = tempfile::tempdir().unwrap();
        write(&json.path().join(JSON_FILE_NAME), &minimal_json("pending"));
        assert_eq!(
            find_in_entry(json.path()).unwrap().unwrap().format,
            TaskFileFormat::Json
        );

        let empty = tempfile::tempdir().unwrap();
        assert!(find_in_entry(empty.path()).unwrap().is_none());
    }

    #[test]
    fn json_progress_counts_only_completed_status() {
        let document = parse_json_document(
            r#"{"schema_version":1,"tasks":[
                {"id":"a","title":"A","status":"pending","section":"implementation"},
                {"id":"b","title":"B","status":"in_progress","section":"implementation"},
                {"id":"c","title":"C","status":"completed","section":"specification"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(document.progress(), TaskProgress::with_counts(1, 3));
    }

    #[test]
    fn empty_task_list_is_never_complete() {
        let document = parse_json_document(r#"{"schema_version":1,"tasks":[]}"#).unwrap();
        assert_eq!(document.progress(), TaskProgress::with_counts(0, 0));
    }

    #[test]
    fn unclaimed_internal_finding_blocks_completion() {
        let document = parse_json_document(
            r#"{"schema_version":1,"tasks":[
                {"id":"a","title":"A","status":"completed","section":"implementation"}
            ],"acceptance_follow_up":{"attempt":2,"findings":[
                {"identity":"repository|src/a.rs|verification","text":"Add the missing test","remediation_claimed":false}
            ],"external_blockers":[
                {"identity":"external||vendor|plain","text":"vendor approval","evidence":[]}
            ]}}"#,
        )
        .unwrap();
        // The external blocker adds no task; the unclaimed finding does.
        assert_eq!(document.progress(), TaskProgress::with_counts(1, 2));
    }

    #[test]
    fn claimed_internal_finding_counts_as_a_remediation_claim() {
        let document = parse_json_document(
            r#"{"schema_version":1,"tasks":[
                {"id":"a","title":"A","status":"completed","section":"implementation"}
            ],"acceptance_follow_up":{"attempt":2,"findings":[
                {"identity":"x","text":"Add the missing test","remediation_claimed":true,"evidence":["cargo test passes"]}
            ]}}"#,
        )
        .unwrap();
        assert_eq!(document.progress(), TaskProgress::with_counts(2, 2));
    }

    #[test]
    fn invalid_documents_fail_closed_with_pointer_diagnostics() {
        let cases: [(&str, &str); 8] = [
            (r#"{"schema_version":2,"tasks":[]}"#, "/schema_version"),
            (r#"{"tasks":[]}"#, "/schema_version"),
            (r#"{"schema_version":1}"#, "/tasks"),
            (r#"{"schema_version":1,"tasks":{}}"#, "/tasks"),
            (
                r#"{"schema_version":1,"tasks":[{"id":"a","title":"A","status":"done","section":"implementation"}]}"#,
                "/tasks/0/status",
            ),
            (
                r#"{"schema_version":1,"tasks":[{"id":"a","title":"A","status":"pending","section":"narrative"}]}"#,
                "/tasks/0/section",
            ),
            (
                r#"{"schema_version":1,"tasks":[{"id":"a","title":"A","status":"pending","section":"implementation"},{"id":"a","title":"B","status":"pending","section":"implementation"}]}"#,
                "/tasks/1/id",
            ),
            (
                r#"{"schema_version":1,"tasks":[{"id":"","title":"A","status":"pending","section":"implementation"}]}"#,
                "/tasks/0/id",
            ),
        ];
        for (content, pointer) in cases {
            let error = parse_json_document(content)
                .map(|_| ())
                .expect_err(content)
                .to_string();
            assert!(
                error.contains(&format!("{JSON_FILE_NAME}:{pointer}")),
                "expected pointer {pointer} in {error}"
            );
        }

        let error = parse_json_document("not json").unwrap_err().to_string();
        assert!(error.contains("invalid JSON"), "{error}");
    }

    #[test]
    fn unknown_verification_kind_fails_closed() {
        let error = parse_json_document(
            r#"{"schema_version":1,"tasks":[{"id":"a","title":"A","status":"pending","section":"implementation","verification":{"kind":"smoke"}}]}"#,
        )
        .map(|_| ())
        .unwrap_err()
        .to_string();
        assert!(error.contains("/tasks/0/verification/kind"), "{error}");
    }

    #[test]
    fn follow_up_replacement_preserves_unknown_fields() {
        let mut document = parse_json_document(
            r#"{"schema_version":1,"vendor_extension":{"keep":true},"tasks":[
                {"id":"a","title":"A","status":"completed","section":"implementation","vendor_note":"kept"}
            ],"acceptance_follow_up":{"attempt":1,"findings":[]}}"#,
        )
        .unwrap();

        document.set_follow_up(Some(JsonFollowUp {
            attempt: 3,
            findings: vec![JsonFinding {
                identity: "id".to_string(),
                text: "Repair the thing".to_string(),
                finding: None,
                remediation_claimed: false,
                evidence: Vec::new(),
            }],
            external_blockers: Vec::new(),
        }));

        let rendered = document.to_content().unwrap();
        let reparsed = parse_json_document(&rendered).unwrap();
        assert_eq!(reparsed.follow_up.as_ref().unwrap().attempt, 3);
        assert_eq!(reparsed.follow_up.as_ref().unwrap().findings.len(), 1);
        assert!(rendered.contains("vendor_extension"), "{rendered}");
        assert!(rendered.contains("vendor_note"), "{rendered}");

        let mut cleared = reparsed;
        cleared.set_follow_up(None);
        let cleaned = cleared.to_content().unwrap();
        assert!(!cleaned.contains(FOLLOW_UP_KEY), "{cleaned}");
        assert!(cleaned.contains("vendor_extension"), "{cleaned}");
    }

    #[test]
    fn narrative_final_validation_is_not_a_task() {
        let document = parse_json_document(
            r#"{"schema_version":1,"tasks":[
                {"id":"a","title":"A","status":"completed","section":"implementation"}
            ],"narrative":{"future_work":["manual check"],"notes":[],"final_validation":"cflx openspec validate x --archive-gate"}}"#,
        )
        .unwrap();
        // The narrative is validated for shape and then contributes nothing.
        assert_eq!(document.progress(), TaskProgress::with_counts(1, 1));
        assert!(document.follow_up.is_none());
    }

    #[test]
    fn resolution_modes_keep_their_distinct_order() {
        let temp = tempfile::tempdir().unwrap();
        let worktree = temp.path().join("wt");
        let change_id = "change";

        // Worktree archive only: progress and archived modes find it, active does not.
        write(
            &worktree
                .join("openspec/changes/archive")
                .join(change_id)
                .join(JSON_FILE_NAME),
            &minimal_json("completed"),
        );
        assert_eq!(
            resolve_progress(change_id, Some(&worktree))
                .unwrap()
                .unwrap()
                .kind,
            TaskLocationKind::WorktreeArchive
        );
        assert_eq!(
            resolve_archived(change_id, Some(&worktree))
                .unwrap()
                .unwrap()
                .kind,
            TaskLocationKind::WorktreeArchive
        );
        assert!(resolve_active(change_id, Some(&worktree))
            .unwrap()
            .is_none());

        // Adding a worktree active file wins for progress and active modes only.
        write(
            &worktree
                .join("openspec/changes")
                .join(change_id)
                .join(MARKDOWN_FILE_NAME),
            "- [ ] todo\n",
        );
        let progress = resolve_progress(change_id, Some(&worktree))
            .unwrap()
            .unwrap();
        assert_eq!(progress.kind, TaskLocationKind::WorktreeActive);
        assert_eq!(progress.file.format, TaskFileFormat::Markdown);
        assert_eq!(
            resolve_archived(change_id, Some(&worktree))
                .unwrap()
                .unwrap()
                .kind,
            TaskLocationKind::WorktreeArchive
        );
    }

    #[test]
    fn mutation_mode_never_falls_back_to_the_base_tree() {
        let temp = tempfile::tempdir().unwrap();
        let worktree = temp.path().join("wt");
        let change_id = "change";

        std::fs::create_dir_all(worktree.join("openspec/changes")).unwrap();
        assert!(resolve_mutation(change_id, &worktree).unwrap().is_none());

        write(
            &worktree
                .join("openspec/changes/archive")
                .join(change_id)
                .join(JSON_FILE_NAME),
            &minimal_json("pending"),
        );
        let resolved = resolve_mutation(change_id, &worktree).unwrap().unwrap();
        assert_eq!(resolved.kind, TaskLocationKind::WorktreeArchive);
        assert_eq!(resolved.file.format, TaskFileFormat::Json);
    }

    #[test]
    fn invalid_higher_priority_entry_is_not_hidden_by_a_lower_one() {
        let temp = tempfile::tempdir().unwrap();
        let worktree = temp.path().join("wt");
        let change_id = "change";

        let active = worktree.join("openspec/changes").join(change_id);
        write(&active.join(MARKDOWN_FILE_NAME), "- [x] done\n");
        write(&active.join(JSON_FILE_NAME), &minimal_json("completed"));
        write(
            &worktree
                .join("openspec/changes/archive")
                .join(change_id)
                .join(MARKDOWN_FILE_NAME),
            "- [x] archived\n",
        );

        let error = resolve_progress(change_id, Some(&worktree)).unwrap_err();
        assert!(
            error.to_string().contains("Ambiguous task artifacts"),
            "{error}"
        );
    }

    #[test]
    fn atomic_write_replaces_content_for_either_format() {
        let dir = tempfile::tempdir().unwrap();
        for format in TaskFileFormat::ALL {
            let file = TaskFile::in_entry(dir.path(), format);
            write_atomically(&file, "first").unwrap();
            write_atomically(&file, "second").unwrap();
            assert_eq!(std::fs::read_to_string(&file.path).unwrap(), "second");
        }
    }

    #[test]
    fn read_progress_dispatches_on_the_bound_format() {
        let dir = tempfile::tempdir().unwrap();
        let markdown = TaskFile::in_entry(dir.path(), TaskFileFormat::Markdown);
        write(&markdown.path, "- [x] one\n- [ ] two\n");
        assert_eq!(
            read_progress(&markdown, None).unwrap(),
            TaskProgress::with_counts(1, 2)
        );

        let json = TaskFile::in_entry(dir.path(), TaskFileFormat::Json);
        write(&json.path, &minimal_json("completed"));
        assert_eq!(
            read_progress(&json, None).unwrap(),
            TaskProgress::with_counts(1, 1)
        );
    }
}

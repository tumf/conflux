//! Independent age and global-byte retention for the evaluation root.
//!
//! Two properties matter more than compactness here. First, *containment*: this
//! code deletes only files it positively recognized as evaluation records
//! beneath the dedicated root, so it can never reach an ordinary log — and
//! ordinary log retention, which is scoped to `<state-root>/logs`, can never
//! reach an evaluation record. Second, *determinism*: eviction order is
//! ascending filename date with the relative path as tie-breaker, so the same
//! inventory always evicts the same files.
//!
//! Neither bound may endanger the active writer. A single day that alone
//! exceeds the global budget is kept and the budget is temporarily exceeded,
//! because the alternative is deleting the file being appended to.

use std::path::{Path, PathBuf};

use chrono::NaiveDate;

use super::error::{EvaluationError, EvaluationResult};
use super::writer::EvaluationPolicy;
use crate::config::defaults::EVALUATION_PURPOSE_PARALLEL_DEPENDENCY;

/// One recognized evaluation file eligible for retention.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    path: PathBuf,
    /// Date parsed from the file *name*, never from filesystem metadata: an
    /// mtime is rewritten by a copy or a restore, the name is not.
    date: NaiveDate,
    /// Path relative to the evaluation root, used as the deterministic
    /// tie-breaker between two files carrying the same date.
    relative: String,
    bytes: u64,
}

/// Why one file could not be removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupFailure {
    /// Relative path, so a report can name the file without leaking the root.
    pub relative: String,
    pub reason: String,
}

/// What one retention pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CleanupReport {
    /// Recognized evaluation files inspected.
    pub considered: usize,
    /// Files removed by the age bound.
    pub removed_by_age: usize,
    /// Files removed by the global byte budget.
    pub removed_by_bytes: usize,
    /// Bytes reclaimed by successful removals.
    pub freed_bytes: u64,
    /// Files kept because they are the active writer.
    pub protected: Vec<String>,
    /// Per-file removal failures. A partial failure never rolls back the
    /// removals that did succeed.
    pub failures: Vec<CleanupFailure>,
}

impl CleanupReport {
    /// Total files removed by either bound.
    pub fn removed(&self) -> usize {
        self.removed_by_age + self.removed_by_bytes
    }
}

/// Apply age retention, then the global byte budget, beneath `root`.
///
/// `current` is the file a writer is about to append to and is never removed.
/// `today` is supplied rather than read from the clock so the age boundary is
/// exactly testable.
///
/// A missing root is success with an empty report: retention has nothing to do
/// and must not create the directory it was asked to clean.
pub fn run_cleanup(
    root: &Path,
    policy: EvaluationPolicy,
    current: Option<&Path>,
    today: NaiveDate,
) -> EvaluationResult<CleanupReport> {
    let mut report = CleanupReport::default();

    if !root.is_dir() {
        return Ok(report);
    }

    let mut candidates = inventory(root)?;
    report.considered = candidates.len();

    // Protection is decided once, up front, against both bounds. Comparing
    // canonical paths rather than the strings avoids keeping a file whose path
    // merely spells the same way through a different route.
    let protected = current.map(canonical_or_original);
    candidates.retain(|candidate| {
        let is_current = protected
            .as_deref()
            .is_some_and(|current| canonical_or_original(&candidate.path) == current);
        if is_current {
            report.protected.push(candidate.relative.clone());
        }
        !is_current
    });

    // Deterministic order for both passes: oldest date first, then relative
    // path. The byte pass consumes this same order, so eviction never depends
    // on directory-iteration order.
    candidates.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| a.relative.cmp(&b.relative))
    });

    // Age bound. `retention_days` counts the retained window inclusive of
    // today, matching ordinary log retention: 30 keeps today plus 29 days.
    let cutoff = today - chrono::Duration::days(i64::from(policy.retention_days.saturating_sub(1)));
    let mut survivors = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if candidate.date < cutoff {
            if remove(&candidate, &mut report) {
                report.removed_by_age += 1;
            } else {
                // A file that could not be removed still occupies bytes, so it
                // stays in the accounting the byte pass works from.
                survivors.push(candidate);
            }
        } else {
            survivors.push(candidate);
        }
    }

    // Byte bound, recomputed from what actually survived — including the
    // protected active writer, which counts against the budget even though it
    // can never be chosen as a victim.
    let protected_bytes: u64 = current
        .and_then(|path| std::fs::metadata(path).ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let mut total: u64 = survivors
        .iter()
        .map(|candidate| candidate.bytes)
        .sum::<u64>()
        .saturating_add(protected_bytes);

    for candidate in &survivors {
        if total <= policy.max_total_bytes {
            break;
        }
        if remove(candidate, &mut report) {
            report.removed_by_bytes += 1;
            total = total.saturating_sub(candidate.bytes);
        }
    }

    Ok(report)
}

/// Remove one candidate, recording either the freed bytes or a typed failure.
///
/// Returns whether the file is gone.
fn remove(candidate: &Candidate, report: &mut CleanupReport) -> bool {
    match std::fs::remove_file(&candidate.path) {
        Ok(()) => {
            report.freed_bytes = report.freed_bytes.saturating_add(candidate.bytes);
            true
        }
        Err(error) => {
            report.failures.push(CleanupFailure {
                relative: candidate.relative.clone(),
                reason: error.kind().to_string(),
            });
            false
        }
    }
}

/// Collect every recognized evaluation file beneath `root`.
///
/// Recognition is positive and exact — `<purpose>/<project>/<YYYY-MM-DD>.jsonl`,
/// a regular file, not a symlink at any of those three levels. Anything else,
/// the installation salt included, is neither removed nor counted against the
/// byte budget: retention must not be the thing that decides an unknown file is
/// disposable.
fn inventory(root: &Path) -> EvaluationResult<Vec<Candidate>> {
    let mut candidates = Vec::new();

    let purpose_dir = root.join(EVALUATION_PURPOSE_PARALLEL_DEPENDENCY);
    if !is_real_dir(&purpose_dir) {
        return Ok(candidates);
    }

    let projects =
        std::fs::read_dir(&purpose_dir).map_err(|source| EvaluationError::Read { source })?;

    for project in projects.flatten() {
        let project_path = project.path();
        if !is_real_dir(&project_path) {
            continue;
        }
        let Ok(files) = std::fs::read_dir(&project_path) else {
            continue;
        };

        for file in files.flatten() {
            let path = file.path();
            // `symlink_metadata` on purpose: a symlink named like a record must
            // be skipped, not followed to whatever it points at.
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                continue;
            }
            let Some(date) = record_file_date(&path) else {
                continue;
            };
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };

            candidates.push(Candidate {
                date,
                relative: relative.to_string_lossy().into_owned(),
                bytes: metadata.len(),
                path,
            });
        }
    }

    Ok(candidates)
}

/// Parse `YYYY-MM-DD.jsonl`, or `None` for any other name.
fn record_file_date(path: &Path) -> Option<NaiveDate> {
    if path.extension()?.to_str()? != "jsonl" {
        return None;
    }
    NaiveDate::parse_from_str(path.file_stem()?.to_str()?, "%Y-%m-%d").ok()
}

/// Whether `path` is a real directory rather than a symlink to one.
fn is_real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
}

/// Canonical form where available, otherwise the path as given.
///
/// A file that does not exist cannot be canonicalized, and that is not an error
/// here: the caller is comparing identity, and a non-existent path simply fails
/// to match an existing one.
fn canonical_or_original(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::defaults::EVALUATION_SALT_FILE_NAME;
    use std::fs;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 18).expect("fixture date")
    }

    fn policy(retention_days: u32, max_total_bytes: u64) -> EvaluationPolicy {
        EvaluationPolicy {
            retention_days,
            max_total_bytes,
        }
    }

    /// Write one recognized record file of `bytes` length.
    fn write_record(root: &Path, project: &str, date: &str, bytes: usize) -> PathBuf {
        let dir = root
            .join(EVALUATION_PURPOSE_PARALLEL_DEPENDENCY)
            .join(project);
        fs::create_dir_all(&dir).expect("project dir");
        let path = dir.join(format!("{date}.jsonl"));
        fs::write(&path, vec![b'x'; bytes]).expect("record file");
        path
    }

    fn surviving(root: &Path) -> Vec<String> {
        let mut names = Vec::new();
        let purpose = root.join(EVALUATION_PURPOSE_PARALLEL_DEPENDENCY);
        let Ok(projects) = fs::read_dir(&purpose) else {
            return names;
        };
        for project in projects.flatten() {
            let Ok(files) = fs::read_dir(project.path()) else {
                continue;
            };
            for file in files.flatten() {
                names.push(format!(
                    "{}/{}",
                    project.file_name().to_string_lossy(),
                    file.file_name().to_string_lossy()
                ));
            }
        }
        names.sort();
        names
    }

    #[test]
    fn a_missing_root_is_an_empty_report_and_creates_nothing() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");

        let report = run_cleanup(&root, policy(30, 1024), None, today()).expect("cleanup");
        assert_eq!(report, CleanupReport::default());
        assert!(!root.exists(), "cleanup must never create the root");
    }

    #[test]
    fn age_retention_removes_only_files_past_the_window() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        // A 3-day window keeps today plus the two preceding days.
        write_record(&root, "proj-a", "2026-09-18", 10);
        write_record(&root, "proj-a", "2026-09-17", 10);
        write_record(&root, "proj-a", "2026-09-16", 10);
        write_record(&root, "proj-b", "2026-09-15", 10);
        write_record(&root, "proj-b", "2026-01-01", 10);

        let report = run_cleanup(&root, policy(3, u64::MAX), None, today()).expect("cleanup");

        assert_eq!(report.considered, 5);
        assert_eq!(report.removed_by_age, 2);
        assert_eq!(report.removed_by_bytes, 0);
        assert_eq!(report.freed_bytes, 20);
        assert_eq!(
            surviving(&root),
            vec![
                "proj-a/2026-09-16.jsonl",
                "proj-a/2026-09-17.jsonl",
                "proj-a/2026-09-18.jsonl",
            ]
        );
    }

    #[test]
    fn byte_budget_evicts_oldest_first_across_every_project() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        write_record(&root, "proj-a", "2026-09-10", 100);
        write_record(&root, "proj-b", "2026-09-11", 100);
        write_record(&root, "proj-a", "2026-09-12", 100);
        write_record(&root, "proj-b", "2026-09-13", 100);

        // 400 bytes present, 250 allowed: the two oldest go, across projects.
        let report = run_cleanup(&root, policy(3650, 250), None, today()).expect("cleanup");

        assert_eq!(report.removed_by_age, 0);
        assert_eq!(report.removed_by_bytes, 2);
        assert_eq!(report.freed_bytes, 200);
        assert_eq!(
            surviving(&root),
            vec!["proj-a/2026-09-12.jsonl", "proj-b/2026-09-13.jsonl"]
        );
    }

    #[test]
    fn same_date_across_projects_evicts_by_relative_path_deterministically() {
        for _ in 0..4 {
            let base = tempfile::tempdir().expect("base");
            let root = base.path().join("evaluations");
            write_record(&root, "proj-z", "2026-09-10", 100);
            write_record(&root, "proj-a", "2026-09-10", 100);
            write_record(&root, "proj-m", "2026-09-10", 100);

            let report = run_cleanup(&root, policy(3650, 150), None, today()).expect("cleanup");

            assert_eq!(report.removed_by_bytes, 2);
            assert_eq!(
                surviving(&root),
                vec!["proj-z/2026-09-10.jsonl"],
                "ties must break by relative path, not directory order"
            );
        }
    }

    #[test]
    fn the_two_bounds_are_independent() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        write_record(&root, "proj-a", "2026-01-01", 100); // age
        write_record(&root, "proj-a", "2026-09-16", 100); // bytes
        write_record(&root, "proj-a", "2026-09-17", 100);
        write_record(&root, "proj-a", "2026-09-18", 100);

        let report = run_cleanup(&root, policy(7, 250), None, today()).expect("cleanup");

        assert_eq!(report.removed_by_age, 1, "the stale file goes by age");
        assert_eq!(report.removed_by_bytes, 1, "the budget then trims one more");
        assert_eq!(report.freed_bytes, 200);
        assert_eq!(
            surviving(&root),
            vec!["proj-a/2026-09-17.jsonl", "proj-a/2026-09-18.jsonl"]
        );
    }

    #[test]
    fn the_active_writer_survives_both_bounds_even_alone_over_budget() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        let current = write_record(&root, "proj-a", "2026-01-01", 5_000);
        write_record(&root, "proj-a", "2026-09-17", 100);

        // The active file is both older than the window and larger than the
        // whole budget; it is still kept.
        let report = run_cleanup(&root, policy(3, 10), Some(&current), today()).expect("cleanup");

        assert!(current.exists(), "the active writer must never be removed");
        assert_eq!(
            report.protected,
            vec!["parallel_dependency/proj-a/2026-01-01.jsonl"]
        );
        assert_eq!(report.removed(), 1, "only the non-active file is eligible");
        assert_eq!(surviving(&root), vec!["proj-a/2026-01-01.jsonl"]);
    }

    #[test]
    fn unknown_entries_and_the_salt_are_never_removed_or_counted() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        fs::create_dir_all(
            root.join(EVALUATION_PURPOSE_PARALLEL_DEPENDENCY)
                .join("proj-a"),
        )
        .expect("dirs");

        // Deliberately huge, so if any of these were counted the byte pass
        // would evict the one real record below.
        fs::write(root.join(EVALUATION_SALT_FILE_NAME), vec![0u8; 4_000]).expect("salt");
        let project = root
            .join(EVALUATION_PURPOSE_PARALLEL_DEPENDENCY)
            .join("proj-a");
        fs::write(project.join("notes.txt"), vec![0u8; 4_000]).expect("unknown file");
        fs::write(project.join("2026-13-45.jsonl"), vec![0u8; 4_000]).expect("malformed date");
        fs::write(project.join("2026-09-17.log"), vec![0u8; 4_000]).expect("wrong extension");
        fs::create_dir(project.join("2026-09-16.jsonl")).expect("directory named like a record");
        write_record(&root, "proj-a", "2026-09-18", 10);

        let report = run_cleanup(&root, policy(3650, 10), None, today()).expect("cleanup");

        assert_eq!(report.considered, 1, "only the real record is recognized");
        assert_eq!(report.removed(), 0);
        assert!(root.join(EVALUATION_SALT_FILE_NAME).is_file());
        assert!(project.join("notes.txt").is_file());
        assert!(project.join("2026-13-45.jsonl").is_file());
        assert!(project.join("2026-09-17.log").is_file());
        assert!(project.join("2026-09-16.jsonl").is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_neither_followed_nor_removed() {
        let outside = tempfile::tempdir().expect("outside");
        let victim_dir = outside.path().join("logs").join("proj-a");
        fs::create_dir_all(&victim_dir).expect("victim dir");
        let victim = victim_dir.join("2026-01-01.jsonl");
        fs::write(&victim, vec![0u8; 9_000]).expect("victim file");

        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        let project = root
            .join(EVALUATION_PURPOSE_PARALLEL_DEPENDENCY)
            .join("proj-a");
        fs::create_dir_all(&project).expect("project");

        // A symlinked record file, and a symlinked project directory that would
        // expose a whole tree if it were walked.
        std::os::unix::fs::symlink(&victim, project.join("2026-01-02.jsonl")).expect("file link");
        std::os::unix::fs::symlink(
            outside.path().join("logs"),
            root.join(EVALUATION_PURPOSE_PARALLEL_DEPENDENCY)
                .join("linked"),
        )
        .expect("dir link");
        write_record(&root, "proj-a", "2026-09-18", 10);

        let report = run_cleanup(&root, policy(3, 1), None, today()).expect("cleanup");

        assert_eq!(report.considered, 1, "only the real record is inventoried");
        assert!(victim.is_file(), "a symlink target must never be removed");
        assert!(
            fs::symlink_metadata(project.join("2026-01-02.jsonl")).is_ok(),
            "the symlink itself must be left in place"
        );
    }

    #[test]
    fn ordinary_logs_beside_the_evaluation_root_are_unreachable() {
        let base = tempfile::tempdir().expect("base");
        let state_root = base.path().join("cflx");
        let logs = state_root.join("logs").join("proj-a");
        fs::create_dir_all(&logs).expect("logs");
        // Same shape as an evaluation record, on the other side of the fence.
        let log_file = logs.join("2026-01-01.jsonl");
        fs::write(&log_file, vec![0u8; 9_000]).expect("log");
        let real_log = logs.join("2026-01-01.log");
        fs::write(&real_log, b"operator diagnostics").expect("log");

        let root = state_root.join("evaluations");
        write_record(&root, "proj-a", "2026-01-01", 10);

        let report = run_cleanup(&root, policy(1, 1), None, today()).expect("cleanup");

        assert_eq!(report.considered, 1);
        assert_eq!(report.removed(), 1, "its own old record does go");
        assert!(log_file.is_file(), "evaluation cleanup must not reach logs");
        assert!(real_log.is_file());
    }

    #[cfg(unix)]
    #[test]
    fn a_partial_removal_failure_preserves_successful_removals() {
        use std::os::unix::fs::PermissionsExt;

        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        let removable = write_record(&root, "proj-a", "2026-01-01", 10);
        write_record(&root, "proj-b", "2026-01-02", 10);

        // A read-only *directory* is what blocks unlink; the file's own mode is
        // irrelevant on POSIX.
        let locked_dir = root
            .join(EVALUATION_PURPOSE_PARALLEL_DEPENDENCY)
            .join("proj-b");
        fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o500)).expect("lock");

        let report = run_cleanup(&root, policy(3, u64::MAX), None, today()).expect("cleanup");

        fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o700)).expect("unlock");

        assert_eq!(report.considered, 2);
        assert_eq!(report.removed_by_age, 1, "the removable file still goes");
        assert_eq!(report.freed_bytes, 10);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(
            report.failures[0].relative,
            "parallel_dependency/proj-b/2026-01-02.jsonl"
        );
        assert!(!removable.exists(), "a partial failure rolls nothing back");
        assert!(locked_dir.join("2026-01-02.jsonl").is_file());
    }

    #[test]
    fn a_file_that_could_not_be_removed_still_counts_against_the_budget() {
        // Pure accounting check: the byte pass must work from what actually
        // survives, so an un-removable file cannot make the budget look free.
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        write_record(&root, "proj-a", "2026-09-17", 100);
        write_record(&root, "proj-a", "2026-09-18", 100);

        let report = run_cleanup(&root, policy(3650, 100), None, today()).expect("cleanup");
        assert_eq!(report.removed_by_bytes, 1);
        assert_eq!(surviving(&root), vec!["proj-a/2026-09-18.jsonl"]);
    }

    #[test]
    fn a_budget_that_already_fits_removes_nothing() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        write_record(&root, "proj-a", "2026-09-18", 100);

        let report = run_cleanup(&root, policy(3650, 1_000), None, today()).expect("cleanup");
        assert_eq!(report.removed(), 0);
        assert_eq!(report.freed_bytes, 0);
        assert!(report.failures.is_empty());
    }
}

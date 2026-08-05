//! Task-complete Apply finalization stage gate.
//!
//! Apply agents own file selection: they stage the files their change owns and
//! leave nothing else behind. Conflux owns WIP preservation, repository-hook
//! execution, and the final commit. This module holds the one predicate that
//! keeps that boundary checkable — *is the managed workspace fully staged?* —
//! separated from Git process execution so it can be verified with plain
//! strings rather than a real repository.
//!
//! The gate reads `git status --porcelain` (v1). Each line is `XY PATH`, where
//! `X` is the index column and `Y` the worktree column:
//!
//! - a non-blank worktree column means content exists that the agent did not
//!   stage, including the `MM` case where a staged file was edited again,
//! - `??` means an untracked file the agent never selected at all,
//! - a staged-only entry (`M `, `A `, `R `, …) is exactly what a compliant
//!   agent leaves behind and passes the gate.
//!
//! `git add -A` in the WIP snapshot would silently absorb both failing cases,
//! which is why the gate runs *before* any snapshot or finalization staging.

/// Why one workspace entry fails the finalization stage gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageDefect {
    /// Worktree content that was never staged (dirty worktree column).
    Unstaged,
    /// A file Git does not track and the agent never selected.
    Untracked,
}

impl StageDefect {
    /// Stable machine-readable label used in diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unstaged => "unstaged",
            Self::Untracked => "untracked",
        }
    }
}

/// One workspace path that fails the gate, with the reason it failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageDefectEntry {
    /// Reason this path blocks finalization.
    pub defect: StageDefect,
    /// Path exactly as porcelain reported it, unquoted.
    pub path: String,
}

/// Maximum number of affected paths carried into bounded Apply feedback.
///
/// The complete captured status still reaches persistent logs; only the prompt
/// is bounded, so a workspace with hundreds of stray files cannot displace the
/// rest of the Apply context.
pub const MAX_REPORTED_STAGE_PATHS: usize = 20;

/// Classification of the managed workspace against the finalization gate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceStageStatus {
    /// Every entry that blocks finalization, in porcelain order.
    pub defects: Vec<StageDefectEntry>,
}

impl WorkspaceStageStatus {
    /// Whether finalization may proceed.
    pub fn is_clean(&self) -> bool {
        self.defects.is_empty()
    }

    /// Paths with a non-blank worktree column.
    pub fn unstaged_paths(&self) -> Vec<&str> {
        self.paths_with(StageDefect::Unstaged)
    }

    /// Paths Git does not track.
    pub fn untracked_paths(&self) -> Vec<&str> {
        self.paths_with(StageDefect::Untracked)
    }

    fn paths_with(&self, defect: StageDefect) -> Vec<&str> {
        self.defects
            .iter()
            .filter(|entry| entry.defect == defect)
            .map(|entry| entry.path.as_str())
            .collect()
    }

    /// Render at most [`MAX_REPORTED_STAGE_PATHS`] affected paths as one bounded
    /// diagnostic block, followed by a truthful count of what was elided.
    ///
    /// Truncation is always stated. A silently shortened list would read as
    /// "these are all the files", which is exactly the false completeness the
    /// gate exists to prevent.
    pub fn bounded_paths_report(&self) -> String {
        let mut lines = Vec::new();
        for entry in self.defects.iter().take(MAX_REPORTED_STAGE_PATHS) {
            lines.push(format!("{}: {}", entry.defect.as_str(), entry.path));
        }
        let elided = self.defects.len().saturating_sub(MAX_REPORTED_STAGE_PATHS);
        if elided > 0 {
            lines.push(format!(
                "... and {} more affected path(s); the complete status is in the persistent log",
                elided
            ));
        }
        lines.join("\n")
    }

    /// One-line summary of what the gate found.
    pub fn summary(&self) -> String {
        format!(
            "{} unstaged and {} untracked workspace entr(ies) remain",
            self.unstaged_paths().len(),
            self.untracked_paths().len()
        )
    }
}

/// Classify `git status --porcelain` output against the finalization gate.
///
/// Only the two status columns decide the outcome, so the classification never
/// depends on rendered Git prose or on path shape. Unparsable short lines are
/// ignored rather than guessed at: the gate must not invent a defect it cannot
/// name a path for.
pub fn classify_porcelain_status(porcelain: &str) -> WorkspaceStageStatus {
    let mut defects = Vec::new();

    for line in porcelain.lines() {
        if line.len() < 4 {
            continue;
        }
        let mut columns = line.chars();
        let index_column = columns.next().unwrap_or(' ');
        let worktree_column = columns.next().unwrap_or(' ');
        let path = line[3..].to_string();

        if index_column == '?' && worktree_column == '?' {
            defects.push(StageDefectEntry {
                defect: StageDefect::Untracked,
                path,
            });
            continue;
        }

        // `!!` never appears because the gate's status query passes
        // `--ignored=no`, but ignoring it here keeps the classifier correct for
        // any caller that captured status differently.
        if index_column == '!' && worktree_column == '!' {
            continue;
        }

        if worktree_column != ' ' {
            defects.push(StageDefectEntry {
                defect: StageDefect::Unstaged,
                path,
            });
        }
    }

    WorkspaceStageStatus { defects }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fully_staged_workspace_passes_the_gate() {
        let status = classify_porcelain_status("M  src/lib.rs\nA  src/new.rs\nD  src/old.rs\n");
        assert!(
            status.is_clean(),
            "staged-only entries must pass: {status:?}"
        );
    }

    #[test]
    fn an_empty_status_passes_the_gate() {
        assert!(classify_porcelain_status("").is_clean());
    }

    #[test]
    fn an_unstaged_worktree_column_fails_the_gate() {
        let status = classify_porcelain_status(" M src/lib.rs\n");
        assert!(!status.is_clean());
        assert_eq!(status.unstaged_paths(), vec!["src/lib.rs"]);
        assert!(status.untracked_paths().is_empty());
    }

    #[test]
    fn a_staged_file_edited_again_fails_the_gate() {
        // `MM` is the case `git add -A` would have silently absorbed.
        let status = classify_porcelain_status("MM src/lib.rs\n");
        assert_eq!(status.unstaged_paths(), vec!["src/lib.rs"]);
    }

    #[test]
    fn an_untracked_file_fails_the_gate() {
        let status = classify_porcelain_status("?? notes.txt\n");
        assert_eq!(status.untracked_paths(), vec!["notes.txt"]);
        assert!(status.unstaged_paths().is_empty());
    }

    #[test]
    fn unmerged_entries_fail_the_gate() {
        let status = classify_porcelain_status("UU src/conflict.rs\n");
        assert_eq!(status.unstaged_paths(), vec!["src/conflict.rs"]);
    }

    #[test]
    fn ignored_entries_do_not_fail_the_gate() {
        assert!(classify_porcelain_status("!! target/debug\n").is_clean());
    }

    #[test]
    fn a_staged_rename_passes_and_keeps_its_reported_path() {
        let status = classify_porcelain_status("R  old.rs -> new.rs\n");
        assert!(status.is_clean());

        let dirty = classify_porcelain_status("RM old.rs -> new.rs\n");
        assert_eq!(dirty.unstaged_paths(), vec!["old.rs -> new.rs"]);
    }

    #[test]
    fn mixed_defects_are_reported_in_porcelain_order() {
        let status = classify_porcelain_status(" M a.rs\n?? b.rs\nM  c.rs\n M d.rs\n");
        assert_eq!(
            status.defects,
            vec![
                StageDefectEntry {
                    defect: StageDefect::Unstaged,
                    path: "a.rs".to_string(),
                },
                StageDefectEntry {
                    defect: StageDefect::Untracked,
                    path: "b.rs".to_string(),
                },
                StageDefectEntry {
                    defect: StageDefect::Unstaged,
                    path: "d.rs".to_string(),
                },
            ]
        );
    }

    #[test]
    fn the_reported_path_list_is_bounded_and_states_what_it_dropped() {
        let porcelain = (0..MAX_REPORTED_STAGE_PATHS + 5)
            .map(|index| format!("?? file{index}.txt"))
            .collect::<Vec<_>>()
            .join("\n");
        let status = classify_porcelain_status(&porcelain);

        let report = status.bounded_paths_report();
        assert_eq!(
            report.lines().count(),
            MAX_REPORTED_STAGE_PATHS + 1,
            "the report keeps the cap plus one truncation notice: {report}"
        );
        assert!(
            report.contains("... and 5 more affected path(s)"),
            "truncation must be stated: {report}"
        );
        assert!(!report.contains("file20.txt"), "report: {report}");
    }

    #[test]
    fn short_or_blank_lines_never_invent_a_defect() {
        assert!(classify_porcelain_status("\n \nM\nMM\n").is_clean());
    }

    #[test]
    fn the_summary_counts_both_defect_kinds() {
        let status = classify_porcelain_status(" M a.rs\n?? b.rs\n?? c.rs\n");
        assert_eq!(
            status.summary(),
            "1 unstaged and 2 untracked workspace entr(ies) remain"
        );
    }
}

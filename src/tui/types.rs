//! Type definitions for the TUI module
//!
//! Contains enums and basic structs used throughout the TUI.
//! Method implementations are in `type_impls.rs`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg(feature = "web-monitoring")]
use utoipa::ToSchema;

/// View mode for TUI navigation
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ViewMode {
    /// Changes view - display and manage changes
    #[default]
    Changes,
    /// Worktrees view - display and manage git worktrees
    Worktrees,
}

/// Stop mode for graceful/force stop handling
#[derive(Debug, Clone, PartialEq, Default)]
pub enum StopMode {
    /// Not stopping, normal operation
    #[default]
    None,
    /// Graceful stop requested, waiting for current process
    GracefulPending,
    /// Immediate stop requested; runtime activity has not been classified yet.
    ///
    /// The second Esc only enqueues the shared stop command. Whether the stop is
    /// truthfully a force stop is decided asynchronously from one runtime
    /// activity snapshot, never from `AppExecutionMode::Stopping` alone.
    ImmediatePending,
    /// Force stop executed
    ForceStopped,
}

/// Orchestration execution mode of the TUI.
///
/// This axis carries lifecycle state only. Transient overlays that own input and
/// presentation live in [`ModalState`], so opening a popup can never replace,
/// capture, or restore an execution transition that happened underneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppExecutionMode {
    /// Selection mode - user selects changes to process
    #[default]
    Select,
    /// Running mode - processing selected changes
    Running,
    /// Stopping mode - graceful stop in progress
    Stopping,
    /// Stopped mode - processing halted, can modify queue
    Stopped,
    /// Error mode - a fatal global error occurred during processing
    Error,
}

impl AppExecutionMode {
    /// Project this frontend's execution mode onto the shared operator vocabulary.
    ///
    /// The shared matrix is the single command-admission authority, so the
    /// conversion is total and explicit rather than a defaulted fallback.
    pub fn operator_mode(self) -> crate::orchestration::operator_command::OperatorMode {
        use crate::orchestration::operator_command::OperatorMode;
        match self {
            AppExecutionMode::Select => OperatorMode::Select,
            AppExecutionMode::Running => OperatorMode::Running,
            AppExecutionMode::Stopping => OperatorMode::Stopping,
            AppExecutionMode::Stopped => OperatorMode::Stopped,
            AppExecutionMode::Error => OperatorMode::Error,
        }
    }

    /// Adopt the Core process lifecycle mode.
    ///
    /// The exact inverse of [`Self::operator_mode`]. It is total for the same
    /// reason: this frontend's mode is a *projection* of the Core value, so
    /// there is no Core state it could refuse to render.
    pub fn from_operator_mode(mode: crate::orchestration::operator_command::OperatorMode) -> Self {
        use crate::orchestration::operator_command::OperatorMode;
        match mode {
            OperatorMode::Select => AppExecutionMode::Select,
            OperatorMode::Running => AppExecutionMode::Running,
            OperatorMode::Stopping => AppExecutionMode::Stopping,
            OperatorMode::Stopped => AppExecutionMode::Stopped,
            OperatorMode::Error => AppExecutionMode::Error,
        }
    }

    /// Canonical `app_mode` token shared with the Web/API surface.
    ///
    /// Production reads this vocabulary in the other direction — the monitoring
    /// snapshot carries the token and `OperatorMode::from_app_mode` resolves it —
    /// so this side exists to pin that the TUI's execution axis and the canonical
    /// token set stay the same execution-only vocabulary.
    #[cfg(test)]
    pub fn app_mode_token(self) -> &'static str {
        match self {
            AppExecutionMode::Select => "select",
            AppExecutionMode::Running => "running",
            AppExecutionMode::Stopping => "stopping",
            AppExecutionMode::Stopped => "stopped",
            AppExecutionMode::Error => "error",
        }
    }
}

/// Transient modal interaction layered over the execution mode.
///
/// Each destructive confirmation carries its own identity payload, so a modal and
/// its target can never diverge: clearing the modal clears the payload with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalState {
    /// QR popup - showing the Web UI URL. Carries no destructive payload.
    QrPopup,
    /// Confirmation dialog for worktree deletion.
    ConfirmWorktreeDelete {
        /// Path of the worktree the confirmation was opened for.
        path: PathBuf,
        /// Branch bound to that worktree when the confirmation was opened.
        ///
        /// Mandatory: a deletion is confirmed against a concrete branch identity
        /// that can be re-checked before the mutation, so a worktree with no
        /// branch to name (detached HEAD) cannot reach this confirmation at all.
        branch: String,
    },
    /// Second, explicitly destructive confirmation for a known-dirty worktree.
    ///
    /// Only the shared service's own fresh `Dirty` refusal opens this, so every
    /// field is the service's observation rather than a TUI projection: the
    /// worktree list carries no dirty state and could not be trusted to.
    ConfirmDirtyDiscard {
        /// Path the service observed as dirty.
        path: PathBuf,
        /// Git worktree identity at observation time.
        identity: String,
        /// Branch at observation time.
        branch: String,
        /// HEAD commit at observation time.
        head: String,
        /// Teardown choice captured from the ordinary confirmation.
        ///
        /// `Y` captures `false` and `S` captures `true`. Carrying it here is what
        /// keeps skip-teardown and dirty discard independent permissions: this
        /// value records one operator decision and the `X` keypress grants the
        /// other.
        skip_teardown: bool,
    },
    /// Second, explicitly destructive confirmation for a worktree whose branch
    /// carries commits base does not have.
    ///
    /// Opened only from the shared service's own fresh commits-ahead refusal, on
    /// the same terms as [`Self::ConfirmDirtyDiscard`]. What makes it a separate
    /// overlay rather than a flag on that one is what it authorizes: deleting an
    /// unmerged branch, which no dirty-discard confirmation ever named.
    ConfirmAheadDiscard {
        /// Path the service observed ahead of base.
        path: PathBuf,
        /// Git worktree identity at observation time.
        identity: String,
        /// Branch at observation time.
        branch: String,
        /// HEAD commit at observation time. The branch is deleted only at this OID.
        head: String,
        /// Whether the same observation also reported uncommitted changes.
        ///
        /// The two losses are disclosed together because one keypress authorizes
        /// both; a confirmation that hid this would be granting dirty discard
        /// implicitly.
        dirty: bool,
        /// Teardown choice captured from the ordinary confirmation.
        skip_teardown: bool,
    },
    /// Force-kill confirmation for a single active change.
    ConfirmForceKill {
        /// The change ID being confirmed for force-kill
        change_id: String,
    },
}

impl ModalState {
    /// Presentation-only header label for this overlay.
    pub fn title_label(&self) -> &'static str {
        match self {
            ModalState::QrPopup => "QR Code",
            ModalState::ConfirmWorktreeDelete { .. } => "Confirm Delete",
            ModalState::ConfirmDirtyDiscard { .. } => "Discard Changes",
            ModalState::ConfirmAheadDiscard { .. } => "Discard Commits",
            ModalState::ConfirmForceKill { .. } => "Confirm Kill",
        }
    }

    /// True when this overlay requires an explicit operator decision.
    ///
    /// QR is presentation only, so it never blocks a workflow.
    pub fn is_user_decision(&self) -> bool {
        !matches!(self, ModalState::QrPopup)
    }
}

/// One confirmed worktree deletion, with every permission it was granted.
///
/// The three permissions are deliberately separate values rather than one
/// "force" flag: skipping teardown, discarding uncommitted work, and discarding
/// unmerged commits are different decisions, taken from different
/// confirmations, and one must never imply another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteIntent {
    /// Worktree the confirmation targets.
    pub path: PathBuf,
    /// Branch identity revalidated before the mutation.
    pub branch: String,
    /// Git worktree identity to revalidate, when the service supplied one.
    pub identity: Option<String>,
    /// HEAD commit to revalidate, when the service supplied one.
    pub head: Option<String>,
    /// Skip `.wt/teardown`. Chosen with `S` in the ordinary confirmation.
    pub skip_teardown: bool,
    /// Discard known uncommitted work. Granted only by `X` in a destructive
    /// confirmation that named that loss, and never by `Y` or `S`.
    pub allow_known_dirty: bool,
    /// Discard known commits ahead of base and delete the confirmed branch.
    /// Granted only by `X` in the ahead-discard confirmation.
    pub allow_commits_ahead: bool,
}

impl DeleteIntent {
    /// The ordinary intent: no discard of any kind, whatever teardown was chosen.
    pub fn ordinary(path: PathBuf, branch: String, skip_teardown: bool) -> Self {
        Self {
            path,
            branch,
            identity: None,
            head: None,
            skip_teardown,
            allow_known_dirty: false,
            allow_commits_ahead: false,
        }
    }
}

/// Information about a git worktree
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
pub struct WorktreeInfo {
    /// Path to the worktree
    #[cfg_attr(feature = "web-monitoring", schema(value_type = String))]
    pub path: PathBuf,
    /// Current HEAD commit (short hash or symbolic ref)
    pub head: String,
    /// Branch name (empty if detached)
    pub branch: String,
    /// Whether HEAD is detached
    pub is_detached: bool,
    /// Whether this is the main worktree
    pub is_main: bool,
    /// Merge conflict information (None if not checked or no conflicts)
    pub merge_conflict: Option<MergeConflictInfo>,
    /// Whether this worktree has commits ahead of the base branch
    pub has_commits_ahead: bool,
    /// Whether a merge operation is in progress for this worktree
    pub is_merging: bool,
}

/// Merge conflict information for a worktree
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web-monitoring", derive(ToSchema))]
pub struct MergeConflictInfo {
    /// List of files with merge conflicts
    pub conflict_files: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestration::operator_command::OperatorMode;

    #[test]
    fn execution_mode_defaults_to_select() {
        assert_eq!(AppExecutionMode::default(), AppExecutionMode::Select);
    }

    #[test]
    fn execution_mode_maps_onto_the_shared_operator_matrix() {
        let pairs = [
            (AppExecutionMode::Select, OperatorMode::Select),
            (AppExecutionMode::Running, OperatorMode::Running),
            (AppExecutionMode::Stopping, OperatorMode::Stopping),
            (AppExecutionMode::Stopped, OperatorMode::Stopped),
            (AppExecutionMode::Error, OperatorMode::Error),
        ];

        for (execution, operator) in pairs {
            assert_eq!(execution.operator_mode(), operator);
        }
    }

    #[test]
    fn canonical_app_mode_tokens_round_trip_through_the_shared_vocabulary() {
        for mode in [
            AppExecutionMode::Select,
            AppExecutionMode::Running,
            AppExecutionMode::Stopping,
            AppExecutionMode::Stopped,
            AppExecutionMode::Error,
        ] {
            assert_eq!(
                OperatorMode::from_app_mode(mode.app_mode_token()),
                mode.operator_mode(),
                "canonical app_mode token must stay execution-only for {:?}",
                mode
            );
        }
    }

    #[test]
    fn destructive_modals_carry_their_own_identity_payload() {
        let worktree = ModalState::ConfirmWorktreeDelete {
            path: PathBuf::from("/tmp/worktree-a"),
            branch: "feature/a".to_string(),
        };
        let kill = ModalState::ConfirmForceKill {
            change_id: "change-a".to_string(),
        };

        // Payload identity is part of modal equality, so a modal can never be
        // "the same modal" while pointing at a different target.
        assert_ne!(
            worktree,
            ModalState::ConfirmWorktreeDelete {
                path: PathBuf::from("/tmp/worktree-b"),
                branch: "feature/a".to_string(),
            }
        );
        assert_ne!(
            worktree,
            ModalState::ConfirmWorktreeDelete {
                path: PathBuf::from("/tmp/worktree-a"),
                branch: "feature/b".to_string(),
            }
        );
        assert_ne!(
            kill,
            ModalState::ConfirmForceKill {
                change_id: "change-b".to_string(),
            }
        );
    }

    #[test]
    fn only_confirmations_are_user_decisions() {
        assert!(!ModalState::QrPopup.is_user_decision());
        assert!(ModalState::ConfirmWorktreeDelete {
            path: PathBuf::from("/tmp/worktree-a"),
            branch: "feature/a".to_string(),
        }
        .is_user_decision());
        assert!(ModalState::ConfirmForceKill {
            change_id: "change-a".to_string(),
        }
        .is_user_decision());
    }

    #[test]
    fn modal_titles_are_presentation_labels_only() {
        assert_eq!(ModalState::QrPopup.title_label(), "QR Code");
        assert_eq!(
            ModalState::ConfirmWorktreeDelete {
                path: PathBuf::from("/tmp/worktree-a"),
                branch: "feature/a".to_string(),
            }
            .title_label(),
            "Confirm Delete"
        );
        assert_eq!(
            ModalState::ConfirmForceKill {
                change_id: "change-a".to_string(),
            }
            .title_label(),
            "Confirm Kill"
        );
    }
}

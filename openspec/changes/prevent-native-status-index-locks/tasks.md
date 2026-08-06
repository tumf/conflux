## Implementation Tasks

- [x] Define one child-command-local native read-only status argv policy with `--no-optional-locks` before `status`, then apply it to `has_uncommitted_changes`, human-readable plain `get_status` used by conflict-resolution prompts, untrimmed `porcelain_status`, `has_changes_to_commit`, `is_working_directory_clean`, `is_clean_including_untracked`, and the existing uncommitted-change monitor without changing each helper's porcelain, untracked, ignored, trimming, or error contract. Update the stage-gate command display and its exactness documentation to match the changed argv. Completion requires exact command-shape assertions for every shared helper, resolve-context coverage that preserves human-readable status text, and no `GIT_OPTIONAL_LOCKS` process environment mutation. (verification: integration - existing passing repository-local output for tests in `src/vcs/git/commands/status_policy_repository_tests.rs` is the recorded evidence and MUST NOT be rerun during Apply repair; verification-id: native-status-lock-regressions)

- [x] Migrate production direct/native status observations in Apply/Archive state classification and retired-but-callable marker cleanup to the shared policy or the same exact child argv ordering. Completion requires archiving detection, archive-complete detection, finalization snapshots, staged/unstaged stage-gate columns, and path-scoped residue checks to retain their current outputs and fail-closed behavior. (verification: integration - existing passing repository-local output for tests in `src/vcs/git/commands/status_policy_repository_tests.rs` is the recorded evidence and MUST NOT be rerun during Apply repair; verification-id: native-status-lock-regressions)

- [x] Apply the same policy to the upstream Git adapter's working-tree cleanliness and porcelain-v2 observations, preserving its observed-command test surface and `UpstreamPortError` mapping. Completion requires adapter tests to record `--no-optional-locks` before `status`, preserve `--porcelain=v2`, and prove fetch, merge, commit, and push argv remain unchanged. (verification: integration - existing passing repository-local output for tests in `src/vcs/git/commands/status_policy_repository_tests.rs` is the recorded evidence and MUST NOT be rerun during Apply repair; verification-id: native-status-lock-regressions)

- [x] Add temporary-repository coverage with a positive control that plain `git status` demonstrably changes complete index bytes in a stale-stat fixture, then prove representative shared, direct, and upstream production status paths report current clean/dirty state without changing those bytes. Cover staged, unstaged, deleted, renamed, untracked, ignored, and conflicted states plus untrimmed first-line porcelain columns; do not use inode or mtime alone as the oracle. (verification: integration - existing passing repository-local output for tests in `src/vcs/git/commands/status_policy_repository_tests.rs` is the recorded evidence and MUST NOT be rerun during Apply repair; verification-id: native-status-lock-regressions)

- [x] Add a production-command inventory regression that detects newly introduced native read-only `git status` argv construction lacking `--no-optional-locks` before the subcommand. Limit the inventory to production argv builders/adapters, excluding `#[cfg(test)]` fixture invocations, display strings, diagnostics, and prompt prose; a shared policy builder plus exact-argv assertions for every adapter is an acceptable structural implementation. Completion requires omission and post-subcommand placement to fail while an add/commit or release-like mutation is proven not to receive optional-lock suppression. (verification: integration - existing passing repository-local output for tests in `src/vcs/git/commands/status_policy_repository_tests.rs` is the recorded evidence and MUST NOT be rerun during Apply repair; verification-id: native-status-lock-regressions)

## Implementation Notes

- The shared policy lives in `src/vcs/git/commands/status_policy.rs`: one
  `read_only_status_argv` builder plus one named argument set per distinct
  observation (human-readable, porcelain, dirty-state, porcelain+untracked,
  change-monitor, path-scoped, porcelain v2). Every production status site now
  builds its argv from it, so no call site can order the global option wrongly.
- Migrated production sites: `basic::has_uncommitted_changes`,
  `basic::porcelain_status`, `basic::get_status`,
  `basic::is_working_directory_clean`, `commit::has_changes_to_commit`,
  `commit::list_changes_with_uncommitted_files`,
  `merge::is_clean_including_untracked`, `execution::state::has_archive_files`,
  `execution::archive::is_archive_commit_complete`,
  `execution::archive::git_status_porcelain`,
  `parallel::acceptance_state::verify_marker_removal_left_worktree_clean`,
  `upstream::git_ops::{is_working_tree_clean, status_porcelain_v2}`.
- `execution::apply::stage_gate_status_command` now renders the operator-facing
  command from `read_only_status_command_display(DIRTY_STATE_STATUS_ARGS)`, so
  the text cannot drift from the argv Conflux ran; a test pins the rendered
  string to the exact expected command line.
- Per-helper exact argv is asserted by running each shared helper outside a
  repository and reading the command line preserved on the resulting
  `VcsError::Command`; the upstream adapter is asserted from its recorded
  command log.
- The inventory regression parses production sources with `#[cfg(test)]` items
  and `tests/` directories removed, flags any argv-shaped `"status"` literal
  that is not preceded by `"--no-optional-locks"`, flags any
  `"--no-optional-locks"` element not immediately followed by `"status"`, and
  carries its own non-vacuity control plus detector self-tests for the omitted
  and post-subcommand cases.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate prevent-native-status-index-locks --archive-gate`.

Repository hooks own Rust style and static-analysis checks. Apply repair MUST NOT regenerate test, full-library, style, or static-analysis evidence.

## Future Work

- If external Git clients continue to cause index contention after Conflux stops taking optional read locks, propose a separately scoped cross-process coordination or narrowly classified retry policy with ownership and idempotency evidence.
- Worktree-scoped `git diff` can opportunistically refresh the index through a path not controlled by this status-specific policy; if diff-path contention is observed, propose and verify a separately scoped mitigation rather than assuming `--no-optional-locks` applies.

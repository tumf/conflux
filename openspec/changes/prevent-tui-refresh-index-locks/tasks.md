## Implementation Tasks

- [x] Define the production monitoring argv in `src/vcs/git/commands/commit.rs` as `--no-optional-locks status --porcelain -u`, with the global option before `status`, and retain all existing helper callers without process-wide environment mutation. (verification: unit - `cargo test vcs::git::commands::commit::tests` asserts the exact production argv and helper behavior; verification-id: refresh-status-lock-tests)

- [x] Add temporary-Git classification coverage for staged and unstaged add/modify/delete, untracked files, same-change rename, and clean committed exclusion, while preserving archive, hidden-directory, ignored-file, and unrelated-path exclusions. Every Git fixture setup command must be asserted successful. (verification: integration - `cargo test vcs::git::commands::commit::tests` exercises real Git output and fails on classification drift or fixture setup failure; verification-id: refresh-status-lock-tests)

- [x] Add a non-vacuous index-safety regression: create a fixture where normal status demonstrably changes complete index bytes, then prove the production helper leaves index bytes unchanged while reporting current worktree changes. Do not use inode or mtime alone as the oracle. (verification: integration - `cargo test vcs::git::commands::commit::tests` requires the positive control and byte-for-byte production-helper comparison; verification-id: refresh-status-lock-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate prevent-tui-refresh-index-locks --archive-gate`

The implementation must also pass `cargo test vcs::git::commands::commit::tests`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings`.

## Notes

- evidence: `src/vcs/git/commands/commit.rs:285` defines `UNCOMMITTED_MONITOR_ARGV = ["--no-optional-locks", "status", "--porcelain", "-u"]` and `list_changes_with_uncommitted_files` passes it to `run_git`; the four call sites in `src/tui/runner.rs`, `src/tui/orchestrator.rs`, and `src/parallel_run_service.rs` are unchanged and keep routing through that helper.
- evidence: `cargo test --lib vcs::git::commands::commit::tests` — 15 passed, 0 failed (0.50s), run with an isolated `CARGO_TARGET_DIR` because a sibling worktree sharing the default target directory had overwritten the test binary.
- evidence: mutation check — replacing the argv with `["--no-pager", "status", "--porcelain", "-u"]` fails both `uncommitted_monitor_argv_disables_optional_locks_before_subcommand` (exact argv equality) and `monitoring_does_not_persist_optional_index_refresh` (index bytes differ), so neither assertion is vacuous.
- evidence: the index test's positive control asserts a plain `git status --porcelain -u` changes the complete index bytes before the production helper is compared byte-for-byte against the same restored stale index; the oracle is full index content, not inode or mtime.
- evidence: `cargo fmt -- --check` clean; `cargo clippy --all-targets -- -D warnings` finished with no warnings.
- note: no repo-mutating Git command was touched, and optional-lock suppression is a child-command argument only — no `GIT_OPTIONAL_LOCKS` environment mutation exists in the tree.

## Future Work

- Broader read-only Git command auditing may be proposed separately if repository evidence identifies another command that takes unnecessary optional locks.
- A minimum supported Git version may be documented separately if Conflux introduces an explicit Git compatibility policy.

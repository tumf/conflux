## Implementation Tasks

- [ ] Change the uncommitted-change monitoring query in `src/vcs/git/commands/commit.rs` to disable Git optional locks while preserving `status --porcelain -u` output and all existing callers. (verification: unit - `cargo test vcs::git::commands::commit::tests` proves the helper still returns the expected change IDs; verification-id: refresh-status-lock-tests)

- [ ] Add temporary-Git regression coverage for staged, unstaged, renamed, and untracked files plus archive, hidden-directory, and unrelated-path exclusions. (verification: integration - `cargo test vcs::git::commands::commit::tests` exercises real Git status output and fails on classification drift; verification-id: refresh-status-lock-tests)

- [ ] Add a temporary-Git lock-safety regression that makes index stat data refreshable, invokes the production monitoring helper, and proves the index identity or metadata remains unchanged while current worktree changes are reported. (verification: integration - `cargo test vcs::git::commands::commit::tests` fails if the monitoring query performs an optional index refresh; verification-id: refresh-status-lock-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate prevent-tui-refresh-index-locks --archive-gate`

The implementation must also pass `cargo test vcs::git::commands::commit::tests`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings`.

## Future Work

- Broader read-only Git command auditing may be proposed separately if repository evidence identifies another command that takes unnecessary optional locks.

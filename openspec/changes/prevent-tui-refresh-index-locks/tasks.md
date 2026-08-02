## Implementation Tasks

- [ ] Define the production monitoring argv in `src/vcs/git/commands/commit.rs` as `--no-optional-locks status --porcelain -u`, with the global option before `status`, and retain all existing helper callers without process-wide environment mutation. (verification: unit - `cargo test vcs::git::commands::commit::tests` asserts the exact production argv and helper behavior; verification-id: refresh-status-lock-tests)

- [ ] Add temporary-Git classification coverage for staged and unstaged add/modify/delete, untracked files, same-change rename, and clean committed exclusion, while preserving archive, hidden-directory, ignored-file, and unrelated-path exclusions. Every Git fixture setup command must be asserted successful. (verification: integration - `cargo test vcs::git::commands::commit::tests` exercises real Git output and fails on classification drift or fixture setup failure; verification-id: refresh-status-lock-tests)

- [ ] Add a non-vacuous index-safety regression: create a fixture where normal status demonstrably changes complete index bytes, then prove the production helper leaves index bytes unchanged while reporting current worktree changes. Do not use inode or mtime alone as the oracle. (verification: integration - `cargo test vcs::git::commands::commit::tests` requires the positive control and byte-for-byte production-helper comparison; verification-id: refresh-status-lock-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate prevent-tui-refresh-index-locks --archive-gate`

The implementation must also pass `cargo test vcs::git::commands::commit::tests`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings`.

## Future Work

- Broader read-only Git command auditing may be proposed separately if repository evidence identifies another command that takes unnecessary optional locks.
- A minimum supported Git version may be documented separately if Conflux introduces an explicit Git compatibility policy.

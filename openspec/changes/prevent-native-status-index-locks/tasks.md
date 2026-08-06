## Implementation Tasks

- [ ] Define one child-command-local native read-only status argv policy with `--no-optional-locks` before `status`, then apply it to `has_uncommitted_changes`, human-readable plain `get_status` used by conflict-resolution prompts, untrimmed `porcelain_status`, `has_changes_to_commit`, `is_working_directory_clean`, `is_clean_including_untracked`, and the existing uncommitted-change monitor without changing each helper's porcelain, untracked, ignored, trimming, or error contract. Update the stage-gate command display and its exactness documentation to match the changed argv. Completion requires exact command-shape assertions for every shared helper, resolve-context coverage that preserves human-readable status text, and no `GIT_OPTIONAL_LOCKS` process environment mutation. (verification: unit - `cargo test --lib native_git_status_optional_locks`; verification-id: native-status-lock-regressions)

- [ ] Migrate production direct/native status observations in Apply/Archive state classification and retired-but-callable marker cleanup to the shared policy or the same exact child argv ordering. Completion requires archiving detection, archive-complete detection, finalization snapshots, staged/unstaged stage-gate columns, and path-scoped residue checks to retain their current outputs and fail-closed behavior. (verification: unit - `cargo test --lib native_git_status_optional_locks`; verification-id: native-status-lock-regressions)

- [ ] Apply the same policy to the upstream Git adapter's working-tree cleanliness and porcelain-v2 observations, preserving its observed-command test surface and `UpstreamPortError` mapping. Completion requires adapter tests to record `--no-optional-locks` before `status`, preserve `--porcelain=v2`, and prove fetch, merge, commit, and push argv remain unchanged. (verification: unit - `cargo test --lib native_git_status_optional_locks`; verification-id: native-status-lock-regressions)

- [ ] Add temporary-repository coverage with a positive control that plain `git status` demonstrably changes complete index bytes in a stale-stat fixture, then prove representative shared, direct, and upstream production status paths report current clean/dirty state without changing those bytes. Cover staged, unstaged, deleted, renamed, untracked, ignored, and conflicted states plus untrimmed first-line porcelain columns; do not use inode or mtime alone as the oracle. (verification: integration - `cargo test --lib native_git_status_optional_locks`; verification-id: native-status-lock-regressions)

- [ ] Add a production-command inventory regression that detects newly introduced native read-only `git status` argv construction lacking `--no-optional-locks` before the subcommand. Limit the inventory to production argv builders/adapters, excluding `#[cfg(test)]` fixture invocations, display strings, diagnostics, and prompt prose; a shared policy builder plus exact-argv assertions for every adapter is an acceptable structural implementation. Completion requires omission and post-subcommand placement to fail while an add/commit or release-like mutation is proven not to receive optional-lock suppression. (verification: integration - `cargo test --lib native_git_status_optional_locks`; verification-id: native-status-lock-regressions)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate prevent-native-status-index-locks --archive-gate`.

The tracked Rust pre-commit hooks are path-scoped, so the declared verification retains explicit `cargo fmt --check` and all-target/all-feature Clippy coverage.

## Future Work

- If external Git clients continue to cause index contention after Conflux stops taking optional read locks, propose a separately scoped cross-process coordination or narrowly classified retry policy with ownership and idempotency evidence.
- Worktree-scoped `git diff` can opportunistically refresh the index through a path not controlled by this status-specific policy; if diff-path contention is observed, propose and verify a separately scoped mitigation rather than assuming `--no-optional-locks` applies.

## Implementation Tasks

- [x] Add a transient classifier requiring a Git `VcsError::Command`, the Conflux-owned iteration-snapshot `git add -A` or WIP commit command, existing-`index.lock` stderr, and a reported lock path resolving to the current managed worktree Git directory; test rejection of other commands, repositories, lock prose, paths, and VCS errors. (verification: unit - `cargo test transient_wip_commit_lock_classifier`; verification-id: transient-wip-lock-tests)
- [x] Add the retry loop at `create_progress_commit` around `snapshot_working_copy` plus `create_iteration_snapshot`, with three total attempts, a fixed 200 ms delay, no backoff, and an injected sleeper for fast deterministic tests; do not add retry behavior to generic Git commands. (verification: unit - `cargo test transient_wip_commit_lock_retry_policy`; verification-id: transient-wip-lock-tests)
- [x] Pass or expose cancellation only at the progress-commit orchestration boundary, checking it after a retryable failure, before delay, and before the next attempt without changing the `WorkspaceManager` cancellation contract. (verification: unit - `cargo test transient_wip_commit_lock_cancellation`; verification-id: transient-wip-lock-tests)
- [x] Make ambiguous success idempotent by capturing `HEAD_before` per attempt and accepting completion only when new HEAD has exactly that parent and the exact expected WIP subject; prove a same-subject historical commit is not accepted. (verification: integration - `cargo test transient_wip_commit_lock_ambiguous_success`; verification-id: transient-wip-lock-tests)
- [x] Add a real temporary Git repository test that holds the managed worktree `index.lock`, releases it within the retry budget, and proves the WIP snapshot succeeds without losing staged or unstaged apply output. (verification: integration - `cargo test transient_wip_commit_lock_recovers`; verification-id: transient-wip-lock-tests)
- [x] Add failure-path tests proving live-lock exhaustion retains the workspace and actionable diagnostics, cancellation prevents another attempt, and representative non-lock Git failures are not retried. (verification: integration - `cargo test transient_wip_commit_lock`; verification-id: transient-wip-lock-tests)

## Implementation Evidence

- Classifier and retry policy: `src/execution/wip_lock_retry.rs`
- Snapshot orchestration boundary (retry plus cancellation): `src/execution/apply.rs` (`create_progress_commit`, `create_progress_commit_with_environment`)
- Call sites updated for the cancellation parameter: `src/execution/apply.rs` (apply loop), `src/orchestrator.rs` (serial snapshot)
- Unit evidence (in-memory doubles only): `src/execution/wip_lock_retry.rs` `mod tests` - classifier, three-attempt limit, fixed 200 ms delay through the injected sleeper, cancellation, ambiguous-success deduplication
- Integration evidence (real Git processes, real temporary repositories): `src/execution/wip_lock_retry_git_tests.rs` - lock recovery, exhaustion diagnostics, cancellation, non-retryable identity failure, ambiguous success, same-subject history rejection, managed lock identity
- `cargo test --lib transient_wip_commit_lock` -> 29 passed; 0 failed (1.63s)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-transient-wip-commit-lock-retry --archive-gate`

## Future Work

- Consider a shared transient-local-VCS retry primitive only if another independently verified Git operation needs the same policy.

## Implementation Tasks

- [x] Add a final-Apply lock-contention classifier requiring a structured Git command from finalization (`git add -A`, add-and-commit, or `git commit --amend --allow-empty`), existing-`index.lock` stderr, and a lock path resolving to the current managed worktree; reject hook failures, other commands, worktrees, backends, and near-match prose. (verification: unit - `cargo test --lib final_apply_commit_lock_classifier`; verification-id: final-apply-lock-retry-tests)
- [x] Add a finalization retry boundary with three total attempts, fixed 200 ms delay, no backoff, injected timing, and cancellation checks before delay and before another attempt; retry the complete path-specific preparation rather than a generic individual Git command. (verification: unit - `cargo test --lib final_apply_commit_lock_retry_policy`; verification-id: final-apply-lock-retry-tests)
- [x] Preserve hook-enabled finalization and the existing typed `RepositoryRejected` Apply repair route on every attempt; lock exhaustion remains terminal and must not consume the Apply-agent hook-repair budget. (verification: unit - `cargo test --lib -- apply_commit_recovery final_apply_commit_lock`; verification-id: final-apply-lock-retry-tests)
- [x] Make ambiguous final commit success idempotent by validating exact new HEAD lineage, `Apply: <change-id>` subject, and expected committed tree before returning success; a same-subject historical commit or mismatched tree must not count. (verification: integration - `cargo test --lib final_apply_commit_lock_ambiguous_success`; verification-id: final-apply-lock-retry-tests)
- [x] Add temporary-repository tests for real lock acquisition and release during both dirty add-and-commit and clean amend finalization, asserting one final commit, hooks enabled, expected tree contents, and no lock deletion. (verification: integration - `cargo test --lib final_apply_commit_lock_recovers`; verification-id: final-apply-lock-retry-tests)
- [x] Add exhaustion, cancellation, wrong-worktree lock, malformed stderr, permission failure, and hook-rejection tests proving only eligible contention retries and all failure paths preserve actionable diagnostics and workspace state. (verification: integration - `cargo test --lib final_apply_commit_lock`; verification-id: final-apply-lock-retry-tests)

## Notes

- Implementation lives in `src/execution/final_commit_lock_retry.rs` (classifier, retry
  boundary, ambiguous-success proof, injectable `FinalCommitEnvironment`) with
  repository-level tests in `src/execution/final_commit_lock_retry_git_tests.rs`.
  `src/execution/apply.rs` wires the boundary into `create_final_commit` through the new
  `create_final_commit_with_environment`, which also threads the apply loop's cancellation
  token into finalization.
- `src/execution/index_lock.rs` holds the lock-evidence primitives the WIP snapshot policy
  and this policy must agree on (existing-`index.lock` stderr grammar, lexical path
  normalization, managed-worktree lock-path resolution).
  `src/execution/wip_lock_retry.rs` now consumes them instead of keeping private copies, so
  the two independent retry policies cannot drift on how a lock failure is recognised. The
  retry semantics themselves stay separate, as the proposal requires.
- Ambiguous success is proven per finalization path: the add-and-commit path requires the
  captured HEAD to be the new commit's sole parent, the amend path requires the captured
  HEAD's parents and tree to be inherited unchanged, and both require the exact
  `Apply: <change-id>` subject plus a worktree left with nothing uncommitted.
- Recovery tests release a real `index.lock` from the injected `sleep` rather than from a
  wall-clock timer: a fixed-delay releaser races the first Git spawn, so an attempt could
  finish without ever seeing contention and the test would pass vacuously.
- The declared verification command for the hook-preservation task was
  `cargo test --lib apply_commit_recovery final_apply_commit_lock`; cargo accepts only one
  positional filter, so the task now records the equivalent working form
  `cargo test --lib -- apply_commit_recovery final_apply_commit_lock`.
- evidence: `cargo test --lib` passed with 3016 passed, 0 failed, 9 ignored
- evidence: `cargo clippy --all-targets -- -D warnings` passed with no warnings

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate retry-final-apply-commit-lock-contention --archive-gate`

## Future Work

- Introduce a shared local-VCS retry abstraction only if another independently specified operation needs identical classification, identity, and idempotency semantics.

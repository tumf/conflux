## Implementation Tasks

- [ ] Add a final-Apply lock-contention classifier requiring a structured Git command from finalization (`git add -A`, add-and-commit, or `git commit --amend --allow-empty`), existing-`index.lock` stderr, and a lock path resolving to the current managed worktree; reject hook failures, other commands, worktrees, backends, and near-match prose. (verification: unit - `cargo test --lib final_apply_commit_lock_classifier`; verification-id: final-apply-lock-retry-tests)
- [ ] Add a finalization retry boundary with three total attempts, fixed 200 ms delay, no backoff, injected timing, and cancellation checks before delay and before another attempt; retry the complete path-specific preparation rather than a generic individual Git command. (verification: unit - `cargo test --lib final_apply_commit_lock_retry_policy`; verification-id: final-apply-lock-retry-tests)
- [ ] Preserve hook-enabled finalization and the existing typed `RepositoryRejected` Apply repair route on every attempt; lock exhaustion remains terminal and must not consume the Apply-agent hook-repair budget. (verification: unit - `cargo test --lib apply_commit_recovery final_apply_commit_lock`; verification-id: final-apply-lock-retry-tests)
- [ ] Make ambiguous final commit success idempotent by validating exact new HEAD lineage, `Apply: <change-id>` subject, and expected committed tree before returning success; a same-subject historical commit or mismatched tree must not count. (verification: integration - `cargo test --lib final_apply_commit_lock_ambiguous_success`; verification-id: final-apply-lock-retry-tests)
- [ ] Add temporary-repository tests for real lock acquisition and release during both dirty add-and-commit and clean amend finalization, asserting one final commit, hooks enabled, expected tree contents, and no lock deletion. (verification: integration - `cargo test --lib final_apply_commit_lock_recovers`; verification-id: final-apply-lock-retry-tests)
- [ ] Add exhaustion, cancellation, wrong-worktree lock, malformed stderr, permission failure, and hook-rejection tests proving only eligible contention retries and all failure paths preserve actionable diagnostics and workspace state. (verification: integration - `cargo test --lib final_apply_commit_lock`; verification-id: final-apply-lock-retry-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate retry-final-apply-commit-lock-contention --archive-gate`

## Future Work

- Introduce a shared local-VCS retry abstraction only if another independently specified operation needs identical classification, identity, and idempotency semantics.

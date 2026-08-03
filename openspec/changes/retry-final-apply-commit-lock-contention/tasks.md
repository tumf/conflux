## Implementation Tasks

- [ ] Consume the dependency's typed confirmed-quiescence result at final Apply entry and reject retry setup when cleanup is unconfirmed; add tests proving zero retry attempts before quiescence. (verification: unit - `cargo test --lib final_apply_commit_lock_requires_quiescence`; verification-id: final-apply-lock-retry-tests)
- [ ] Build an immutable finalization plan using `git --no-optional-locks status --porcelain`, baseline HEAD/parents, fixed add-or-amend mode, exact subject, and expected tree from an ephemeral isolated index; prove snapshot generation includes staged, unstaged, deleted, and untracked files without mutating the real index. (verification: unit - `cargo test --lib final_apply_commit_lock_plan`; verification-id: final-apply-lock-retry-tests)
- [ ] Implement retry preflight that recognizes mode-specific exact success first, otherwise requires baseline HEAD plus the same isolated-index full tree and compatible real index; return terminal concurrent-mutation diagnostics on any drift and never switch mode. (verification: unit - `cargo test --lib final_apply_commit_lock_drift`; verification-id: final-apply-lock-retry-tests)
- [ ] Add narrow structured classification for final `git add -A` and fixed verified commit lock-acquisition failures, requiring current managed-worktree lock identity and non-hook-rejection status; use three total attempts, fixed 200 millisecond injected delay, and cancellation checks. (verification: unit - `cargo test --lib final_apply_commit_lock_retry_policy`; verification-id: final-apply-lock-retry-tests)
- [ ] Implement separate ambiguous-success proofs: add-and-commit must be a sole child of baseline HEAD; amend must preserve baseline HEAD's ordered parent set; both require exact subject and expected tree. (verification: unit - `cargo test --lib final_apply_commit_lock_ambiguous_success`; verification-id: final-apply-lock-retry-tests)
- [ ] Add temporary-repository tests for real add/amend lock recovery and a counting pre-commit hook, asserting failed eligible attempts run zero hooks, eventual success runs one hook, hook rejection enters existing Apply repair, and unsupported platform behavior fails terminally. (verification: integration - `cargo test --lib final_apply_commit_lock_hooks`; verification-id: final-apply-lock-retry-tests)
- [ ] Add race and failure coverage for external HEAD advance, staged/unstaged/untracked arrivals, mode-changing drift, wrong-worktree lock, near-match stderr, permission/configuration/conflict errors, cancellation, and three-attempt exhaustion; assert no external content is staged/amended and no lock is deleted. (verification: integration - `cargo test --lib final_apply_commit_lock`; verification-id: final-apply-lock-retry-tests)
- [ ] Preserve existing commit-hook repair behavior and verify the complete finalization suite with separate valid Cargo filters. (verification: integration - `cargo test --lib final_apply_commit_lock && cargo test --lib apply_commit_recovery`; verification-id: final-apply-lock-retry-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate retry-final-apply-commit-lock-contention --archive-gate`

## Future Work

- Introduce a shared local-VCS retry abstraction only if another operation needs the same immutable-plan, drift, hook, identity, and idempotency contract.

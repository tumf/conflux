## Implementation Tasks

- [ ] Add structural classification for final Apply commit outcomes so commit-hook rejection preserves command, working directory, available exit status, stdout, and stderr while unrelated VCS failures remain terminal. (verification: unit - `cargo test --lib apply_commit_recovery` exercises synthetic structured VCS outcomes without invoking real Git or hooks; verification-id: apply-commit-recovery-tests)

- [ ] Extend Apply history and prompt construction to carry bounded, untrusted orchestration-originated commit diagnostics into the next Apply iteration with explicit repair and validation-rerun instructions. (verification: unit - `cargo test --lib apply_commit_recovery` asserts generated prompts include diagnostic fields, truncate oversized output, preserve trust-boundary wording, and do not instruct final commit bypass; verification-id: apply-commit-recovery-tests)

- [ ] Wire commit-hook recovery into the shared Apply loop so the same workspace receives a repair iteration, final commit is retried with hooks enabled, retries consume `max_iterations`, and completed results are emitted only after commit success. (verification: integration - `cargo test --lib apply_commit_recovery` uses fake workspace and agent boundaries to prove reject-then-repair success, repeated rejection exhaustion, terminal non-hook failure, and no premature completed result; verification-id: apply-commit-recovery-tests)

- [ ] Preserve serial and parallel error propagation while removing frontend-level reliance on generic resume for commit-hook recovery. (verification: integration - `cargo test --lib apply_commit_recovery` verifies shared-loop outcomes observed by caller fixtures and confirms terminal VCS errors still surface as Apply failures; verification-id: apply-commit-recovery-tests)

- [ ] Run repository quality gates and fix any warnings introduced by the recovery path. (verification: integration - `cargo clippy -- -D warnings` and `cargo test --lib apply_commit_recovery` pass with final commit verification still enabled; verification-id: apply-commit-recovery-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate retry-apply-after-commit-hook-failure --archive-gate`

The implementation must also pass `cargo test --lib apply_commit_recovery` and `cargo clippy -- -D warnings`.

## Future Work

- General recovery policy for non-commit Git failures remains outside this change.

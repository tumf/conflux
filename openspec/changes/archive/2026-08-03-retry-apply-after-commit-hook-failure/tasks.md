## Implementation Tasks

- [x] Add a typed final-commit outcome and preserve the actual Git exit code so the final `git commit` invocation can distinguish repository rejection from spawn, setup, validation, and fatal VCS failures without parsing rendered error strings. (verification: unit - `cargo test --lib apply_commit_recovery` exercises typed committed, repository-rejected, and terminal outcomes with exit-code assertions; verification-id: apply-commit-recovery-tests)

- [x] Make both finalization paths propagate verified commit failure: dirty-tree add-and-commit and clean-tree amend must return structured rejection instead of logging and converting amend failure to success. (verification: integration - `cargo test --lib apply_commit_recovery` creates temporary real Git repositories with failing `core.hooksPath` hooks and proves both paths preserve exit code/stdout/stderr and do not report completion; verification-id: apply-commit-recovery-tests)

- [x] Extend Apply history and prompt construction to carry orchestration-originated commit diagnostics through a dedicated recording API, reusing bounded tail collection and explicit untrusted-output wrapping while preserving the final-commit-only scope of verification-bypass guidance. (verification: unit - `cargo test --lib apply_commit_recovery` asserts generated prompts include bounded diagnostic fields, resist embedded instructions, and do not universally prohibit WIP snapshot `--no-verify`; verification-id: apply-commit-recovery-tests)

- [x] Wire commit-hook recovery into the shared Apply loop so pending repair bypasses the task-complete short circuit, dispatches one real Apply agent command, retries final commit with hooks enabled, consumes `max_iterations`, and emits completion only after commit success. (verification: integration - `cargo test --lib apply_commit_recovery` asserts agent dispatch count between rejection and retry, reject-then-repair success, repeated rejection exhaustion, terminal non-hook failure, and zero Acceptance dispatch before commit success; verification-id: apply-commit-recovery-tests)

- [x] Preserve serial and parallel error propagation while removing frontend-level reliance on generic resume for commit-hook recovery. (verification: integration - `cargo test --lib apply_commit_recovery` verifies shared-loop outcomes observed by caller fixtures and confirms terminal VCS errors still surface as Apply failures; verification-id: apply-commit-recovery-tests)

- [x] Run repository quality gates and fix any warnings introduced by the recovery path. (verification: integration - `cargo clippy -- -D warnings` and `cargo test --lib apply_commit_recovery` pass with final commit verification still enabled; verification-id: apply-commit-recovery-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate retry-apply-after-commit-hook-failure --archive-gate`

The implementation must also pass `cargo test --lib apply_commit_recovery` and `cargo clippy -- -D warnings`.

## Notes

- evidence: `cargo test --lib apply_commit_recovery` - 16 passed, 0 failed (0.71s)
- evidence: `cargo clippy -- -D warnings` and `cargo clippy --all-targets -- -D warnings` - clean
- evidence: `cargo test --lib` - 2766 passed, 0 failed, 7 ignored
- The amend finalization path now passes `--allow-empty`. Without it, amending an
  empty WIP snapshot exits 1 for "would make it empty", which is
  indistinguishable from a hook rejection by exit code and would spend the whole
  repair budget on a failure no agent can fix. This is consistent with
  `create_iteration_snapshot` and `squash_wip_commits`, which already allow empty
  commits: the final commit names the completed apply rather than introducing
  content.
- The `WorkspaceManager::set_commit_message` boundary was replaced by
  `create_verified_commit`, which returns the typed outcome. The old method could
  only return `Ok(())`, which is what allowed an amend failure to be logged and
  reported as success.
- Some recovery tests can appear to take ~60s on macOS. That is an environmental
  first-process-exec stall on this machine (reproducible outside cargo with any
  freshly executed script, and it also affects pre-existing hook tests such as
  `execution::archive::tests::test_direct_archive_commit_fallback_to_ai_resolve_on_pre_commit_failure`).
  Warm, the whole `apply_commit_recovery` set runs in 0.71s, so the suite stays
  within the repository's one-second unit-test budget.

## Future Work

- General recovery policy for non-commit Git failures remains outside this change.
- Replacing clean-tree amend finalization with a soft-reset and normal commit may later make staged-diff-based hooks validate the complete change delta.
- The obsolete legacy serial squash path outside the shared Apply loop remains outside this change.

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
  captured HEAD to be the new commit's sole parent *and* the pre-attempt workspace tree to
  be the recorded tree, the amend path requires the captured HEAD's parents and tree to be
  inherited unchanged, and both require the exact `Apply: <change-id>` subject plus a
  worktree left with nothing uncommitted. Subject, parent and post-attempt cleanliness are
  all reproducible by another actor committing on the same HEAD, so the expected tree is
  what makes the proof exclusive.
- The expected dirty-worktree tree must be captured *before* the attempt: afterwards the
  worktree is indistinguishable from one another actor rewrote. It is computed by replaying
  `git add -A` and `git write-tree` against a throwaway copy of the index selected with
  `GIT_INDEX_FILE`, which keeps the capture a pure observation - the real index is never
  written and its `index.lock` is never taken, which matters because this runs exactly when
  another process may hold that lock. The scratch-index Git invocation is local to
  `final_commit_lock_retry.rs` rather than added to the shared Git helpers, so no general
  caller gains the ability to redirect Git's index or object store.
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

## Current Acceptance Follow-up
- attempt: 1
- [x] `src/execution/final_commit_lock_retry.rs:200-209,272-298` の dirty/add-and-commit 曖昧成功判定は試行前の期待 workspace tree を保存・比較せず、親・subject・事後 clean のみで成功扱いするため、同じ subject と親を持つ別 tree の commit を誤認できる。期待 dirty-workspace tree を厳密に比較し、same-subject/same-parent/clean だが tree 不一致の unit test と temporary-repository test を追加すること。
  evidence: `FinalizationState.workspace_tree` captures the pre-attempt dirty-worktree tree via `FinalCommitEnvironment::workspace_tree`, and the add-and-commit branch of `final_commit_recorded` now rejects any observed commit whose tree differs from it
  evidence: `GitFinalCommitEnvironment::workspace_tree` replays `git add -A` plus `git write-tree` against a throwaway copy of the index through `GIT_INDEX_FILE`, so the expected tree is observed without writing the managed worktree's index or taking its `index.lock`
  evidence: unit `final_apply_commit_lock_ambiguous_success_rejects_mismatched_tree_on_the_add_path` builds a same-subject, same-parent, worktree-clean commit of other content and asserts 3 attempts then `did not clear`
  evidence: temporary-repository `git_tests::final_apply_commit_lock_ambiguous_success_rejects_mismatched_tree_on_the_add_path` lands a real same-subject/same-parent commit of tampered content and asserts exhaustion plus an untouched foreign commit and worktree
  evidence: temporary-repository `git_tests::final_apply_commit_lock_environment_workspace_tree_matches_the_committed_tree` proves the captured tree equals the tree real `git add -A` plus `git commit` records over addition, removal and ignored-file cases, with nothing staged in the real index and no `index.lock` taken
  evidence: both new mismatch tests fail with `Committed` when the tree comparison is removed, so they are non-vacuous
  evidence: `cargo test --lib` passed with 3019 passed, 0 failed, 9 ignored; `cargo clippy --all-targets -- -D warnings` passed with no warnings

## Implementation Tasks

- [x] Add `SequentialMergeItem` in `src/parallel/merge.rs` and carry ordered `revision`, `change_id`, `archive_path`, and any admission-time branch base through `attempt_merge`, `merge_and_resolve`, `merge_and_resolve_with`, and `ResolveMergesWithRetryArgs`, rejecting cardinality/order loss before agent execution. (verification: unit - `cargo test parallel::` proves empty manager workspace lists retain supplied evidence and malformed batches fail before resolve; verification-id: resolve-continuation-tests)

- [x] Add exact branch-aware validation and stale-path rediscovery in `src/vcs/git/commands/worktree.rs`; remove process-local path/base reconstruction and all missing-path `continue` branches from `src/parallel/conflict.rs`. (verification: integration - `cargo test vcs::git::commands::worktree parallel::tests::conflict` covers registered path, one exact rediscovery, missing/ambiguous path, wrong repository/branch, detached HEAD, and Git errors; verification-id: resolve-continuation-tests)

- [x] Add Git helpers in `src/vcs/git/commands/merge.rs` for exact candidate enumeration, complete parent lists, first-parent-lineage checks, `MERGE_HEAD` identity, index conflict stages, committed-tree paths, and clean tracked/untracked status. (verification: unit/integration - `cargo test vcs::git::commands::merge` covers zero/one/multiple exact candidates, two-parent topology, false first-parent-only ancestry, stage-0 versus conflict stages, HEAD/index/worktree disagreement, and untracked dirt; verification-id: resolve-continuation-tests)

- [x] Implement repository-derived target state `T` and pre-sync validation in `src/parallel/conflict.rs`: first-parent inclusion needs no merge; otherwise require one reachable two-parent `Pre-sync base into <change_id>` commit whose non-first parent is exactly `T`; exempt only historical ancestry-only integration. (verification: integration - `cargo test parallel::tests::conflict` covers valid first-parent inclusion, valid pre-sync merge, missing/multiple/wrong-parent candidates, target `MERGE_HEAD` with valid/invalid pre-sync, and historical ancestry exemption; verification-id: resolve-continuation-tests)

- [x] Implement batch-aware target `MERGE_HEAD` owner selection and ordered state classification: uniquely identify the owner before item evaluation, require all prior items committed complete, require owner to be first incomplete, and require all items plus clean target state for batch completion. (verification: integration - `cargo test parallel::tests::conflict` covers A-complete/B-in-progress success path, unknown/ambiguous owner, earlier incomplete item, later out-of-order integration, and multi-change clean completion; verification-id: resolve-continuation-tests)

- [x] Unify retry and `verify_merge_commits` final identity policy: one exact candidate, exact subject, two parents, first parent `T`, non-first parent validated branch tip; allow ancestry fallback only when no exact candidate exists. (verification: integration - `cargo test parallel::` covers right topology, expected revision only on first-parent side, wrong second parent, multiple candidates, no-candidate/not-ancestral failure, and no-candidate/already-ancestral success in both callers; verification-id: resolve-continuation-tests)

- [x] Extract pure archive name/layout predicates in `src/archive_layout.rs` and add pre-final worktree-HEAD, in-progress target-index, and post-final target-HEAD adapters; align runtime and `skills/cflx-resolve/SKILL.md` on active `proposal.md` identities and reject conflict-stage cleanup. (verification: integration - `cargo test parallel:: vcs::git::commands::` covers exact/dated archives, nested/unrelated/suffix collisions, worktree-only archive evidence, target stage-0 evidence, conflict stages, and committed HEAD evidence; verification-id: resolve-continuation-tests)

- [x] Add post-final forward cleanup protocol to classifier and embedded skill using exact `Cleanup resurrected change: <change_id>`; verify one-parent predecessor identity, deletion-only live subtree diff, unchanged archive, clean index/worktree, and full batch re-verification. (verification: integration - `cargo test parallel::tests::conflict embedded_skills` proves staged-only/unstaged/mixed/amend/wrong-diff cleanup remains incomplete and only a valid clean forward cleanup commit reaches success; verification-id: resolve-continuation-tests)

- [x] Remove the target conflict-free auto-commit bypass and route identity-verified target merges through normal phase continuation; never generate combined `Merge changes: ...` commits. (verification: integration - `cargo test parallel::tests::conflict` proves no direct commit occurs before owner, pre-sync, index, cleanup, and terminal checks; verification-id: resolve-continuation-tests)

- [x] Add resolve-specific 2 KiB stream-tail and wrapper-inclusive 8 KiB context bounds in `src/history.rs` without changing shared collector defaults; trim oldest attempts, older streams, then newest stream detail while retaining newest structured diagnosis and valid UTF-8. (verification: unit - `cargo test history::` covers oversized ASCII/multibyte lines, repeated prompt echo, max-attempt retention, deterministic trim order, wrapper inclusion, and diagnosis-only overflow; verification-id: resolve-continuation-tests)

- [x] Add the full reported regression using a valid supplied archive path absent from `workspace_manager.workspaces()`, incomplete pre-sync, phase continuation, final merge, resurrection handling, and clean committed terminal success; keep default tests under one second or mark impractical cases heavy. (verification: integration - `cargo test parallel::tests::conflict` fails on `(unknown)`, skipped evidence, generic fallback, false exact topology, staged-only cleanup, or dirty terminal acceptance; verification-id: resolve-continuation-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-resolve-merge-continuation --archive-gate`

The implementation must also pass `cargo test parallel:: && cargo test history:: && cargo test vcs::git::commands:: && cargo test embedded_skills`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings`.

## Notes

- evidence: `cargo test --lib` passes (2831 passed, 0 failed); `cargo test --lib --features heavy-tests parallel::tests::conflict` passes (32 passed) including the full real-Git regression `sequential_resolve_tracks_the_full_reported_regression`.
- evidence: `cargo fmt --all -- --check` and `cargo clippy --lib --tests --all-features` are clean.
- The batch classifier lives in the new `src/parallel/resolve_state.rs`, behind the `ResolveEvidence` trait; `src/parallel/conflict.rs` consumes it and `src/parallel/merge.rs` shares the same verifier through `verify_final_integration`.
- Classifier decision-table tests use an in-memory commit-graph double (unit-scoped, isolated from process/filesystem boundaries); the end-to-end reported regression uses a real temporary Git repository and is gated behind `heavy-tests` because it takes ~2.5s.
- The superseded subject-only helpers `missing_merge_commits_since`, `merge_commit_hash_by_subject_since`, `presync_merge_subject_mismatches_since`, and `first_parent_of` were deleted so the exact-topology policy cannot re-diverge.
- Acceptance repair attempt 1 (4 findings): `is_invalid_nested_archive_path` is now reachable from the runtime through the new `archive_layout::paths_contain_invalid_nested_archive`, which the pre-final worktree-HEAD archive gate calls; `BatchState::allows_agent_action` now gates `resolve_merges_with_retry`, which fails closed through the shared `fail_resolve` helper instead of re-invoking the agent on unproven identity.
- Repair verification: `cargo clippy --locked --all-targets --all-features -- -D warnings` (the exact `.pre-commit-config.yaml` hook command) exits 0, as does the `--locked --bin cflx --all-features` command from the finding evidence; `cargo fmt --all -- --check` is clean; `cargo test --lib` passes 2834/2834 (3 new classifier tests); `cargo test --lib --features heavy-tests parallel::tests::conflict` passes including the real-Git regression.
- Pre-existing, out of scope for these findings: 5 `parallel::tests::executor` tests fail only under `--features heavy-tests`. The identical 5 fail on the unmodified baseline (verified by stashing this repair), so they are not caused by it. Their fixtures build change branches that never archive the change, which the default suite and the proposal's `cargo test parallel::` rerun command do not exercise.

## Future Work

- Deterministic Conflux-owned identity-verified Git commits may be proposed separately if correct continuation remains operationally insufficient.
- Repair of already preserved failed worktrees remains an explicit operator action.

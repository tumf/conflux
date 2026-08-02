## Implementation Tasks

- [ ] Add an ordered sequential merge input record in `src/parallel/merge.rs` and carry each `revision`, `change_id`, and `archive_path` through `attempt_merge`, `merge_and_resolve`, `merge_and_resolve_with`, and `ResolveMergesWithRetryArgs`, enforcing cardinality/order at the boundary. (verification: unit - `cargo test parallel::` proves empty process-local workspace lists still retain valid supplied paths and mismatched input cardinality fails before agent execution; verification-id: resolve-continuation-tests)

- [ ] Add repository-local Git worktree path validation/rediscovery helpers and fail-closed evidence results for missing, stale, ambiguous, wrong-repository, wrong-branch, detached-HEAD, and Git-query-failed paths; remove `Option`-based verification skips in `src/parallel/conflict.rs`. (verification: integration - `cargo test parallel::` uses temporary worktrees to prove valid stale paths are rediscovered by exact branch while every unverifiable identity becomes actionable `UnsafeEvidence`; verification-id: resolve-continuation-tests)

- [ ] Implement the closed side-effect-free sequential state classifier in `src/parallel/conflict.rs`, with ordered `UnsafeEvidence`, identity-verified target merge, identity-verified pre-sync, pre-sync validity, final integration, resurrection, and terminal states; validate `MERGE_HEAD` parent identity before suggesting any commit. (verification: unit - `cargo test parallel::conflict` covers state precedence and multi-change order; integration - `cargo test parallel::tests::conflict` covers wrong target/worktree `MERGE_HEAD`, conflicts, expected parentage, wrong parentage, and no false completion; verification-id: resolve-continuation-tests)

- [ ] Unify final integration verification used by `resolve_merges_with_retry` and `verify_merge_commits`: exact `Merge change: <change_id>` evidence must integrate the expected revision, while a revision already ancestral to target `HEAD` remains an accepted idempotent fast-forward/already-integrated state. (verification: integration - `cargo test parallel::` covers exact-subject/right-parent success, exact-subject/wrong-parent failure, missing-subject/not-ancestral failure, and missing-subject/already-ancestral success in both verification callers; verification-id: resolve-continuation-tests)

- [ ] Route the pre-loop conflict-free target `MERGE_HEAD` shortcut through the classifier and normal resolve cycle so it cannot bypass merge identity, per-change ordering, pre-sync checks, resurrection cleanup, or terminal verification, and never creates a combined `Merge changes: ...` commit for ordered per-change integration. (verification: integration - `cargo test parallel::tests::conflict` proves unrelated/ambiguous target merges fail closed and an expected conflict-free merge does not complete until cleanup and terminal predicates pass; verification-id: resolve-continuation-tests)

- [ ] Reuse `archive_layout::invalid_layout_error` and `find_valid_archive_entry` for phase-specific exact/date-prefixed archive identity; align runtime and `skills/cflx-resolve/SKILL.md` on the active live-change predicate; diagnose resurrection both before/during final merge and after integration. (verification: integration - `cargo test parallel::` proves worktree-only archive evidence predicts cleanup, target index-visible archive/live coexistence blocks completion, exact/date-prefixed layouts work, and nested/unrelated/suffix-collision layouts never authorize deletion; verification-id: resolve-continuation-tests)

- [ ] Emit phase-specific continuation through existing `ResolveOutput` and `ResolveContext`, including validated path, completed/next phase, target state, exact subject, and cleanup requirement; update `skills/cflx-resolve/SKILL.md` in English to resume only from identity-validated guidance and stop safely on `UnsafeEvidence`. (verification: unit - `cargo test parallel::conflict embedded_skills` checks prompt formatting and actual embedded skill bytes, while integration tests prove pre-sync-only state requests final merge without repeating pre-sync; verification-id: resolve-continuation-tests)

- [ ] Bound `OutputCollector`/`ResolveContext` in `src/history.rs` to 2 KiB per stdout/stderr tail, 8 KiB total injected context, UTF-8-safe truncation, newest diagnosis retention, and at most configured retry count. (verification: unit - `cargo test history::` uses oversized ASCII and multibyte lines plus repeated prompt echo to prove all byte/attempt limits and retention of the newest phase diagnosis; verification-id: resolve-continuation-tests)

- [ ] Add a temporary-Git regression reproducing the reported path-loss failure: archived branch and live target, valid archive path absent from `workspace_manager.workspaces()`, incomplete pre-sync, phase diagnosis, final merge cleanup, and terminal success; keep each default-path test under one second or mark impractical cases heavy. (verification: integration - `cargo test parallel::tests::conflict` fails if the supplied path becomes `(unknown)`, worktree checks are skipped, generic missing-commit guidance replaces the phase diagnosis, cleanup is omitted, or final evidence is falsely accepted/rejected; verification-id: resolve-continuation-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-resolve-merge-continuation --archive-gate`

The implementation must also pass `cargo test parallel:: && cargo test history:: && cargo test embedded_skills`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings`.

## Future Work

- Deterministic Conflux-owned completion of identity-verified conflict-free merges may be proposed separately if correct diagnosis and agent continuation remain operationally insufficient.
- Operational repair of already preserved failed worktrees remains an explicit operator action.

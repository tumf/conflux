## Implementation Tasks

- [ ] Add a pure sequential-merge continuation classifier that derives the earliest unfinished phase and bounded evidence from target-repository/worktree Git state for every `(revision, change_id)` pair, without using agent prose, exit status, logs, or durable external state. (verification: unit - `cargo test parallel::tests::conflict` covers unfinished target merge, unfinished worktree pre-sync, missing/invalid pre-sync evidence, missing final merge, and completed integration classifications; verification-id: resolve-continuation-tests)

- [ ] Wire the classifier into post-attempt verification and existing `ResolveOutput`/`ResolveContext` continuation history so retries identify the exact repository/worktree path, completed phase, required next phase, target branch, and exact commit subject while preserving all existing terminal success checks. (verification: integration - `cargo test parallel::tests::conflict` proves a pre-sync-only attempt receives final-merge continuation, a worktree `MERGE_HEAD` receives pre-sync completion guidance, and neither state emits `ConflictResolutionCompleted`; verification-id: resolve-continuation-tests)

- [ ] Detect valid exact and date-prefixed archive entries when diagnosing final merge, and include mandatory removal of a resurrected live `openspec/changes/<change_id>` before `Merge change: <change_id>` whenever both forms exist. (verification: integration - `cargo test parallel::tests::conflict` uses temporary OpenSpec trees to prove exact/date-prefixed archives trigger cleanup guidance while unrelated/invalid archive entries do not; verification-id: resolve-continuation-tests)

- [ ] Update the embedded `skills/cflx-resolve/SKILL.md` retry continuation contract in English so the agent resumes from the diagnosed incomplete phase, does not repeat a verified pre-sync, and completes all remaining sequential protocol steps including resurrection cleanup and hook-modified re-commit handling before returning. (verification: unit - `cargo test parallel::conflict` verifies generated retry prompts retain the skill prelude and phase-specific completion contract without duplicating fixed instructions in the variable prompt builder; verification-id: resolve-continuation-tests)

- [ ] Add temporary-Git regression coverage for the reported convergence failure: archive a change on its branch, leave the live change on the target branch, complete pre-sync only, verify actionable final-merge continuation, then complete final merge with resurrection cleanup and verify the existing merge/ancestry checks accept the result. (verification: integration - `cargo test parallel::tests::conflict` fails if pre-sync-only state is accepted, cleanup is omitted, the exact final subject is absent, or completed integration remains classified as retryable; verification-id: resolve-continuation-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-resolve-merge-continuation --archive-gate`

The implementation must also pass `cargo test parallel::tests::conflict`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings`.

## Future Work

- Deterministic Conflux-owned completion of conflict-free pre-sync and final merge can be proposed separately if phase-specific agent continuation still proves insufficient in production.
- Operational repair of already preserved failed worktrees remains an explicit operator action.

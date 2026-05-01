## Implementation Tasks

- [x] Update reducer blocker classification so automatic `ResolveWait` is created only when another non-terminal change is actively `Resolving` or `Rejecting`. (verification: unit - reducer tests in `src/orchestration/state.rs` assert `ChangeArchived` with other resolving/rejecting becomes `resolve pending`, while other applying/accepting/archiving/terminal rejected/no active blocker does not)
- [x] Update TUI post-archive dispatch so automatic resolve-pending events are emitted only for the eligible resolving/rejecting blocker set. (verification: unit - tests in `src/tui/orchestrator.rs` cover post-archive dispatch with resolving, rejecting, applying, accepting, terminal rejected, and no active blocker)
- [x] Update merge-deferred event handling so `auto_resumable=true` is produced from structured blocker classification, not free-form reason parsing, and manual dirty-base deferrals remain `MergeWait`. (verification: unit - tests in `src/parallel/merge.rs` or `src/parallel/tests/executor.rs` prove applying/accepting blockers do not enter `resolve_wait_changes` and dirty-base/manual deferrals remain merge wait)
- [x] Wire rejection-review completion/failure to retry deferred resolve-pending merge work when rejecting was the blocker. (verification: integration - tests in `src/parallel/queue_state.rs` or parallel executor tests prove `RejectionReviewCompleted`/`RejectionReviewFailed` causes `retry_deferred_merges` or equivalent scheduler retry path to run for existing ResolveWait entries)
- [x] Preserve explicit user/scheduler resolve intent for existing `MergeWait` rows. (verification: unit - existing or new tests in `src/orchestration/state.rs` and `src/tui/state.rs` prove `ReducerCommand::ResolveMerge` still transitions an eligible `MergeWait` row to `ResolveWait`)
- [x] Run focused verification for touched behavior and the repository-required validation commands. (verification: manual - record successful `cargo test` target(s), lint/typecheck commands if available, and `cflx openspec validate restrict-resolve-pending-blockers --strict --evidence warn` output in implementation notes or acceptance evidence)

## Future Work

- Broader merge conflict UX improvements remain separate from this blocker-classification fix.

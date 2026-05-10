## Implementation Tasks

- [ ] Verify archive completion against a stable existing repository root during merge attempts, not a possibly deleted worktree path. (verification: unit - add/extend `src/parallel/merge.rs` tests so `attempt_merge` with a deleted workspace path does not return a `Failed to check git status: No such file or directory` deferral)
- [ ] Add stale retry handling before deferred merge retry invokes archive verification on workspace paths. (verification: integration - add/extend `src/parallel/tests/executor.rs` or adjacent queue-state tests so a `ResolveWait` retry whose worktree path has been deleted converges by clearing/suppressing retry intent when repository evidence shows no valid retry worktree)
- [ ] Preserve legitimate manual merge blocker behavior for existing roots. (verification: unit - add/extend `src/parallel/merge.rs` or `src/parallel/tests/executor.rs` tests where the base repository root exists but is dirty or in conflict, and the merge attempt remains `MergeDeferred(auto_resumable=false)` / `MergeWait`)
- [ ] Deduplicate repeated identical merge-deferred TUI diagnostics. (verification: unit - add/extend `src/tui/state/event_handlers/errors.rs` tests so repeated `MergeDeferred` events with the same `change_id`, `reason`, and `auto_resumable` append at most one visible warning until the reason changes)
- [ ] Prove distinct merge-deferred reasons remain visible. (verification: unit - add/extend `src/tui/state/event_handlers/errors.rs` tests so a later `MergeDeferred` for the same change with a different reason produces a new warning)
- [ ] Run focused Rust regression tests for merge retry and TUI warning behavior. (verification: integration - run targeted `cargo test` filters for the added `src/parallel` and `src/tui/state/event_handlers/errors.rs` tests)

## Future Work

- Full end-to-end reproduction in a live multi-change Conflux run may be useful after implementation, but local unit/integration tests are the required acceptance evidence.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-stale-merge-retry-worktree-status --archive-gate`

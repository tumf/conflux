## Implementation Tasks

- [ ] Rename the executor helper or document its broader outcome semantics. (verification: unit - update `src/parallel/queue_state.rs` so `clear_resolve_wait_intent_for_success` is renamed to an outcome-neutral helper such as `clear_resolve_wait_intent_for_outcome`, or add explicit comments at the helper and stale/missing-workspace call sites; completion condition: stale/missing workspace cleanup no longer reads as success-only behavior)
- [ ] Update all internal call sites without changing behavior. (verification: unit - compile references in `src/parallel/queue_state.rs` and related modules; completion condition: already-merged, merge success, missing workspace, and stale workspace paths still clear executor-local and reducer-owned `ResolveWait` intent)
- [ ] Preserve resolve-pending regression coverage. (verification: integration - run focused tests `cargo test test_missing_workspace_retry_clears_resolve_wait_in_reducer`, `cargo test test_stale_workspace_retry_clears_resolve_wait_in_reducer`, and `cargo test test_scheduler_dispatches_synced_manual_resolve_wait_without_queued_work`; completion condition: all focused tests pass without changing expected state transitions)
- [ ] Run formatting and lightweight verification. (verification: manual - run `cargo fmt --check`; inspect `/Users/tumf/work/conflux/AGENTS.md` and `Cargo.toml` for the repository lint/typecheck command and run it if documented; completion condition: commands pass or failures are documented as unrelated)

## Future Work

- None.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate clarify-resolve-wait-clear-outcome --archive-gate`

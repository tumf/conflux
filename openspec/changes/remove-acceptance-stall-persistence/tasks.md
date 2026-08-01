## Implementation Tasks

- [ ] Amend `openspec/CONSTITUTION.md`: remove law 1a (Narrow runtime pause/resume exception) entirely (verification: manual - file diff; verification-id: acceptance-stall-inmemory)
- [ ] Remove `persist_acceptance_stall` call from `src/parallel/dispatch.rs` acceptance stall handler; keep in-memory `ParallelEvent::AcceptanceGated` dispatch intact (verification: unit - cargo test src/parallel/tests/executor.rs; verification-id: acceptance-stall-inmemory)
- [ ] Replace `record_acceptance_stall` in `src/serial_run_service.rs`: remove `AcceptanceStallStore::save()` call, keep in-memory `mark_stalled` and `ChangeProcessResult::AcceptanceStalled` event emission (verification: unit - cargo test serial_run_service; verification-id: acceptance-stall-inmemory)
- [ ] Remove `preflight_acceptance_stall()` disk load in `src/serial_run_service.rs`: on restart, skip disk load; worktree state determines next action (verification: integration - cargo test serial_restart; verification-id: acceptance-stall-inmemory)
- [ ] Remove `reconcile_acceptance_stall()` calls from parallel state reconciliation; keep the function body as dead code with `#[allow(dead_code)]` for one release (verification: unit - cargo test acceptance_state; verification-id: acceptance-stall-inmemory)
- [ ] Remove `load_valid_acceptance_stall()` from `src/execution/state.rs`; delete or `#[allow(dead_code)]` the function (verification: unit - cargo test execution::state; verification-id: acceptance-stall-inmemory)
- [ ] Delete existing stall files under `~/.local/state/cflx/acceptance-stalls/` at Conflux startup, before acceptance dispatch begins (verification: integration - inspect startup log; verification-id: acceptance-stall-inmemory)
- [ ] Update `openspec/specs/parallel-execution/spec.md`: MODIFY `Requirement: Acceptance stalled retry evidence is workspace-local` — replace out-of-worktree persistence language with in-memory-only semantics. MODIFY `Requirement: Acceptance execution creates no JSON checkpoint` — remove reference to out-of-worktree stall record. (verification: unit - cflx openspec validate remove-acceptance-stall-persistence --strict; verification-id: acceptance-stall-inmemory)
- [ ] Verify explicit operator retry still works: `F5` / `retry` on a stalled change consumes the in-memory hold and resumes acceptance without re-applying (verification: manual - TUI retry on stalled change; verification-id: acceptance-stall-inmemory)
- [ ] Verify restart re-runs acceptance: apply a change, let acceptance return stalled, restart Conflux, confirm acceptance re-runs instead of showing stalled (verification: integration - end-to-end restart scenario; verification-id: acceptance-stall-inmemory)

## Future Work

- Remove `AcceptanceStallStore`, `AcceptanceStallRecord`, `reconcile_acceptance_stall()`, and related dead code after one release cycle.

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate remove-acceptance-stall-persistence --archive-gate`

## Implementation Tasks

- [ ] Amend `openspec/CONSTITUTION.md`: remove law 1a entirely; remove `except as permitted by law 1a` reference from law 1; add one sentence to law 1 clarifying that ephemeral in-memory state within a single process lifetime is not "durable" and is permitted (verification: manual - file diff; verification-id: acceptance-stall-inmemory)
- [ ] Remove `persist_acceptance_stall` call from `src/parallel/dispatch.rs` acceptance stall handler; keep in-memory `ParallelEvent::AcceptanceGated` dispatch intact (verification: unit - cargo test src/parallel/tests/executor.rs; verification-id: acceptance-stall-inmemory)
- [ ] Replace `record_acceptance_stall` in `src/serial_run_service.rs`: remove `AcceptanceStallStore::save()` call, keep in-memory `mark_stalled` and `ChangeProcessResult::AcceptanceStalled` event emission (verification: unit - cargo test serial_run_service; verification-id: acceptance-stall-inmemory)
- [ ] Remove `preflight_acceptance_stall()` disk load in `src/serial_run_service.rs`: on restart, skip disk load; worktree state determines next action (verification: integration - cargo test serial_restart; verification-id: acceptance-stall-inmemory)
- [ ] Delete `reconcile_acceptance_stall()` and its callers from parallel state reconciliation (verification: unit - cargo test acceptance_state; verification-id: acceptance-stall-inmemory)
- [ ] Delete `load_valid_acceptance_stall()` from `src/execution/state.rs` (verification: unit - cargo test execution::state; verification-id: acceptance-stall-inmemory)
- [ ] Ignore existing stall files under `~/.local/state/cflx/acceptance-stalls/`: do NOT load them on startup, do NOT delete them (leaving them is harmless for concurrent old-cflx processes sharing the same state dir) (verification: integration - inspect startup log confirms no load; verification-id: acceptance-stall-inmemory)
- [ ] Update `openspec/specs/parallel-execution/spec.md`: MODIFY `Requirement: Acceptance stalled retry evidence is workspace-local` — replace out-of-worktree persistence language with in-memory-only semantics. MODIFY `Requirement: Acceptance execution creates no JSON checkpoint` — remove reference to out-of-worktree stall record. (verification: unit - cflx openspec validate remove-acceptance-stall-persistence --strict; verification-id: acceptance-stall-inmemory)
- [ ] Update `openspec/specs/orchestration-state/spec.md`: MODIFY `Requirement: Stalled blocker metadata` — replace "outside the managed worktree in versioned Conflux runtime state" with "in-memory OrchestratorState only"; remove restart-survival scenario, add restart-clears-stall scenario (verification: unit - cflx openspec validate remove-acceptance-stall-persistence --strict; verification-id: acceptance-stall-inmemory)
- [ ] Verify explicit operator retry still works: `F5` / `retry` on a stalled change consumes the in-memory hold and resumes acceptance without re-applying (verification: manual - TUI retry on stalled change; verification-id: acceptance-stall-inmemory)
- [ ] Verify restart re-runs acceptance: apply a change, let acceptance return stalled, restart Conflux, confirm acceptance re-runs instead of showing stalled (verification: integration - end-to-end restart scenario; verification-id: acceptance-stall-inmemory)

## Future Work

- Remove `AcceptanceStallStore`, `AcceptanceStallRecord`, and `migrate_legacy_acceptance_marker` after one release cycle (dead code from disk persistence removal).

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate remove-acceptance-stall-persistence --archive-gate`

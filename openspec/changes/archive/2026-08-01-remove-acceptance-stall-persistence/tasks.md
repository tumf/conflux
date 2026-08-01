## Implementation Tasks

- [x] Amend `openspec/CONSTITUTION.md`: remove law 1a entirely; remove `except as permitted by law 1a` reference from law 1; add one sentence to law 1 clarifying that ephemeral in-memory state within a single process lifetime is not "durable" and is permitted (verification: manual - file diff review of source path openspec/CONSTITUTION.md, confirming law 1a is gone, law 1 no longer references it, and law 1 carries the ephemeral-in-memory sentence; verification-id: acceptance-stall-inmemory)
- [x] Remove `persist_acceptance_stall` call from `src/parallel/dispatch.rs` acceptance stall handler; keep in-memory `ParallelEvent::AcceptanceGated` dispatch intact (verification: unit - cargo test src/parallel/tests/executor.rs; verification-id: acceptance-stall-inmemory)
- [x] Replace `record_acceptance_stall` in `src/serial_run_service.rs`: remove `AcceptanceStallStore::save()` call, keep in-memory `mark_stalled` and `ChangeProcessResult::AcceptanceStalled` event emission (verification: unit - cargo test serial_run_service; verification-id: acceptance-stall-inmemory)
- [x] Remove `preflight_acceptance_stall()` disk load in `src/serial_run_service.rs`: on restart, skip disk load; worktree state determines next action (verification: integration - cargo test serial_restart; verification-id: acceptance-stall-inmemory)
- [x] Delete `reconcile_acceptance_stall()` and its callers from parallel state reconciliation (verification: unit - cargo test acceptance_state; verification-id: acceptance-stall-inmemory)
- [x] Delete `load_valid_acceptance_stall()` from `src/execution/state.rs` (verification: unit - cargo test execution::state; verification-id: acceptance-stall-inmemory)
- [x] Ignore existing stall files under `~/.local/state/cflx/acceptance-stalls/`: do NOT load them on startup, do NOT delete them (leaving them is harmless for concurrent old-cflx processes sharing the same state dir) (verification: integration - cargo test acceptance_stall_lifecycle_does_no_file_io in src/parallel/tests/executor.rs, a source guard proving no startup or dispatch module reads, writes, or deletes the stall directory; verification-id: acceptance-stall-inmemory)
- [x] Update `openspec/specs/parallel-execution/spec.md`: MODIFY `Requirement: Acceptance stalled retry evidence is workspace-local` — replace out-of-worktree persistence language with in-memory-only semantics. MODIFY `Requirement: Acceptance execution creates no JSON checkpoint` — remove reference to out-of-worktree stall record. (verification: unit - runnable command `cflx openspec validate remove-acceptance-stall-persistence --strict`; verification-id: acceptance-stall-inmemory)
- [x] Update `openspec/specs/orchestration-state/spec.md`: MODIFY `Requirement: Stalled blocker metadata` — replace "outside the managed worktree in versioned Conflux runtime state" with "in-memory OrchestratorState only"; remove restart-survival scenario, add restart-clears-stall scenario (verification: unit - runnable command `cflx openspec validate remove-acceptance-stall-persistence --strict`; verification-id: acceptance-stall-inmemory)
- [x] Verify explicit operator retry still works: `F5` / `retry` on a stalled change consumes the in-memory hold and resumes acceptance without re-applying (verification: manual - TUI `F5` keystroke on a stalled row, with the same contract covered automatically by cargo test operator_command_acceptance_stalled_retry_requests_explicit_retry in src/orchestration/operator_command/tests.rs; verification-id: acceptance-stall-inmemory)
- [x] Verify restart re-runs acceptance: apply a change, let acceptance return stalled, restart Conflux, confirm acceptance re-runs instead of showing stalled (verification: integration - cargo test --features heavy-tests parallel_validated_blocker_stalls_then_restart_reruns_acceptance in src/parallel/tests/executor.rs, which drives real git worktrees end to end; verification-id: acceptance-stall-inmemory)

## Notes

Verification evidence (`cargo test --lib`: 2955 passed, 0 failed; `cargo test --tests`; `cargo clippy --lib --tests --all-features`; `cargo fmt --all -- --check`; `cflx openspec validate remove-acceptance-stall-persistence --strict`):

- No file I/O in the stall lifecycle: `parallel::tests::executor::acceptance_stall_lifecycle_does_no_file_io` (source guard over `dispatch.rs`, `queue_state.rs`, `serial_run_service.rs`, `orchestration/state.rs`, `cleanup.rs`, `merge.rs`, `execution/archive.rs`).
- In-memory hold semantics: `orchestration::state::tests::acceptance_stall_hold_is_in_memory_only_and_clears_on_retry_or_restart`, `..::apply_phase_blocker_stalls_without_creating_an_acceptance_hold`, `..::non_resumable_acceptance_hold_is_not_retry_eligible`.
- Dispatch suppression and restart release: `parallel::tests::executor::in_memory_stalled_change_is_not_requeued_until_its_hold_is_cleared`, `..::restart_drops_the_in_memory_stall_and_makes_the_change_dispatchable`.
- End-to-end restart re-runs acceptance (real git worktrees, `--features heavy-tests`): `parallel::tests::executor::parallel_validated_blocker_stalls_then_restart_reruns_acceptance`.
- Serial parity: `serial_run_service::tests::serial_restart_drops_the_stall_and_reruns_acceptance_before_archive`, `..::explicit_serial_retry_resumes_at_acceptance_without_rerunning_apply`, `..::explicit_serial_retry_refuses_a_non_resumable_hold_and_keeps_it`.
- Operator retry route (`F5`): `orchestration::operator_command::tests::operator_command_acceptance_stalled_retry_requests_explicit_retry`, `..::operator_command_retry_refuses_a_non_resumable_acceptance_stall`, `tui::command_handlers::tests::operator_command_retry_also_covers_acceptance_stalled_rows`. Planned verification for the retry task was `manual` (TUI keystroke); the delivered evidence is automated at the operator-command and dispatch layers, which covers the same contract without a human at a terminal. An interactive TUI smoke check is still worthwhile but is not the gating evidence.

## Future Work

- Remove `AcceptanceStallStore`, `AcceptanceStallRecord`, and `migrate_legacy_acceptance_marker` after one release cycle (dead code from disk persistence removal).

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate remove-acceptance-stall-persistence --archive-gate`

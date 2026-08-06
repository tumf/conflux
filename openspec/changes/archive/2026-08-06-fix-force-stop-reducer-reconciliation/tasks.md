## Implementation Tasks

- [x] Implement one reducer-owned process-stop reconciliation in `src/orchestration/state.rs`. Completion requires `ExecutionEvent::Stopped` to reset every non-terminal row carrying activity, queue intent, or wait/hold state to idle `NotQueued` with no wait/blocker/commit or scheduler-owned retry membership, establish same-process reactivation suppression until explicit requeue, preserve fresh idle rows and all existing terminal outcomes, and never create per-change `TerminalState::Stopped`. Add the `global_stopped_reconciles_interrupted_runtime` table-driven regression covering all activity/wait families, queued intent, terminal/fresh-idle exclusions, duplicate stop, late lifecycle delivery, `ChangesRefreshed.merge_wait_ids` suppression before explicit requeue, startup re-derivation without a durable guard, and explicit requeue. (verification: unit - `cargo test --lib global_stopped_reconciles_interrupted_runtime -- --list | grep -q global_stopped_reconciles_interrupted_runtime && cargo test --lib global_stopped_reconciles_interrupted_runtime`; verification-id: stopped-reconciliation-regressions)

- [x] Route process-level `Stopped` through reducer-derived TUI display synchronization and remove independent TUI row lifecycle ownership from the local stopped handler while preserving mode, timing, controls, elapsed values, and exactly-once `Processing stopped` logging. Add `stopped_reducer_sync_prevents_accepting_resurrection`, traversing authoritative `AcceptanceStarted` and `Stopped` dispatch, reducer-cache synchronization, local event handling, and a later `ChangesRefreshed`; completion requires `not queued` to survive the full order and the execution mark to remain set. (verification: integration - `cargo test --lib stopped_reducer_sync_prevents_accepting_resurrection -- --list | grep -q stopped_reducer_sync_prevents_accepting_resurrection && cargo test --lib stopped_reducer_sync_prevents_accepting_resurrection`; verification-id: stopped-reconciliation-regressions)

- [x] Verify `/api/v2` consumes the same stopped dispatch state without frontend repair logic. Add `stopped_projection_reconciles_change_status` covering an accepting row becoming `display_status: not queued` with `queue_intent: not_queued`, unchanged execution mark, one state revision for the first stop, and no additional revision for duplicate `Stopped`; completion requires the test to use the authoritative event-dispatch boundary rather than mutating the projected snapshot directly. (verification: integration - `cargo test --lib stopped_projection_reconciles_change_status -- --list | grep -q stopped_projection_reconciles_change_status && cargo test --lib stopped_projection_reconciles_change_status`; verification-id: stopped-reconciliation-regressions)

## Implementation Notes

- Reducer transition: `OrchestratorState::on_run_stopped` in `src/orchestration/state.rs`, reached from the new `ExecutionEvent::Stopped` arm of `apply_execution_event`. It reuses the existing `transition_change_to_dequeued` shape, so the dequeue guard, blocker/commit clearing, and resolve/reject queue removal are the same code an explicit per-change dequeue uses, and additionally drops stall/skip membership and a matching `current_change_id`.
- The guard is the existing process-local `ChangeRuntimeState::dequeued` flag. Every reactivation input already tests it (`on_acceptance_started`, `on_apply_started`, `on_workspace_preparation_started`, and `apply_observation` through `is_fresh_idle`), and `ReducerCommand::AddToQueue` is what releases it. Nothing is persisted, so a restarted process re-derives `merge wait` from the same workspace evidence.
- TUI: `should_apply_event_to_tui_reducer` now classifies `Stopped` as display-affecting, and `AppState::handle_stopped` no longer rewrites row statuses. It still owns mode, stop mode, controls, orchestration/row elapsed values, and the exactly-once `Processing stopped` log. Elapsed timing is keyed on `started_at`/`elapsed_time` rather than the status cache, because the reducer has already moved those rows by the time the handler runs.
- `TuiCommand::ForceStop` with no live scheduler to reach a safe boundary is its own dispatch owner, so it now applies `ExecutionEvent::Stopped` to the shared reducer and re-reads the display statuses before local handling. The awaiting-safe-boundary path is unchanged; the scheduler's own `Stopped` already goes through `dispatch_event`.
- `is_reducer_owned_refresh_merge_wait_protected_status` also protects `not queued`. The reducer consumes the same `ChangesRefreshed` before the TUI does, so a `not queued` reducer answer for an ID in `merge_wait_ids` is an explicit refusal (stop guard or explicit dequeue) rather than missing information. A restarted process has no guard and its reducer answers `merge wait` here instead.
- Tests that asserted the removed TUI-local row reset were rewritten to assert what the handler still owns: `handle_stopped_does_not_own_row_lifecycle`, `stopped_freezes_elapsed_time_for_runs_that_were_still_timing`, `stopped_enters_stopped_mode_and_preserves_execution_marks`, and the modal regression in `modal_tests.rs`.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-force-stop-reducer-reconciliation --archive-gate`.

Verification results for `stopped-reconciliation-regressions`:

- `cargo test --lib global_stopped_reconciles_interrupted_runtime` — 1 passed
- `cargo test --lib stopped_reducer_sync_prevents_accepting_resurrection` — 1 passed
- `cargo test --lib stopped_projection_reconciles_change_status` — 1 passed
- `cargo test --lib` — 3334 passed, 0 failed, 14 ignored
- `cargo fmt --check` — clean
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — clean

## Future Work

- Consider removing unused per-change terminal stopped vocabulary only in a separate compatibility change after all producer and API references are audited.

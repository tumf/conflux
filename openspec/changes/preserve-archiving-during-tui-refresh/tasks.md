## Implementation Tasks

- [ ] Add a production-order regression test for `ArchiveStarted(alpha)` followed by reducer cache synchronization and `ChangesRefreshed` containing `alpha` in `merge_wait_ids`. Completion requires the test to assert that reducer and TUI remain `archiving`, and that queue intent and execution marks are unchanged by presentation refresh. (verification: integration - add focused coverage near `src/tui/state/event_handlers/refresh.rs` or the existing TUI adapter harness; run `cargo test --lib archive_refresh`; verification-id: tui-archive-refresh-tests)

- [ ] Replace the TUI refresh helper's partial active-status list with the existing shared `src/orchestration/operator_command.rs::is_active_status` classifier while preserving the current non-active protections for pending, terminal/error, and explicit stop/dequeue states. Completion requires no new lifecycle vocabulary or duplicate active-status table and makes the event-order regression pass. (verification: unit - exercise refresh protection through public state behavior in `src/tui/state/event_handlers/refresh.rs`; run `cargo test --lib merge_wait_refresh`; verification-id: tui-archive-refresh-tests)

- [ ] Add table-driven regression coverage for every status accepted by the shared active-status classifier and compatibility coverage for stale display-only correction, fresh-process archived-workspace restoration, and concrete manual `MergeDeferred(auto_resumable=false)`. Completion requires active statuses never to become `merge wait`, while the existing legitimate merge-wait paths remain observable. (verification: unit - extend focused refresh and reducer/TUI synchronization tests; run `cargo test --lib merge_wait_refresh`; verification-id: tui-archive-refresh-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate preserve-archiving-during-tui-refresh --archive-gate`.

## Future Work

- A typed presentation-precedence API may be proposed separately if more refresh hints are added; this bug does not justify a broader lifecycle refactor.

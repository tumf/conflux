## Implementation Tasks

- [x] Add `archive_refresh_preserves_reducer_owned_archiving_row` beside `stopped_reducer_sync_prevents_accepting_resurrection` in `src/tui/runner.rs`. Dispatch `ArchiveStarted` through `crate::events::dispatch_event`, synchronize reducer display caches, then dispatch, synchronize, and handle `ChangesRefreshed` with the change in `merge_wait_ids`. Completion requires asserting that the TUI row and reducer status both remain `archiving`, and that queue intent and the execution mark are unchanged. (verification: integration - run `cargo test --lib archive_refresh_preserves_reducer_owned_archiving_row`; verification-id: tui-archive-refresh-tests)

- [x] Replace the TUI refresh helper's partial active-status list in `src/tui/state/event_handlers/refresh.rs` with the existing shared `src/orchestration/operator_command.rs::is_active_status` classifier while preserving the current non-active protections for pending, terminal/error, and explicit stop/dequeue states. Completion requires the existing `merge_wait_refresh_*` tests and `archive_refresh_preserves_reducer_owned_archiving_row` to pass without introducing another lifecycle vocabulary. (verification: unit - run `cargo test --lib merge_wait_refresh`; verification-id: tui-archive-refresh-tests)

- [x] Expose the shared active-status vocabulary next to `is_active_status` in `src/orchestration/operator_command.rs`, and implement `is_active_status` over that single backing vocabulary. Add `merge_wait_refresh_protects_every_shared_active_status` by iterating the exposed vocabulary, plus `merge_wait_refresh_restores_fresh_process_archived_workspace` and `merge_wait_refresh_preserves_concrete_manual_merge_deferral`. Completion requires active statuses never to become `merge wait`, while stale display-only correction, startup restoration, and concrete manual deferral remain observable. (verification: unit - run `cargo test --lib merge_wait_refresh`; verification-id: tui-archive-refresh-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate preserve-archiving-during-tui-refresh --archive-gate`.

## Notes

- evidence: `cargo test --lib archive_refresh_preserves_reducer_owned_archiving_row` — 1 passed, 0 failed.
- evidence: `cargo test --lib merge_wait_refresh` — 9 passed, 0 failed (6 pre-existing plus `merge_wait_refresh_protects_every_shared_active_status`, `merge_wait_refresh_restores_fresh_process_archived_workspace`, `merge_wait_refresh_preserves_concrete_manual_merge_deferral`).
- evidence: regression coverage confirmed by mutation — with the shared classifier removed from `is_reducer_owned_refresh_merge_wait_protected_status`, `merge_wait_refresh_protects_every_shared_active_status` fails with `left: "merge wait", right: "preparing"`; the fix was restored before final verification.
- evidence: `cargo test --lib` — 3654 passed, 0 failed, 17 ignored.
- evidence: `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check` both clean.
- The `tui-archive-refresh-tests` verification is satisfied by the two declared rerun commands above.

## Future Work

- A typed presentation-precedence API may be proposed separately if more refresh hints are added; this bug does not justify a broader lifecycle refactor.

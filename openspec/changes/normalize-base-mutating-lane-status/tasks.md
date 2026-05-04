## Implementation Tasks

- [x] Model base-mutating lane ownership in the reducer. (verification: unit - add/update `src/orchestration/state.rs` tests proving at most one non-terminal `Resolving`/`Rejecting` activity can be active and that `has_other_post_archive_lane_blocker()` treats both as lane occupants; completion condition: reducer display and invariants fail if `Resolving` and `Rejecting` can coexist)

- [x] Add reducer-owned `reject pending` state and queue APIs. (verification: unit - add tests in `src/orchestration/state.rs` for a new `RejectWait`/equivalent wait state, `display_status() == "reject pending"`, `reject_wait_change_ids()` membership, and clear-on-start/completion semantics; completion condition: reject-review intent cannot be represented as `resolve pending`, `queued`, or `merge wait`)

- [x] Route rejection-review handoff through the base-mutating lane scheduler. (verification: integration - add/update `src/parallel/dispatch.rs` and `src/parallel/queue_state.rs` tests where Change B generates `REJECTED.md` while Change A is `Resolving` or `Rejecting`; completion condition: B displays `reject pending`, does not start rejection review until A releases the lane, then transitions to `rejecting`)

- [x] Preserve archive-merge lane deferral as `resolve pending`. (verification: unit - keep/update tests in `src/tui/orchestrator.rs`, `src/parallel/merge.rs`, or `src/parallel/tests/executor.rs` showing an archived change becomes `resolve pending` when another change is `Resolving` or `Rejecting`; completion condition: archive-merge work is never routed to `reject pending` and active `Applying`/`Accepting`/`Archiving` alone does not create `resolve pending`)

- [x] Make lane-clear promotion deterministic and single-occupant. (verification: integration - add scheduler tests in `src/parallel/queue_state.rs` or `src/parallel/tests/executor.rs` for lane release after `MergeCompleted`, `ResolveCompleted`, `ResolveFailed`, `RejectionReviewCompleted`, and `RejectionReviewFailed`; completion condition: at most one pending base-mutating operation is promoted, pending queues are updated, and the next operation type matches its pending status)

- [x] Synchronize TUI display from reducer for all base-lane lifecycle events. (verification: unit - add/update `src/tui/runner.rs`, `src/tui/state.rs`, and `src/tui/state/event_handlers/*.rs` tests proving `ChangeArchived`, `MergeCompleted`, `ResolveStarted`, `ResolveCompleted`, `MergeDeferred`, `WorkspaceStatusUpdated(Rejecting)`, `RejectionReviewCompleted`, and `RejectionReviewFailed` cannot leave stale `archived`, `queued`, or `not queued` over reducer-derived statuses; completion condition: TUI rows show `resolving`, `rejecting`, `resolve pending`, `reject pending`, `merge wait`, and `merged` according to reducer output)

- [x] Synchronize Web/server snapshots for `reject pending` and post-archive lane states. (verification: unit - add/update `src/web/state.rs` tests proving snapshots/API payloads expose reducer-derived `reject pending` and do not regress `resolving`/`merged` after post-archive events; completion condition: Web display cannot show stale archive milestone as stable state in parallel mode)

- [x] Run targeted Rust verification for reducer, parallel scheduler, TUI, and Web state behavior. (verification: integration - run targeted `cargo test` filters covering the new reducer, parallel dispatch/queue, TUI display, and Web state tests; completion condition: commands exit 0 and any test taking over 1 second is optimized or marked heavy per AGENTS.md)

## Future Work

- If real-world dogfooding still reveals status drift outside reducer/TUI/Web/scheduler paths, create a focused follow-up proposal with logs from `~/.local/state/cflx/logs/conflux-{slug}/YYYY-MM-DD.log` and the exact event sequence.

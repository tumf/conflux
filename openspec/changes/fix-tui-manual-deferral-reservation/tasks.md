## Implementation Tasks

- [ ] Update TUI manual-deferral handling so `MergeDeferred(auto_resumable=false)` received after an optimistic `M` reservation clears that reservation, leaves the reducer-derived row at `merge wait`, and does not enqueue or return another resolve command. Completion condition: `src/tui/state/event_handlers/errors.rs` distinguishes the selected change's pre-`ResolveStarted` reservation from an actually resolving row and no local branch overwrites manual deferral with `resolve pending`. (verification: unit - add a focused test in `src/tui/state/event_handlers/errors.rs` that starts from `is_resolving=true` plus `resolve pending`, applies the reducer-first manual-deferral display snapshot, invokes the handler, and asserts `merge wait`, `is_resolving=false`, and no returned `TuiCommand::ResolveMerge`.)

- [ ] Add minimal targeted cleanup for a manually deferred change in the TUI-local resolve queue and set without disturbing unrelated FIFO entries. Completion condition: the deferred ID is removed from both `resolve_queue` and `resolve_queue_set`, unrelated queued IDs retain order and membership, and no new lifecycle abstraction or durable state is introduced. (verification: unit - add queue tests in `src/tui/state.rs` covering removal from the front/middle and absence/no-op behavior; run `cargo test resolve_queue --lib`.)

- [ ] Preserve actual concurrent resolve and auto-resumable behavior while applying manual cleanup. Completion condition: `MergeDeferred(auto_resumable=true)` for another change behind an active resolve remains `resolve pending` and queued, while `MergeDeferred(auto_resumable=false)` for one change does not clear `is_resolving` when a different row is actually `resolving`. (verification: unit - extend `src/tui/state/event_handlers/errors.rs` tests with distinct current/deferred change IDs and assertions on display, queue membership, and serialization state.)

- [ ] Prove retry actionability across the complete local lifecycle. Completion condition: a test models `M` reservation, reducer demotion through `MergeDeferred(false)`, TUI event handling, blocker cleanup as an unchanged repository precondition, and a second `resolve_merge()` call that returns a fresh `TuiCommand::ResolveMerge` without reconstructing `AppState`. (verification: integration - add a focused TUI state/event sequence test in `src/tui/state/event_handlers/errors.rs` or `src/tui/state.rs` using a shared `OrchestratorState`, assert reducer `ResolveWait` membership after the second `M` and absence of stale local queue state from the first attempt, and run `cargo test manual_deferral --lib`.)

- [ ] Run Rust quality gates and keep the default test tier fast. Completion condition: formatting, lint, focused regression tests, and the complete default suite pass; every added default-path test remains under one second. (verification: integration - run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, the new focused test filters, and `cargo test`.)

## Future Work

Removing the legacy TUI-local resolve queue in favor of exclusively reducer-derived serialization is a broader refactor and remains outside this bug fix.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected authoring check: `cflx openspec validate fix-tui-manual-deferral-reservation --strict --evidence warn`
Expected archive gate: `cflx openspec validate fix-tui-manual-deferral-reservation --archive-gate`

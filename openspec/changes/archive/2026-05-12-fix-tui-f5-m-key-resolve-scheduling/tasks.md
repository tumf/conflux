## Implementation Tasks

- [x] Remove cursor-local `MergeWait` handling from `F5` in `src/tui/key_handlers.rs`. Completion condition: `handle_f5_key()` no longer checks the current cursor row for `merge wait` and cannot return `ctx.app.resolve_merge()` from an F5 path. (verification: unit - add/update tests in `src/tui/key_handlers.rs` or `src/tui/state.rs`; focused command: `cargo test f5` must include a case proving F5 on a `merge wait` row does not emit `TuiCommand::ResolveMerge`.)

- [x] Remove the F5-level unrelated-resolve block from `src/tui/key_handlers.rs`. Completion condition: `F5` delegates to `start_processing()`, `resume_processing()`, or `retry_error_changes()` even when `app.is_resolving == true`, preserving resolve serialization only for M/merge operations. (verification: unit - add/update tests in `src/tui/key_handlers.rs` or `src/tui/state.rs` covering Select, Stopped, and Error modes with `is_resolving == true` and runnable work; focused command: `cargo test resolving`.)

- [x] Preserve and tighten Changes-view `M` behavior in `src/tui/key_handlers.rs`, `src/tui/state.rs`, and `src/tui/command_handlers.rs`. Completion condition: Changes-view `M` on `MergeWait` registers reducer-owned `ResolveMerge` intent, non-`MergeWait` rows do nothing, and no TUI code directly executes merge/resolve outside scheduler-owned command handling. (verification: unit - extend `cargo test resolve_merge` coverage for accepted intent, rejected/no-op intent, and non-`MergeWait` rows.)

- [x] Encode resolve/base-mutating occupancy-before-dirty classification in scheduler merge retry code. Completion condition: merge retry deferral classification first checks active resolve/base-mutating lane occupancy and only then evaluates base/workspace dirty state when no active lane exists. (verification: unit - add/adjust tests in `src/parallel/tests/executor.rs` or `src/parallel/merge.rs` proving dirty state during active resolve yields `auto_resumable=true` / `resolve pending`, while dirty state with no active resolve yields `auto_resumable=false` / `merge wait`.)

- [x] Ensure a running scheduler promotes exactly one clean `ResolveWait` retry when no resolve/base-mutating operation is active. Completion condition: scheduler-owned `ResolveWait` changes are consumed by the normal scheduler loop; one eligible item emits `ResolveStarted`/retry dispatch and other pending items remain pending. (verification: integration - add/update scheduler tests around `maybe_dispatch_resolve_wait_retry()` / `retry_deferred_base_lane_waiters()` and run focused `cargo test resolve_wait`.)

- [x] Update TUI key hints and stale spec wording around M. Completion condition: Changes view shows `M: resolve` / `M: queue resolve` only for `merge wait` rows, and specs/tests no longer claim M immediately displays `resolving` before scheduler start events. (verification: unit - update render tests in `src/tui/render.rs` and mode/state tests in `src/tui/state.rs` for `resolve pending` before scheduler `ResolveStarted`; focused command: `cargo test render --lib` or `cargo test resolve_merge --lib`.)

- [x] Add regression tests that cover the original F5 regression and M/F5 responsibility split. Completion condition: test coverage fails if F5 becomes cursor-local again or if F5 emits `ResolveMerge` for a `merge wait` row. (verification: unit - focused TUI tests runnable with `cargo test f5 resolve_merge` or repository-equivalent filters.)

- [x] Run formatting, lint, and relevant tests for touched Rust code. Completion condition: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings` when practical, and focused `cargo test` filters for TUI/parallel resolve paths pass or failures are documented with repository evidence. (verification: integration - record command outputs for `cargo fmt --check`, `cargo test resolve_merge --lib`, `cargo test resolve_wait --lib`, and any updated focused test filters in the implementation summary.)

## Future Work

- Manual TUI smoke test in a real parallel run: place one change in `merge wait`, press `M` while another resolve is active, confirm the row stays `resolve pending`; then after the active resolve clears and the workspace is clean, confirm exactly one pending retry starts.
- Manual TUI smoke test for the F5 regression: place cursor on `merge wait`, mark a separate runnable change, press `F5`, and confirm normal orchestration starts without resolving the cursor row.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-tui-f5-m-key-resolve-scheduling --archive-gate`

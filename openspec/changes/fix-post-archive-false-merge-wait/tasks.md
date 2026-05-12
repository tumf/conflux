## Implementation Tasks

- [x] Trace the post-archive event path before applying behavior changes. Completion condition: repository notes, test names, or implementation comments identify the observed ordering across `ChangeArchived`, reducer sync, periodic `ChangesRefreshed.merge_wait_ids`, `ResolveStarted`, `MergeDeferred`, `WorkspaceStatusUpdated`, and `MergeCompleted`, and the code change is targeted at the confirmed failing boundary. (verification: not-testable - this is root-cause evidence gathering required before implementation; reviewer verifies the subsequent tests target the documented event path)

- [x] Fix reducer reconciliation so archive-complete refresh evidence cannot regress an active post-archive merge/resolve lifecycle. Completion condition: `src/orchestration/state.rs` preserves `ActivityState::Resolving` and reducer-owned wait/terminal states when `ChangesRefreshed.merge_wait_ids` or `WorkspaceObservation::WorkspaceArchived` arrives without concrete manual deferral evidence. (verification: unit - add/update `cargo test` coverage in `src/orchestration/state.rs` for `ChangeArchived` no-blocker -> `resolving`, refresh with matching `merge_wait_ids` -> still `resolving`, active lane -> `resolve pending`, and `MergeDeferred(auto_resumable=false)` -> `merge wait`)

- [x] Fix TUI refresh display precedence for post-archive states. Completion condition: `src/tui/state/event_handlers/refresh.rs` and any required state snapshot plumbing prevent refresh-derived `merge_wait_ids` from overwriting reducer-owned `resolving`, `resolve pending`, `rejecting`, `reject pending`, `merged`, `rejected`, or `error`, while still correcting stale display-only rows when the reducer does not own active/pending/terminal status. (verification: unit - add/update tests around `handle_changes_refreshed()` and `apply_display_statuses_from_reducer()` that fail if `resolving` is temporarily changed to `merge wait` by refresh evidence, and keep existing stale display-only correction tests passing)

- [x] Separate auto-resumable deferral from manual merge-wait evidence in the parallel merge path. Completion condition: `src/parallel/merge.rs` and `src/parallel/queue_state.rs` no longer expose `auto_resumable=true` deferrals as manual `MergeWait` status in ways that can drive false `merge wait` display, while `auto_resumable=false` still produces manual `merge wait`. (verification: unit - add/update parallel merge/queue tests proving `MergeDeferred(auto_resumable=true)` remains `resolve pending` / retryable and `MergeDeferred(auto_resumable=false)` becomes `merge wait`)

- [x] Verify TUI/Web status parity after the corrected reducer state is applied. Completion condition: TUI and Web status derivation both report reducer-owned `resolving`, `resolve pending`, and `merge wait` consistently for the same reducer state, and no UI-local cache becomes workflow-control input. (verification: unit - add/update tests in `src/tui/state.rs` or `src/web/state.rs` for reducer-derived status mapping; manual - if Web-specific automated coverage is impractical, perform a documented local state/API check showing `/api/state` or Web snapshot matches reducer status)

- [x] Preserve related manual resolve lifecycle behavior. Completion condition: existing guarantees from `fix-manual-resolve-refresh-regression` and `fix-merge-completed-resolve-flag` remain covered: reducer-owned `resolve pending` survives refresh evidence, stale display-only pending can still return to `merge wait`, and `MergeCompleted` can close a manual resolve lifecycle. (verification: unit - run targeted tests such as `cargo test merge_wait_refresh`, `cargo test resolve_merge`, and `cargo test merge_completed`, adjusting names to the final test filters present in the repo)

- [x] Run final Rust verification for the affected areas. Completion condition: formatting and tests relevant to the modified modules pass without introducing default-suite heavy tests over 1 second unless marked `heavy`. (verification: integration - run `cargo fmt --check`, targeted `cargo test` filters for orchestration/TUI/parallel merge state, and the repository lint/typecheck command if present in project scripts)

## Future Work

- Manual long-running TUI smoke test in a real parallel run: archive a change while no manual dirty blocker exists and confirm the row does not visibly flicker to `merge wait` before `resolving` / `merged`.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-post-archive-false-merge-wait --archive-gate`

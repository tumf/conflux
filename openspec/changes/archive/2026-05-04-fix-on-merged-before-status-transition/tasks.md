## Implementation Tasks

- [x] Make `on_merged` failure block merged transition in all parallel success paths. (verification: unit - implemented guards in `src/parallel/merge.rs`, `src/parallel/queue_state.rs`, and manual TUI merge path to return/continue before `MergeCompleted`/`BranchMergeCompleted` after non-continuable `run_hook(HookType::OnMerged, ...)` failure; evidence: `cargo check --lib` passes and targeted hook/TUI tests pass; completion condition: merge success paths cannot fall through from hook failure to `MergeCompleted` when `continue_on_failure=false`)

- [x] Preserve truthful reducer and UI state after `on_merged` failure. (verification: integration - added TUI event-handler coverage in `src/tui/state/event_handlers/errors.rs`; parallel paths now emit `ResolveFailed`/`HookFailed` and manual branch merges emit `BranchMergeFailed`/`HookFailed` instead of merged events; evidence: `cargo test on_merged_hook_failed --lib` passes; completion condition: a later refresh or stale event cannot falsely promote the row to `merged`)

- [x] Strengthen `on_merged` root-repo write-safety checks and diagnostics. (verification: unit - added tests in `src/hooks.rs` for `.git/index.lock` wait logging, timeout behavior, and repo-mutating preflight diagnostics around hook execution; evidence: `cargo test test_on_merged --lib` passes; completion condition: logs distinguish lock already present, lock released, timeout, and hook execution failure using repository-verifiable output)

- [x] Cover the concrete lock-contention regression from `make bump-patch`. (verification: integration - added deterministic hook-runner simulation in `src/hooks.rs` for pre-existing root `.git/index.lock` timeout plus non-continuable `on_merged` failure diagnostics; evidence: `cargo test test_on_merged --lib` passes; completion condition: lock-contention failure is observable and propagates as hook failure before any merged transition event can be emitted)

- [x] Verify the fix against the logged failure path and current hook contract. (verification: integration - ran `cargo test test_on_merged --lib`, `cargo test on_merged_hook_failed --lib`, `cargo check --lib`, and `cflx openspec validate fix-on-merged-before-status-transition --strict --evidence warn`; completion condition: targeted commands exit 0 and the proposal validates strictly)

## Future Work

- If lock contention still occurs after gating fixes, create a follow-up proposal focused on deeper root-repo Git lock ownership tracing across merge cleanup, release commands, and worktree teardown.

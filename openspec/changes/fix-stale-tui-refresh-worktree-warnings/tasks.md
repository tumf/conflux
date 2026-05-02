## Implementation Tasks

- [ ] Detect stale auto-refresh roots before snapshot commands. Completion condition: the TUI auto-refresh loop checks that the refresh root still exists and is usable as a git worktree/repository before calling `list_changes_in_head`, `list_changes_with_uncommitted_files`, or `list_worktree_change_ids`. (verification: unit - add or update a `src/tui/runner.rs`-adjacent test/helper test that passes a missing temp path and proves the snapshot command group is skipped)

- [ ] Bound stale-root warning volume. Completion condition: a missing or invalid refresh root produces at most one warning per affected TUI session, or uses an explicit backoff/rate limiter that prevents one warning per refresh tick. (verification: unit - add `src/tui/runner.rs` helper tests or `src/tui/tests` coverage that simulates multiple refresh ticks for the same missing root and asserts only one stale-root warning/log event or one warning within the rate-limit window)

- [ ] Preserve actionable warnings for existing roots. Completion condition: when the refresh root exists but git snapshot commands fail for a real command reason, the TUI still logs `Failed to refresh ... snapshot` or an equivalent actionable warning with command/root context. (verification: unit/integration - add `src/tui/runner.rs` or `src/vcs` test coverage using an existing temp directory with a controlled failing git/VCS command path and assert the failure remains visible)

- [ ] Keep remote mode unchanged. Completion condition: WebSocket/remote mode continues to return before local auto-refresh starts and does not run local stale-root checks. (verification: unit - cover the `is_remote_mode` path in `src/tui/runner.rs` tests or update existing remote-mode runner tests to assert local refresh setup is bypassed)

- [ ] Validate formatting and targeted regression coverage. Completion condition: formatting passes and targeted tests for the new stale-root behavior pass; any test taking over 1 second is optimized or marked heavy. (verification: integration - `cargo fmt --check`; targeted `cargo test` for the added TUI refresh tests)

## Future Work

- If operators want automatic TUI recovery after external worktree deletion, create a separate proposal for explicit reload/rebind behavior rather than adding hidden durable refresh state.

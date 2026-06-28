## Implementation Tasks

- [x] Add `--push [remote]` parsing for bare TUI launch and explicit `cflx tui` in `src/cli.rs`, reusing the existing `run --push` remote validation and defaulting rules. (verification: unit - CLI parser tests in `src/cli.rs` cover `cflx --push`, `cflx --push upstream`, `cflx tui --push`, `cflx tui --push upstream`, and colon-containing rejection; completion condition: parsed args expose the expected remote values and invalid branch-selection syntax fails before runtime)
- [x] Convert TUI push input to post-archive action in `src/main.rs` and reject `--push` with TUI `--server` before launching local or remote TUI. (verification: integration - add or update CLI-level tests in `tests/run_exit_tests.rs` or the nearest CLI test module for rejected `cflx --push --server <url>` and `cflx tui --push --server <url>` paths; completion condition: the error is emitted before TUI initialization or remote run control is reachable)
- [x] Thread the post-archive action through `src/tui/runner.rs` and `src/tui/command_handlers.rs` into the spawned local parallel orchestrator task without changing serial orchestrator behavior. (verification: unit - add or update tests in `src/tui/command_handlers.rs` that exercise context construction and parallel start with a non-default action while serial start does not require or apply parallel push wiring; completion condition: repository code shows the action is part of the local TUI command context and only consumed for parallel execution)
- [x] Apply the action in `src/tui/orchestrator.rs` by calling `ParallelRunService::set_post_archive_action` before `run_parallel_with_channel_and_queue_state`. (verification: unit - add a focused test or test-only seam in `src/tui/orchestrator.rs` proving `run_orchestrator_parallel` configures `PostArchiveAction::PushToRemote { remote: "origin" }` when started with TUI push mode; completion condition: the check fails if `ParallelRunService` remains at `MergeToBase`)
- [x] Preserve existing `cflx run --push` behavior while sharing validation helpers where practical. (verification: unit - existing and/or expanded `RunArgs` parser tests in `src/cli.rs` continue to pass for `run --parallel --push --all`, remote override, and colon rejection; completion condition: no run-mode parser behavior changes except shared implementation details)
- [x] Run targeted verification for CLI/TUI push-mode parsing and wiring. (verification: integration - run `cargo test cli::` plus the added focused `src/tui/command_handlers.rs` or `src/tui/orchestrator.rs` tests; completion condition: all focused parser and TUI wiring tests pass locally)

## Future Work

- Add remote-server push-mode support only after the remote control API and `server::runner::ProjectRunRequest` can carry post-archive action explicitly.

## Final Validation

Expected proposal validation: `cflx openspec validate add-tui-push-mode --strict --evidence warn`
Expected archive gate: `cflx openspec validate add-tui-push-mode --archive-gate`

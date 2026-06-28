## Implementation Tasks

- [x] Add CLI parsing for `cflx run --push [remote]`. Completion condition: `RunArgs` stores an optional push remote, `--push` defaults to `origin`, `--push upstream` stores `upstream`, and any value containing `:` fails before orchestration starts with a branch-selection-not-supported error. (verification: unit - `cargo test cli_push` or equivalent parser tests in `src/cli.rs`)

- [x] Propagate post-archive action from CLI into run orchestration. Completion condition: `src/main.rs`, `src/orchestrator.rs`, `src/parallel_run_service.rs`, and `src/parallel/mod.rs` carry a `MergeToBase`/`PushToRemote` action without changing default merge behavior when `--push` is absent. (verification: unit - add constructor/service assertions in the relevant Rust module tests and run `cargo test post_archive_action` or the final test filter name used by the implementation)

- [ ] Implement Git push primitive for completed worktree branches. Completion condition: helpers under `src/vcs/git/commands/` can run `git push <remote> <branch>:<branch>` with contextual error reporting and without accepting a destination branch override. (verification: integration - add a local bare-repository test under `src/vcs/git/` or `tests/` and run `cargo test push_post_archive` or the final test filter name used by the implementation)

- [ ] Split post-archive terminal action between merge and push. Completion condition: `src/parallel/merge.rs` and `src/parallel/queue_state.rs` keep existing base merge logic for merge mode, while push mode skips checkout/merge/conflict resolution and invokes the Git push primitive for the workspace branch after archive verification. (verification: integration - add a local bare-remote test and run `cargo test push_post_archive`; the test must assert base HEAD is unchanged and the remote branch receives the archived change commit)

- [x] Add pushed-specific execution events and reducer state. Completion condition: `src/events.rs` defines pushed-specific started/completed/failed events, `src/orchestration/state.rs` can set `TerminalState::Pushed`, display status is `pushed`, and merge-completed state is not used for push success. (verification: unit - add reducer/display tests in `src/orchestration/state.rs` and run `cargo test pushed_terminal` or the final test filter name used by the implementation)

- [ ] Wire CLI, TUI, and Web status reporting for pushed outcomes. Completion condition: `src/orchestrator.rs` logs successful pushes distinctly from merges, TUI state/rendering can show `pushed`, `src/web/state.rs` can represent pushed terminal state, and existing merged display behavior remains unchanged. (verification: unit - add/update tests in `src/tui/`, `src/web/state.rs`, and CLI event-handler coverage, then run `cargo test pushed_status` or the final test filter name used by the implementation)

- [ ] Preserve workspace on push failure and clean it up on push success. Completion condition: successful push follows the existing safe cleanup path in `src/parallel/merge.rs`/`src/parallel/queue_state.rs`; failed push reports a visible error and leaves the worktree/branch available for inspection or retry. (verification: integration - add failing-remote and successful-local-bare-remote tests and run `cargo test push_post_archive`; tests must assert failure keeps the worktree and success removes it)

- [ ] Ensure `on_merged` hooks are not executed in push mode. Completion condition: push success does not call `HookType::OnMerged`, while merge mode still does through the existing path in `src/parallel/merge.rs`. (verification: unit/integration - add a hook counter or fixture test in `src/parallel/tests/` or `src/hooks.rs` and run `cargo test on_merged_push_mode` or the final test filter name used by the implementation)

- [ ] Protect existing merge behavior with regressions. Completion condition: representative merge-mode tests still pass with `PostArchiveAction::MergeToBase`, and no `--push` path changes base-merge default behavior. (verification: unit/integration - run targeted merge tests such as `cargo test merge_completed` and relevant parallel merge tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected proposal validation before implementation: `cflx openspec validate add-push-post-archive-mode --strict --evidence warn`.
Expected archive gate before archive: `cflx openspec validate add-push-post-archive-mode --archive-gate`.

## Future Work

- Add `on_pushed` hook semantics if users need post-push automation.
- Add remote branch collision policy or protected-branch checks if same-name push proves too permissive in production workflows.
- Add provider-specific PR creation in a separate proposal if remote push should open review requests automatically.

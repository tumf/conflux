## Implementation Tasks

- [ ] Add `merge_branch_aborting_on_conflict`, `AbortingMergeOutcome`, `AbortReport`, and `Restoration` to `src/vcs/git/commands/merge.rs`, verifying restoration from `HEAD`, `MERGE_HEAD`, and the read-only clean check, and turn `merge_branch` into a wrapper whose error text names the verified outcome. Completion: the real-conflict, shimmed-abort-failure, unreadable-verification, and non-conflict-failure cases pass with nonzero counts (verification: unit - `src/vcs/git/commands/merge.rs`; `cargo test --lib --features web-monitoring,heavy-tests remote_merge_abort -- --nocapture`; verification-id: remote-merge-abort-vcs).
- [ ] Add `MergeAttempt::ConflictAborted`, `WorktreeOpError::PredictedConflict` and `MergeConflictAborted`, the backend mapping in `src/worktree_ops/git_backend.rs`, the service arm in `src/worktree_ops/service.rs` (event text, no `on_merged`), and the TUI message mapping in `src/tui/command_handlers.rs`. Completion: service fake-backend cases and the TUI message case pass, and `PreserveConflict` behavior is unchanged (verification: unit - `src/worktree_ops/service/tests.rs`, `src/tui/command_handlers.rs`; `cargo test --lib --features web-monitoring,heavy-tests remote_merge_abort -- --nocapture`; verification-id: remote-merge-abort-api).
- [ ] Add the `merge_worktree_abort_on_conflict` command variant, the `merge_abort` result kind, the `merge_conflict_predicted` / `merge_conflict_aborted` / `merge_abort_failed` error codes, the `merge_abort` capability block, the narrowed `merge_conflict_preserves_state` description, the `merge_abort` operation, the port method, executor dispatch, and `map_worktree_error` in `src/web/remote_control_api/`. Completion: the `merge_abort_tests.rs` module registered in `tests/mod.rs` passes every case in `design.md`, including predicted refusal, restoration evidence, root busy after a failed abort, next merge after a confirmed abort, and unchanged `merge_worktree` (verification: integration - `src/web/remote_control_api/tests/mod.rs`; `cargo test --lib --features web-monitoring,heavy-tests remote_merge_abort -- --nocapture`; verification-id: remote-merge-abort-api).
- [ ] Extend `src/web/openapi.rs` and `tests/openapi_contract_tests.rs` with the end-to-end publication case, the `merge_conflict_preserves_state` still-required-and-true assertion, and the omission negative control; update the worktree capability `operations` assertion for `"merge_abort"`. Completion: both contract cases pass and the CLI export equals the live document (verification: integration - `tests/openapi_contract_tests.rs`; `cargo test --features web-monitoring --test openapi_contract_tests remote_merge_abort -- --nocapture`; verification-id: remote-merge-abort-contract).
- [ ] Update `docs/guides/USAGE.md` and `AGENTS.md`: the two merge commands, predicted-conflict refusal, the three restoration verifications, the difference between `merge_conflict_aborted` and `merge_abort_failed`, `root_busy` after a failed abort, no Server-side compensation, and older-owner behavior. Completion: the text matches the implemented codes and never claims restoration from the attempt alone (verification: manual - documentation review of `docs/guides/USAGE.md` and `AGENTS.md` against `src/vcs/git/commands/merge.rs` and the change-scoped `git diff --stat -- src/ tests/ docs/`; verification-id: remote-merge-abort-contract).
- [ ] Launch the independent implementation review exactly per the "Independent review launch contract" in `design.md` (`agent-exec run --until 100 --stdin-file /tmp/<UUID>.md -- claude-auto-fable …`, no `--model`), and report job ID, raw log path, argv, responding model, and final answer without killing or duplicating a running job. Completion: the review artifacts are attached to the handoff and no push, install, or owner replacement was performed (verification: manual - review artifacts recorded against `openspec/changes/add-remote-abort-merge/design.md` and the change-scoped `git diff --stat -- src/ tests/ docs/`; verification-id: remote-merge-abort-api).

## Final Validation

All three declared gates must select and execute their `remote_merge_abort`
cases; zero selected tests is a failure. Archive validation is the
authoritative OpenSpec structural/evidence gate:
`cflx openspec validate add-remote-abort-merge --archive-gate`. Authoring
uses `cflx openspec validate add-remote-abort-merge --strict`. Neither
command implements or archives this change, and structural PASS is not
implementation PASS.

## Future Work

Web presentation of the abort evidence and the Server relay (CFSRV-55) are
the Server's work against this contract. Remote conflict resolution remains
out of scope.

## Implementation Tasks

- [x] Add an explicit process-local workspace-preparation activity/event in `src/events.rs` and reducer transition in `src/orchestration/state.rs`; every path that leaves dispatch after emission but before a next-phase event, including global cancellation and pre-spawn early return, must emit a reducer-visible clearing or terminal transition without persisting preparation as resume evidence. (verification: unit - `cargo test --all-features orchestration::state::tests::preparing` proves `queued → preparing → next phase|error`, eventless-exit prevention, and restart routing that ignores ephemeral preparation; verification-id: local-tests)
- [x] Wire parallel dispatch under `src/parallel/` to emit preparation only after the slot permit is acquired and stop/terminal gates pass, immediately before force-recreate cleanup or slow workspace operations, including dependency-resolved fresh-workspace recreation; preserve repository-derived routing when the prepared workspace resumes at acceptance, rejection, archive, or resolve instead of Apply. (verification: integration - `cargo test --all-features parallel::tests::executor::preparing` uses controlled workspaces to prove waiting candidates remain `queued`, admitted candidates become `preparing`, and fresh/resumed event ordering is correct; verification-id: local-tests)
- [x] Project `preparing` consistently through `src/tui/`, `src/web/`, `web/app.js`, `/api/v2`, lifecycle/operator facts, active-status classifiers, and API documentation when the contract enumerates statuses; add `tests/web/` coverage for the Active group. (verification: integration - `cargo test --all-features preparing_projection && make web-test && make check-openapi` asserts the same `preparing` token from one execution event, operator-console grouping, and generated-contract consistency; verification-id: local-tests)
- [x] Treat `preparing` as active execution in `src/orchestration/operator_command.rs` and TUI worktree-delete safety checks using the current inline-preparation contract: without a termination handle, dequeue is refused with `MissingCancellationHandle`, the stop mark remains recorded, and execution stops after preparation before operation-agent startup. (verification: unit - `cargo test --all-features preparing_is_active` rejects unsafe mutation and proves missing-handle refusal plus stop-mark enforcement after preparation; verification-id: local-tests)
- [x] Emit bounded `.wt/setup` start, completion-duration, and failure diagnostics from the worktree setup command path under `src/vcs/git/` without making diagnostics or elapsed time workflow-control inputs. (verification: integration - `cargo test --all-features worktree_setup_preparing` uses a setup command test double to verify start/completion ordering, elapsed diagnostic shape, and actionable non-zero failure; verification-id: local-tests)
- [x] Update canonical user-facing status documentation and generated API documentation required by the implementation. (verification: integration - `make check-openapi && cargo test --all-features preparing_projection` detects stale generated contracts or omitted `preparing` mappings; verification-id: local-tests)
- [x] Run formatting, linting, targeted tests, default tests, and web tests; optimize any new test over one second or mark it heavy according to repository policy. (verification: integration - `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test && make web-test`; verification-id: local-tests)

## Future Work

- Command-level setup progress and duration estimates may be proposed separately if a binary preparing state is insufficient.

## Notes

- The two new events are `WorkspacePreparationStarted` / `WorkspacePreparationEnded`. Started is refused for any change that is not idle, terminal, or dequeued; Ended only undoes `Preparing`, so both are idempotent and cannot regress a real transition.
- Eventless-exit prevention is centralised rather than sprinkled across the ~50 early returns inside the spawned workspace task: `handle_workspace_completion` clears preparation for every ordinary return, and the two cancellation drains clear it for aborted tasks that never reach that funnel.
- The pre-spawn failure paths (`get_or_create_workspace`, `ensure_original_branch_initialized`) propagate through `?` into the dispatch loop's existing `ProcessingError`, which is itself a terminal clearing transition.
- `docs/openapi.yaml` does not enumerate display statuses or wire event types, so the generated contract is unchanged; the canonical taxonomy comment in `src/web/state.rs` was updated instead.
- The two `parallel::tests::executor::preparing*` tests drive real Git worktree creation and a real blocking `.wt/setup` process, so they are gated behind `heavy-tests` per repository policy; `--all-features` enables that tier.
- evidence: `cargo test --all-features preparing` -> 19 passed, 0 failed.
- evidence: `npm --prefix tests/web test` (`make web-test`) -> 172 passed across 8 files.
- evidence: `cargo fmt --all --check` -> clean; `cargo clippy --all-targets --all-features -- -D warnings` -> clean.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate show-workspace-preparing-status --archive-gate`

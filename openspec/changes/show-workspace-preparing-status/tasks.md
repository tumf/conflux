## Implementation Tasks

- [ ] Add an explicit process-local workspace-preparation activity/event in `src/events.rs` and reducer transition in `src/orchestration/state.rs` before worktree creation, recreation, setup, or pre-operation inspection; clear it through every success, alternate-route, cancellation, and failure path without persisting it as resume evidence. (verification: unit - `cargo test --all-features orchestration::state::tests::preparing` proves `queued → preparing → next phase|error` and restart routing ignores ephemeral preparation; verification-id: local-tests)
- [ ] Wire parallel dispatch under `src/parallel/` to emit preparation before invoking slow workspace operations, including dependency-resolved fresh-workspace recreation, and preserve repository-derived routing when the prepared workspace resumes at acceptance, rejection, archive, or resolve instead of Apply. (verification: integration - `cargo test --all-features parallel::tests::executor::preparing` uses a controlled workspace to hold preparation open and assert event ordering for fresh and resumed workspaces; verification-id: local-tests)
- [ ] Project `preparing` consistently through `src/tui/`, `src/web/`, `/api/v2`, lifecycle/operator facts, active-status classifiers, and `docs/openapi.yaml` where status values are enumerated. (verification: integration - `cargo test --all-features preparing_projection && make check-openapi` asserts the same `preparing` token from one execution event and detects generated-contract drift; verification-id: local-tests)
- [ ] Treat `preparing` as active execution in `src/orchestration/operator_command.rs` and TUI worktree-delete safety checks while preserving confirmed cancellation behavior. (verification: unit - `cargo test --all-features preparing_is_active` rejects unsafe mutation and exercises cancellation from preparation; verification-id: local-tests)
- [ ] Emit bounded `.wt/setup` start, completion-duration, and failure diagnostics from the worktree setup command path under `src/vcs/git/` without making diagnostics or elapsed time workflow-control inputs. (verification: integration - `cargo test --all-features worktree_setup_preparing` uses a setup command test double to verify start/completion ordering, elapsed diagnostic shape, and actionable non-zero failure; verification-id: local-tests)
- [ ] Update canonical user-facing status documentation and generated API documentation required by the implementation. (verification: integration - `make check-openapi && cargo test --all-features preparing_projection` detects stale generated contracts or omitted `preparing` mappings; verification-id: local-tests)
- [ ] Run formatting, linting, targeted tests, default tests, and web tests; optimize any new test over one second or mark it heavy according to repository policy. (verification: integration - `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test && make web-test`; verification-id: local-tests)

## Future Work

- Command-level setup progress and duration estimates may be proposed separately if a binary preparing state is insufficient.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate show-workspace-preparing-status --archive-gate`

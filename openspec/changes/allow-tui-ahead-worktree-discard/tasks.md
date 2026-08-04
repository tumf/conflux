## Implementation Tasks

- [ ] Add a typed commits-ahead deletion refusal and a local-only `allow_commits_ahead` policy independent from `allow_known_dirty` and `skip_teardown`; include fresh path, identity, branch, HEAD, and dirty evidence needed for explicit confirmation, while `DeleteOptions::fail_closed()` keeps every destructive permission disabled. (verification: unit - add the policy matrix to `src/worktree_ops/service/tests.rs` and run `cargo test --all-features worktree_ops::service::tests`; verification-id: local-tests)
- [ ] Wire the service deletion path to accept explicitly authorized ahead state only after post-teardown re-observation and exact identity/branch/HEAD/ref checks, refusing all changed or unknown safety facts before removal. (verification: unit - add teardown-order and drift cases to `src/worktree_ops/service/tests.rs` and run `cargo test --all-features worktree_ops::service::tests`; verification-id: local-tests)
- [ ] Add a backend operation for force-deleting only the explicitly confirmed local branch, keep ordinary cleanup on merged-only deletion, and retain/report the branch if its ref moved, cannot be read, or deletion fails after worktree removal. (verification: unit - cover backend/service call ordering and partial success in `src/worktree_ops/service/tests.rs`, then run `cargo test --all-features worktree_ops::service::tests`; verification-id: local-tests)
- [ ] Extend TUI modal and delete-intent state with a dedicated ahead-discard confirmation that discloses path, branch, HEAD, teardown choice, unmerged commit loss, branch deletion, and dirty loss when present; revalidate the target before command emission. (verification: unit - add state/render cases under `src/tui/state.rs`, `src/tui/state/modal_logic.rs`, and `src/tui/render.rs`, then run `cargo test --all-features tui::`; verification-id: local-tests)
- [ ] Restrict ahead discard submission to uppercase `X`, leaving `Y`, `S`, lowercase `x`, and unrelated keys inert while `N` and Escape cancel; route the confirmed intent through the shared service and surface full success, refusal, and partial-success logs. (verification: integration - exercise modal-to-service behavior in `src/tui/key_handlers.rs` and `src/tui/command_handlers.rs` tests with `cargo test --all-features tui::`; verification-id: local-tests)
- [ ] Preserve remote fail-closed behavior without adding DTO fields or controls: ahead worktrees remain undeletable in projection and delete commands cannot request dirty, ahead, teardown-skip, path, branch, or generic force permissions. (verification: integration - add projection and request-shape cases to `src/web/remote_control_api/tests/worktree_tests.rs` and run `cargo test --all-features web::remote_control_api::tests`; verification-id: local-tests)
- [ ] Update operator-facing TUI/worktree documentation to describe ordinary deletion, dirty discard, ahead discard, combined loss, partial branch retention, and the remote safety boundary. (verification: unit - review `docs/guides/USAGE.md` and `docs/guides/WEBUI.md` against `src/tui/render.rs`, then run `cargo test --all-features`; verification-id: local-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate allow-tui-ahead-worktree-discard --archive-gate`

Repository quality gates: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`.

## Future Work

- No remote ahead-discard control is planned. A future request would require a separate threat and authorization review rather than reusing this local permission.

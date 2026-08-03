## Implementation Tasks

- [x] Replace local unknown-dirty permission with `allow_known_dirty`, make dirty/base/ahead/merge/identity/ref observation failures explicit and fail-closed, and make status request explicit non-ignored untracked entries. Completion requires policy tests for clean, known dirty, ignored-only, unknown dirty, unknown ahead/base/merge, main, and ahead states. (verification: unit - `cargo test --lib dirty_discard -- --list | grep -q dirty_discard && cargo test --lib dirty_discard`; verification-id: dirty-worktree-delete-tests)

- [x] Split backend teardown and Git worktree removal, re-observe safety facts after teardown or immediately before removal when skipped, and retain the branch when its ref moved or safe reachability cannot be reconfirmed. Completion requires real-Git tests that mutate HEAD/ref or create commits during teardown and prove forced removal never uses stale facts to delete the branch. (verification: e2e - `cargo test --features heavy-tests --test e2e_git_worktree_tests tui_dirty_worktree_delete -- --list | grep -q tui_dirty_worktree_delete && cargo test --features heavy-tests --test e2e_git_worktree_tests tui_dirty_worktree_delete`; verification-id: dirty-worktree-delete-tests)

- [x] Inject one repository-scoped `Arc<WorktreeService>` or shared guard into TUI and Web runtime paths instead of constructing isolated per-command services. Completion requires concurrency tests proving overlapping Conflux mutations serialize/refuse and review evidence that every TUI and `/api/v2` worktree mutation uses the shared boundary. (verification: integration - `cargo test --lib dirty_discard`; verification-id: dirty-worktree-delete-tests)

- [x] Add typed `DeleteIntent` and `ConfirmDirtyDiscard` state carrying path, expected Git identity, branch, HEAD, and captured skip-teardown policy; transition only from the shared service's fresh `Dirty` refusal. Completion requires tests proving clean deletion completes directly and dirty/unknown/non-dirty errors take distinct paths without adding dirty state to `WorktreeInfo` or remote DTOs. (verification: unit/integration - `cargo test --lib tui_dirty_worktree_delete -- --list | grep -q tui_dirty_worktree_delete && cargo test --lib tui_dirty_worktree_delete`; verification-id: dirty-worktree-delete-tests)

- [x] Implement and render the exact key matrix: ordinary `Y` selects teardown, `S` selects skip-teardown, neither grants discard; destructive uppercase `X` grants known-dirty discard with the captured teardown bit; `N`/`Esc` cancel and all other keys do nothing. Recheck active/deleting state before dispatch, adding a process-local delete reservation honored by run/queue admission only if needed to close the activation race. Completion requires key, modal-copy, payload, cancellation, and active-transition tests. (verification: unit/integration - `cargo test --lib tui_dirty_worktree_delete`; verification-id: dirty-worktree-delete-tests)

- [x] Preserve the closed remote surface and existing fail-closed errors without adding WebUI controls. Completion requires non-empty tests rejecting `allow_dirty`, `dirty_discard`, `force`, `skip_teardown`, `path`, and `branch`, plus unchanged OpenAPI verification and no removal delegation. (verification: integration - `cargo test --lib remote_worktree_dirty_discard -- --list | grep -q remote_worktree_dirty_discard && cargo test --lib remote_worktree_dirty_discard && make check-openapi`; verification-id: dirty-worktree-delete-tests)

- [ ] Run the repository-local gate and fix failures without adding stash/backup, ignored-file enumeration, remote unsafe controls, or durable workflow state. Completion requires non-empty filtered tests, real-Git heavy coverage, OpenAPI consistency, formatting, the default suite, and all-feature clippy to pass. (verification: integration - `cargo test --lib tui_dirty_worktree_delete -- --list | grep -q tui_dirty_worktree_delete && cargo test --lib tui_dirty_worktree_delete && cargo test --lib dirty_discard -- --list | grep -q dirty_discard && cargo test --lib dirty_discard && cargo test --lib remote_worktree_dirty_discard -- --list | grep -q remote_worktree_dirty_discard && cargo test --lib remote_worktree_dirty_discard && cargo test --features heavy-tests --test e2e_git_worktree_tests tui_dirty_worktree_delete -- --list | grep -q tui_dirty_worktree_delete && cargo test --features heavy-tests --test e2e_git_worktree_tests tui_dirty_worktree_delete && make check-openapi && cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings`; verification-id: dirty-worktree-delete-tests)

## Notes

Shared-boundary review evidence for the injection task, stated at the scope it
actually holds at. Every TUI and `/api/v2` *operator worktree operation* —
create, delete, merge of an addressed worktree — now goes through the single
`Arc<WorktreeService>` built once in `src/tui/runner.rs` and handed to both
`TuiCommandContext::worktree_service` and `RemoteWorktreeOperations`. Reviewed by
grepping `worktree_remove`, `worktree_add`, and `branch_delete` across `src/tui/`
and `src/web/`; the only remaining direct Git worktree calls in those trees are
the `+` key's ad-hoc `ws-session-*` scratch worktree
(`src/tui/key_handlers.rs`) and its own setup-failure rollback. That path is a
separate capability: it never addresses a managed worktree, and it is not routed
through the shared guard by this change.

`src/vcs/git/mod.rs` worktree removal belongs to the orchestrator's own workspace
lifecycle, not the operator surface, and is likewise outside this change.

Activation race, stated at the guarantee actually delivered: active/deleting
state is rechecked from the latest TUI observation immediately before dispatch
(`modal_logic::evaluate_dirty_discard`), and repository facts are revalidated
under the service's mutation guard. No process-local delete reservation was
added, so this is not atomic exclusion against an activation that lands after
dispatch — the design permits that limit and requires it not be overclaimed.

WebUI review evidence: `web/app.js` builds `delete_worktree` with
`target: { worktree_id }` and `params: {}` only, and gained no control in this
change.

Two deliberate behavior tightenings that follow from making unknown states
refuse, recorded so they are not mistaken for accidents:

- A detached worktree, or one whose branch Git does not report, has no
  commits-ahead answer to give, so its `has_commits_ahead` is `Unknown` and
  `/api/v2` deletion now refuses it. The TUI already refused such a worktree for
  lack of a revalidatable identity, so no frontend loses a path it had.
- When `.wt/teardown` itself leaves tracked or reported-untracked changes behind,
  the second observation sees a dirty worktree and an ordinary deletion refuses
  after teardown has run. The refusal is the same `Dirty` the operator can answer
  with the destructive confirmation, so the flow completes in one sitting; the
  teardown script runs again on that second attempt.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate allow-tui-dirty-worktree-delete --archive-gate`.

## Future Work

- Add recoverable export/stash only if operators later request preservation rather than intentional disposal.

## Implementation Tasks

- [x] Replace local unknown-dirty permission with `allow_known_dirty`, make dirty/base/ahead/merge/identity/ref observation failures explicit and fail-closed, and make status request explicit non-ignored untracked entries. Completion requires policy tests for clean, known dirty, ignored-only, unknown dirty, unknown ahead/base/merge, main, and ahead states. (verification: unit - `cargo test --lib dirty_discard -- --list | grep -q dirty_discard && cargo test --lib dirty_discard`; verification-id: dirty-worktree-delete-tests)

- [x] Split backend teardown and Git worktree removal, re-observe safety facts after teardown or immediately before removal when skipped, and retain the branch when its ref moved or safe reachability cannot be reconfirmed. Completion requires real-Git tests that mutate HEAD/ref or create commits during teardown and prove forced removal never uses stale facts to delete the branch. (verification: e2e - `cargo test --features heavy-tests --test e2e_git_worktree_tests tui_dirty_worktree_delete -- --list | grep -q tui_dirty_worktree_delete && cargo test --features heavy-tests --test e2e_git_worktree_tests tui_dirty_worktree_delete`; verification-id: dirty-worktree-delete-tests)

- [x] Inject one repository-scoped `Arc<WorktreeService>` or shared guard into TUI and Web runtime paths instead of constructing isolated per-command services. Completion requires concurrency tests proving overlapping Conflux mutations serialize/refuse and review evidence that every TUI and `/api/v2` worktree mutation uses the shared boundary. (verification: integration - `cargo test --lib dirty_discard`; verification-id: dirty-worktree-delete-tests)

- [x] Add typed `DeleteIntent` and `ConfirmDirtyDiscard` state carrying path, expected Git identity, branch, HEAD, and captured skip-teardown policy; transition only from the shared service's fresh `Dirty` refusal. Completion requires tests proving clean deletion completes directly and dirty/unknown/non-dirty errors take distinct paths without adding dirty state to `WorktreeInfo` or remote DTOs. (verification: unit/integration - `cargo test --lib tui_dirty_worktree_delete -- --list | grep -q tui_dirty_worktree_delete && cargo test --lib tui_dirty_worktree_delete`; verification-id: dirty-worktree-delete-tests)

- [x] Implement and render the exact key matrix: ordinary `Y` selects teardown, `S` selects skip-teardown, neither grants discard; destructive uppercase `X` grants known-dirty discard with the captured teardown bit; `N`/`Esc` cancel and all other keys do nothing. Recheck active/deleting state before dispatch, adding a process-local delete reservation honored by run/queue admission only if needed to close the activation race. Completion requires key, modal-copy, payload, cancellation, and active-transition tests. (verification: unit/integration - `cargo test --lib tui_dirty_worktree_delete`; verification-id: dirty-worktree-delete-tests)

- [x] Preserve the closed remote surface and existing fail-closed errors without adding WebUI controls. Completion requires non-empty tests rejecting `allow_dirty`, `dirty_discard`, `force`, `skip_teardown`, `path`, and `branch`, plus unchanged OpenAPI verification and no removal delegation. (verification: integration - `cargo test --lib remote_worktree_dirty_discard -- --list | grep -q remote_worktree_dirty_discard && cargo test --lib remote_worktree_dirty_discard && make check-openapi`; verification-id: dirty-worktree-delete-tests)

- [x] Run the repository-local gate and fix failures without adding stash/backup, ignored-file enumeration, remote unsafe controls, or durable workflow state. Completion requires non-empty filtered tests, real-Git heavy coverage, OpenAPI consistency, formatting, the default suite, and all-feature clippy to pass. (verification: integration - `cargo test --lib tui_dirty_worktree_delete -- --list | grep -q tui_dirty_worktree_delete && cargo test --lib tui_dirty_worktree_delete && cargo test --lib dirty_discard -- --list | grep -q dirty_discard && cargo test --lib dirty_discard && cargo test --lib remote_worktree_dirty_discard -- --list | grep -q remote_worktree_dirty_discard && cargo test --lib remote_worktree_dirty_discard && cargo test --features heavy-tests --test e2e_git_worktree_tests tui_dirty_worktree_delete -- --list | grep -q tui_dirty_worktree_delete && cargo test --features heavy-tests --test e2e_git_worktree_tests tui_dirty_worktree_delete && make check-openapi && cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings`; verification-id: dirty-worktree-delete-tests)

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

Gate evidence, recorded at the scope it actually holds at. Every step was re-run
against a private `CARGO_TARGET_DIR` because the repository's shared cargo target
directory is written by several worktrees at once and had served a stale test
binary (a run reporting 3038 tests where this branch has 3069). Results: filtered
`tui_dirty_worktree_delete` 18 tests, `dirty_discard` 24, and
`remote_worktree_dirty_discard` 4, all non-empty and passing; heavy real-Git
`e2e_git_worktree_tests` 6 passing; `make check-openapi` up to date;
`cargo fmt --check` clean; the default `cargo test` suite exiting 0 with 3060 lib
tests and every integration binary passing; and
`cargo clippy --all-targets --all-features -- -D warnings` exiting 0.

One default-suite test is excluded from that run and is not evidence for this
change either way: `run_exit_tests::killing_the_lock_owner_releases_the_repository_lock`
hangs indefinitely under machine load. The hang is pre-existing rather than
caused by this change — on a loaded machine the merge-base binary
(`a72ef831`) hung in 3 of 5 runs and this branch's binary hung in 5 of 5, and a
four-hour-old orphan of the same test from an unrelated gate run in a different
target directory was already stuck the same way. The mechanism is in the test,
not the product: `cflx_output` (`tests/run_exit_tests.rs:378`) waits on the child
pipes with no timeout, so whenever the polling loop's competing `cflx run --all`
wins the repository lock ahead of the spawned owner, it proceeds into the real
`sleep 120` orchestration and the harness blocks on EOF instead of reaching the
loop's 30-second assertion. Fixing that test belongs to whoever owns
`run_exit_tests`, not to this change. The remaining 27 tests in that binary pass.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate allow-tui-dirty-worktree-delete --archive-gate`.

## Future Work

- Add recoverable export/stash only if operators later request preservation rather than intentional disposal.
- Bound the child-process wait in `cflx_output` (`tests/run_exit_tests.rs:378`) so `killing_the_lock_owner_releases_the_repository_lock` fails with a diagnostic instead of hanging when the competing invocation wins the repository lock. Owned by `run_exit_tests`, not by this change.

## Notes on the pre-removal branch-ref gate

`WorktreeService::confirm_branch_ref` (`src/worktree_ops/service.rs`) runs after
the post-teardown re-observation and eligibility recheck and before
`remove_worktree`, so a branch ref that moved, vanished, or cannot be read
refuses the whole deletion instead of being discovered by best-effort cleanup
once the directory is already gone. A detached target (empty branch) has no
branch ref to reconfirm and is still covered by `classify_delete_drift`'s HEAD
check.

`cleanup_branch` keeps its own ref read rather than trusting that answer: the
removal itself is a window in which the ref can still move, which is the
"Branch ref drift preserves branch" scenario. The unit test for that scenario
now scripts two ref answers — intact at authorization, drifted at cleanup — so
it exercises that residual window rather than a state the gate would refuse.

## Current Acceptance Follow-up
- attempt: 1
- [x] [dirty-delete-pre-removal-branch-ref-validation] (major) 強制削除前にbranch refを検証しておらず、ref不一致・観測不能でもworktreeを削除する | evidence: openspec/changes/allow-tui-dirty-worktree-delete/specs/vcs-worktree-operations/spec.md:27-32はbranch refを確定できない場合にworktreeとbranchを保持するよう要求している; src/worktree_ops/service.rs:790-805は再観測とeligibility確認後、branch_refを読まずにremove_worktreeを実行する; src/worktree_ops/service.rs:807-859はbranch_refを強制削除後のbranch cleanupで初めて確認するため、ref不一致・観測失敗時にもworktreeは既に失われている; src/worktree_ops/service/tests.rs:729-777はref移動・観測失敗時にworktree削除が実行されることを期待しており、仕様のfail-closed要件を反証している | required_changes: src/worktree_ops/service.rs — 強制削除直前にbranch refを再観測し、現在のrefが検証済みHEADと一致しない、存在しない、または観測不能な場合はremove_worktreeを呼ばずに拒否する | verification: src/worktree_ops/service/tests.rs — branch refの移動・消失・観測失敗について、remove_worktreeとbranch削除が一切呼ばれず対象が保持されることを検証するテストを追加・修正する
  finding: {"evidence":["openspec/changes/allow-tui-dirty-worktree-delete/specs/vcs-worktree-operations/spec.md:27-32はbranch refを確定できない場合にworktreeとbranchを保持するよう要求している","src/worktree_ops/service.rs:790-805は再観測とeligibility確認後、branch_refを読まずにremove_worktreeを実行する","src/worktree_ops/service.rs:807-859はbranch_refを強制削除後のbranch cleanupで初めて確認するため、ref不一致・観測失敗時にもworktreeは既に失われている","src/worktree_ops/service/tests.rs:729-777はref移動・観測失敗時にworktree削除が実行されることを期待しており、仕様のfail-closed要件を反証している"],"id":"dirty-delete-pre-removal-branch-ref-validation","required_changes":[{"description":"強制削除直前にbranch refを再観測し、現在のrefが検証済みHEADと一致しない、存在しない、または観測不能な場合はremove_worktreeを呼ばずに拒否する","file":"src/worktree_ops/service.rs"}],"severity":"major","summary":"強制削除前にbranch refを検証しておらず、ref不一致・観測不能でもworktreeを削除する","verification":[{"description":"branch refの移動・消失・観測失敗について、remove_worktreeとbranch削除が一切呼ばれず対象が保持されることを検証するテストを追加・修正する","file":"src/worktree_ops/service/tests.rs"}]}
  evidence: src/worktree_ops/service.rs `confirm_branch_ref` refuses before `remove_worktree` when the branch ref moved, is gone, or is unreadable; src/worktree_ops/service/tests.rs adds `dirty_discard_refuses_removal_when_the_branch_ref_cannot_be_confirmed` (moved/missing/unreadable -> no RemoveWorktree, no DeleteBranch, no Deleted event) and `dirty_discard_refuses_when_the_branch_ref_moves_during_teardown`, and rescripts the cleanup test to drift only after removal is authorized; verified `cargo test --lib worktree_ops::` 32 passed, `--lib dirty_discard` 26, `--lib tui_dirty_worktree_delete` 18, `--lib remote_worktree_dirty_discard` 4, heavy `e2e_git_worktree_tests tui_dirty_worktree_delete` 6, `cargo test --lib` 3062 passed, `cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings` clean (private CARGO_TARGET_DIR).

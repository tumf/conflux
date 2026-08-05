## Implementation Tasks

- [x] Add a typed commits-ahead deletion refusal and a local-only `allow_commits_ahead` policy independent from `allow_known_dirty` and `skip_teardown`; include fresh path, identity, branch, HEAD, and known dirty evidence needed for explicit confirmation, while unknown dirty state remains a non-escalating refusal and `DeleteOptions::fail_closed()` keeps every destructive permission disabled. (verification: unit - add the policy matrix including ahead plus dirty-unknown and concurrent root-busy cases to `src/worktree_ops/service/tests.rs`, then run `cargo test --all-features worktree_ops::service::tests`; verification-id: local-tests)
- [x] Wire the service deletion path to accept explicitly authorized ahead state only after post-teardown re-observation and exact unchanged identity, branch, HEAD/ref, dirty, commits-ahead, and base-merge facts, refusing every drift or unknown fact before removal. (verification: unit - add teardown-order, all-fact drift, root-busy, and worktree-removal-failure cases to `src/worktree_ops/service/tests.rs`, then run `cargo test --all-features worktree_ops::service::tests`; verification-id: local-tests)
- [x] Add a backend operation that accepts the expected OID and atomically compare-and-deletes only the explicitly confirmed local branch, keep ordinary cleanup on merged-only deletion, and retain/report the branch if the comparison fails, the ref is missing or unreadable, or deletion fails after worktree removal. (verification: unit - cover atomic expected-OID deletion, moved/missing/unreadable refs, backend/service call ordering, and partial success in `src/worktree_ops/service/tests.rs`, then run `cargo test --all-features worktree_ops::service::tests`; verification-id: local-tests)
- [x] Extend TUI modal and delete-intent state with a dedicated ahead-discard confirmation that discloses path, branch, HEAD, teardown choice, unmerged commit loss, branch deletion, and dirty loss when present; revalidate the target before command emission. (verification: unit - add state/render cases under `src/tui/state.rs`, `src/tui/state/modal_logic.rs`, and `src/tui/render.rs`, then run `cargo test --all-features tui::`; verification-id: local-tests)
- [x] Restrict ahead discard submission to uppercase `X`, leaving `Y`, `S`, lowercase `x`, and unrelated keys inert while `N` and Escape cancel; route the confirmed intent through the shared service and surface full success, refusal, and partial-success logs. (verification: integration - exercise modal-to-service behavior in `src/tui/key_handlers.rs` and `src/tui/command_handlers.rs` tests with `cargo test --all-features tui::`; verification-id: local-tests)
- [x] Preserve remote fail-closed behavior without adding DTO fields or controls: map the typed ahead refusal to the existing ineligible error class, keep ahead worktrees undeletable in projection, prevent absolute target paths from appearing in blocked reasons, and reject dirty, ahead, teardown-skip, path, branch, or generic force parameters. (verification: integration - add error mapping, path redaction, projection, and request-shape cases to `src/web/remote_control_api/tests/worktree_tests.rs` and run `cargo test --all-features web::remote_control_api::tests`; verification-id: local-tests)
- [x] Update operator-facing TUI/worktree documentation to describe ordinary deletion, dirty discard, ahead discard, combined loss, partial branch retention, and the remote safety boundary. (verification: unit - review `docs/guides/USAGE.md` and `docs/guides/WEBUI.md` against `src/tui/render.rs`, then run `cargo test --all-features`; verification-id: local-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate allow-tui-ahead-worktree-discard --archive-gate`

Repository quality gates: `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`.

Status of those gates on this branch:

- `cargo fmt` — applied; tree is formatted.
- `cargo clippy --all-targets --all-features -- -D warnings` — passes with exit code 0.
- `cargo test --all-features` — 3362 passed, 6 failed, 7 ignored. Every test added by this change passes. Of the 6 failures, `serial_run_service::tests::serial_restart_reruns_acceptance_after_missing_verdict_exhaustion` passes in isolation (it fails only under parallel execution, with `Unable to read current working directory` — cross-test working-directory pollution), and the remaining 5 are the pre-existing `parallel::tests::executor::test_merge_*` cases described under "Pre-existing failures" below.

## Pre-existing failures

The 5 `parallel::tests::executor::test_merge_*` failures are pre-existing on this branch and are not caused by this change. This was confirmed by reproduction on a clean baseline rather than by inspection: a detached worktree was created at this branch's HEAD (`e08431b3`), which contains none of this change's work, and built against its own target directory. `cargo test --all-features --lib -- --test-threads=1 parallel::tests::executor::test_merge` there produced `4 passed; 5 failed` with exactly the same five test names and the same underlying errors:

- `test_merge_conflict_path_emits_resolve_started_event`, `test_merge_resolves_conflict_with_resolve_command`, `test_merge_retries_when_merge_commit_missing` — `evidence_withheld` / `Branch 'change-a' tip … has no valid archive proposal for 'change-a'`
- `test_merge_retries_after_pre_commit_changes` — `evidence_withheld` / `Every item is integrated but the target index or worktree is not clean`
- `test_merge_conflictless_path_skips_resolve_started_event` — `retries_exhausted` after 2 attempts

They originate in `src/parallel/resolve_state.rs` archive-proposal validation. This change touches no file under `src/parallel/**`, and `src/parallel/**` contains no reference to `worktree_ops`. Fixing them belongs to a separate change against the parallel merge/resolve capability.

## Future Work

- No remote ahead-discard control is planned. A future request would require a separate threat and authorization review rather than reusing this local permission.

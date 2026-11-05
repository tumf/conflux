## Implementation Tasks

- [ ] Add `src/worktree_ops/consent.rs` (`DeleteConsent`, `ConfirmedDeleteReason`, the bounded `discardable_content_id` oracle over `CONSENT_STATUS_ARGS` from `src/vcs/git/commands/status_policy.rs` and `O_NOFOLLOW` byte hashing) and `WorktreeService::delete_worktree_with_consent` with the pre-teardown and post-teardown consent comparisons layered on the unchanged identity, drift, eligibility, branch-ref, removal, and cleanup steps. Completion: service and oracle cases pass with nonzero counts and the status argv inventory test accepts the new call site (verification: unit - `src/worktree_ops/service/tests.rs`, `src/worktree_ops/consent.rs`, `src/vcs/git/commands/status_policy_repository_tests.rs`; `cargo test --lib --features web-monitoring,heavy-tests remote_confirmed_delete -- --nocapture`; verification-id: remote-confirmed-delete-service).
- [ ] Add the `delete-preview` route, `DeletePreviewResponse`, `ConfirmedDeleteParams`, the `delete_worktree_confirmed` command variant with `validate()`, the `delete_worktree_confirmed` result kind (carried by refusals), the `delete_consent_stale` error code, the `confirmed_delete` capability block, the narrowed `delete_requires_teardown` description, and the `delete_confirmed` operation in `src/web/remote_control_api/`; wire the port method on `RemoteWorktreeOperations` and executor dispatch. Completion: the real-Git `confirmed_delete_tests.rs` module registered in `tests/mod.rs` passes every case in `design.md`, including same-HEAD same-dirty content drift, cancel, teardown failure recovery, ahead+dirty, unknown, main, and busy root, and `delete_worktree` tests pass unchanged (verification: integration - `src/web/remote_control_api/tests/mod.rs`; `cargo test --lib --features web-monitoring,heavy-tests remote_confirmed_delete -- --nocapture`; verification-id: remote-confirmed-delete-api).
- [ ] Extend `src/web/openapi.rs` and `tests/openapi_contract_tests.rs` with the end-to-end publication case, the `delete_requires_teardown` still-required-and-true assertion, and the omission negative control; update the worktree capability `operations` assertion for `"delete_confirmed"`. Completion: both contract cases pass and the CLI export equals the live document (verification: integration - `tests/openapi_contract_tests.rs`; `cargo test --features web-monitoring --test openapi_contract_tests remote_confirmed_delete -- --nocapture`; verification-id: remote-confirmed-delete-contract).
- [ ] Update `docs/guides/USAGE.md` ("Deleting a Worktree from the TUI" remote paragraph and the `/api/v2` notes) and `AGENTS.md` with the preview/consent flow, the three independent permissions, content-identity binding, the stale and unknown refusals, the post-teardown refusal, the guarantee boundary, and older-owner behavior. Completion: the text matches the implemented reasons and never claims ordinary deletion changed (verification: manual - documentation review of `docs/guides/USAGE.md` and `AGENTS.md` against `src/worktree_ops/consent.rs` and the change-scoped `git diff --stat -- src/ tests/ docs/`; verification-id: remote-confirmed-delete-contract).
- [ ] Launch the independent implementation review exactly per the "Independent review launch contract" in `design.md` (`agent-exec run --until 100 --stdin-file /tmp/<UUID>.md -- claude-auto-fable …`, no `--model`), and report job ID, raw log path, argv, responding model, and final answer without killing or duplicating a running job. Completion: the review artifacts are attached to the handoff and no push, install, or owner replacement was performed (verification: manual - review artifacts recorded against `openspec/changes/add-remote-confirmed-delete/design.md` and the change-scoped `git diff --stat -- src/ tests/ docs/`; verification-id: remote-confirmed-delete-api).

## Final Validation

All three declared gates must select and execute their
`remote_confirmed_delete` cases; zero selected tests is a failure. Archive
validation is the authoritative OpenSpec structural/evidence gate:
`cflx openspec validate add-remote-confirmed-delete --archive-gate`.
Authoring uses `cflx openspec validate add-remote-confirmed-delete --strict`.
Neither command implements or archives this change, and structural PASS is
not implementation PASS.

## Future Work

Binding the TUI dirty/ahead confirmations to the same content identity is a
separate follow-up. The Web two-step confirmation UI and Server relay
(CFSRV-52) are the Server's work against this contract.

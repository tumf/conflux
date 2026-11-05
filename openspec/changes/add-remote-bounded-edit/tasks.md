## Implementation Tasks

- [ ] Add `src/web/remote_control_api/text_edit.rs` with the path rules, the `O_NOFOLLOW` descriptor walk, `sha256:` content identity, the pure `classify_text_eligibility`, the bounded read, and the temporary-file/`renameat` write with mode preservation and failure cleanup as specified in `design.md`. Completion: unit cases cover the lexical table, the sha256 vector, mode preservation, and temporary-file removal on a forced failure (verification: unit - `src/web/remote_control_api/text_edit.rs`; `cargo test --lib --features web-monitoring,heavy-tests remote_text_edit -- --nocapture`; verification-id: remote-text-edit-api).
- [ ] Add the DTOs (`TextReadResponse`, `TextWriteParams`, `TextExpectation`, `TextWriteResult`, `TextEditCapability`, `TextEditBlockedReason`), the `write_change_text` / `write_worktree_text` command variants with `deny_unknown_fields` and `validate()` size/NUL/identity checks, the `text_write` result kind, the `text_stale` / `text_not_editable` error codes, and the `#[serde(default)]` capability block in `src/web/remote_control_api/dto.rs`. Completion: closed-command, validation, and old-owner decode cases pass (verification: integration - `src/web/remote_control_api/tests/dto_tests.rs`; `cargo test --lib --features web-monitoring,heavy-tests remote_text_edit -- --nocapture`; verification-id: remote-text-edit-api).
- [ ] Wire the two read routes in `src/web/remote_control_api/reads.rs` and `mod.rs`, the worktree-scope operations on the `WorktreeOperations` port in `worktrees.rs` (identity resolution, post-write observation refresh), the change-scope root source on `WebState`, `WorktreeService::root_guard_is_held`, and executor dispatch for both commands with the `text_written` state event. Completion: the real-Git `text_edit_tests.rs` module registered in `tests/mod.rs` passes every case listed in `design.md`, including stale, absent, containment, writer exclusion, cross-target isolation, and replay (verification: integration - `src/web/remote_control_api/tests/mod.rs`; `cargo test --lib --features web-monitoring,heavy-tests remote_text_edit -- --nocapture`; verification-id: remote-text-edit-api).
- [ ] Extend `src/web/openapi.rs` (`SUPPORTED_V2_PATHS`, schemas, result kind) and `tests/openapi_contract_tests.rs` with the positive end-to-end case and the omission negative control; update the existing worktree capability `operations` assertion for `"text"`. Completion: both contract cases pass and `cflx openapi` and the live document remain byte-identical (verification: integration - `tests/openapi_contract_tests.rs`; `cargo test --features web-monitoring --test openapi_contract_tests remote_text_edit -- --nocapture`; verification-id: remote-text-edit-contract).
- [ ] Document the surface in `docs/guides/USAGE.md` (routes, commands, limits, excluded paths, the compare-and-rename race boundary, writer exclusion reasons, no commit/push, old-owner behavior) and in `AGENTS.md` under the local API socket section. Completion: the text matches the implemented reasons and limits and does not claim external editors are excluded (verification: manual - documentation review of `docs/guides/USAGE.md` and `AGENTS.md` against `src/web/remote_control_api/text_edit.rs` and the change-scoped Git diff; verification-id: remote-text-edit-contract).
- [ ] Launch the independent implementation review exactly per the "Independent review launch contract" in `design.md` (`agent-exec run --until 100 --stdin-file /tmp/<UUID>.md -- claude-auto-fable …`, no `--model`), and report job ID, raw log path, argv, responding model, and final answer without killing or duplicating a running job. Completion: the review artifacts are attached to the handoff and no push, install, or owner replacement was performed (verification: manual - review artifacts recorded against `openspec/changes/add-remote-bounded-edit/design.md` and the change-scoped `git diff --stat -- src/ tests/ docs/`; verification-id: remote-text-edit-api).

## Final Validation

Both declared gates must select and execute their `remote_text_edit` cases;
zero selected tests is a failure. Archive validation is the authoritative
OpenSpec structural/evidence gate:
`cflx openspec validate add-remote-bounded-edit --archive-gate`. Authoring
uses `cflx openspec validate add-remote-bounded-edit --strict`. Neither
command implements or archives this change, and structural PASS is not
implementation PASS.

## Future Work

Server-side editor UI and the stale-text client behavior (CFSRV-50) are the
Server's work against this contract. Binary content, rename/delete, and
directory listing are not planned.

---
change_type: implementation
priority: high
dependencies:
  - add-instance-remote-control-api
references:
  - "openspec/CONSTITUTION.md"
  - "openspec/specs/web-monitoring/spec.md"
  - "openspec/changes/add-instance-remote-control-api/proposal.md"
  - "src/worktree_ops.rs"
  - "src/tui/command_handlers.rs"
  - "src/parallel/acceptance_state.rs"
  - "src/web/api.rs"
  - "src/web/state.rs"
verifications:
  - id: remote-worktree-local
    requirement: Opaque worktree identity, safe list/create/delete/merge behavior, conflict preservation, and shared TUI/API operations are covered by non-empty repository-local tests.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/worktree_ops.rs
    evidence: non-empty remote_worktree test listing and passing filtered test output
    rerun: cargo test --lib remote_worktree -- --list | grep -q remote_worktree && cargo test --lib remote_worktree
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-worktree-heavy
    requirement: Real Git repositories and worktrees verify teardown, dirty guards, merge conflicts, hook ordering, and identity retirement outside the default suite.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/e2e_git_worktree_tests.rs
    evidence: non-empty heavy remote_worktree test listing and passing filtered test output
    rerun: cargo test --features heavy-tests --test e2e_git_worktree_tests remote_worktree -- --list | grep -q remote_worktree && cargo test --features heavy-tests --test e2e_git_worktree_tests remote_worktree
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Change: secure remote worktree operations

**Change Type**: implementation

## Problem / Context

Legacy web worktree operations expose frontend-shaped inputs, including paths and a general worktree command. A remote-control client needs stable resource identities and must not gain arbitrary command execution, teardown bypasses, or direct absolute-path disclosure.

This change extends the `/api/v2` resources from `add-instance-remote-control-api`. It reuses the same worktree operation service as TUI so create, delete, merge, hooks, and events cannot diverge.

## Proposed Solution

Add exact worktree resources:

- `GET /api/v2/worktrees`
- `GET /api/v2/worktrees/{worktree_id}`

Extend the closed v2 command enum with `create_worktree`, `delete_worktree`, and `merge_worktree`. All inherit required authentication, `expected_revision`, idempotency, typed errors, registry admission, and correlation validation.

`create_worktree` uses `target: { "change_id": "<id>" }` and an empty `params` object. The change must exist, be a managed non-archived change eligible for a change worktree, and not already have one. The base is the current managed base HEAD; clients cannot supply branch, path, or base commit. An existing worktree returns `409 worktree_exists`, never a no-op.

`delete_worktree` and `merge_worktree` use `target: { "worktree_id": "<opaque-id>" }` and empty `params`. Mutation targets accept no path or branch fields.

Each observed worktree receives a process-local opaque `worktree_id`: a random 128-bit hexadecimal value allocated on first observation. When the resource disappears, its ID is retired; recreating a worktree at the same path receives a new ID.

Responses expose `repository_id` as a 16-character hexadecimal FNV-1a 64-bit hash over the canonical repository identity already used by `repository_identity(repo_root)`. It is an authenticated-client correlation value, not a secret, authorization input, or guarantee against path dictionary inference. Responses still never directly serialize canonical/absolute repository roots; worktree paths are repository-relative display values.

Deletion is fail-closed: unknown dirty state is represented as `dirty: null` and is not deletable; managed teardown is mandatory; `skip_teardown`, arbitrary recovery bypasses, path targets, branch targets, and generic worktree commands are not part of v2.

Merge uses the same base-merge operation, `on_merged` hook, reducer/event path, and repository locking as TUI. A Git conflict is a failed command with retained intermediate merge state and conflict file evidence; the service does not auto-abort it. The failed command and capabilities response state that recovery requires local/TUI resolve or abort; v2 intentionally offers no remote recovery command and returns `root_busy` for incompatible mutations until recovery.

## Acceptance Criteria

1. Authenticated clients can list worktrees, fetch exact detail by opaque ID, and use only the defined change/worktree target shapes for mutations.
2. IDs are random, process-local, retired when resources disappear, and never reused when the same path is recreated.
3. Responses expose repository-relative paths and FNV-based `repository_id` without directly serializing canonical/absolute roots; the contract does not misrepresent the hash as confidential.
4. Create uses only an existing eligible `change_id`, current managed base HEAD, and server-derived branch/path; existing worktrees conflict deterministically.
5. Dirty detection failure returns `dirty: null` and blocks deletion.
6. Remote delete always executes managed teardown and offers no teardown bypass or unsafe recovery parameter.
7. Remote merge uses TUI-equivalent base merge, lock, `on_merged` hook, state, and event semantics.
8. Merge conflicts preserve repository intermediate state, return conflict files/evidence, identify local/TUI recovery, and do not automatically abort.
9. Create/delete/merge inherit v2 authentication, required idempotency/revision, capacity, typed error, and busy/lifecycle guards.
10. V2 does not expose arbitrary external-editor commands, configured `worktree_command`, temporary sessions, UI preferences, remote resolve, or remote abort.

## Explicit Completion Conditions

- A shared worktree operation service is used by both TUI and v2 adapters.
- V2 schema and generated OpenAPI include both exact read routes and the three fully specified command target/parameter shapes, with no absolute path, branch, base commit, or generic command input.
- Fast repository-local tests cover non-empty test discovery, identity allocation/retirement, direct path redaction, repository hash caveat, create eligibility/existing conflict, unknown dirty fail-closed behavior, target lookup, inherited idempotency/revision guards, local-recovery capability, and adapter parity.
- Heavy tests use real Git repositories/worktrees for current-HEAD create, mandatory teardown delete, dirty refusal, conflict-preserving merge, successful merge plus `on_merged`, root-busy behavior, and identity recreation; they remain behind the existing `heavy-tests` feature and are proven non-empty before execution.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, generated OpenAPI checks, and strict OpenSpec validation pass after both non-empty test gates pass.

## Dependencies

Consumes the v2 closed command envelope, auth, all-command revision/idempotency contract, command registry, typed errors, resource DTO conventions, and event transport from `add-instance-remote-control-api`.

## Out of Scope

- Arbitrary commands in worktrees or launching editors.
- Temporary session worktrees.
- Client-selected base commits, paths, or branch names.
- Teardown bypasses, force deletion, or unsafe recovery permissions.
- Automatic merge abort, remote resolve/abort, or AI conflict resolution from this endpoint.
- Cross-process stable worktree IDs or confidential repository correlation IDs.

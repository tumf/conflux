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
    requirement: Opaque worktree identity, safe list/create/delete/merge behavior, conflict preservation, and shared TUI/API operations are covered by repository-local tests.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/worktree_ops.rs
    evidence: cargo test output for remote_worktree cases
    rerun: cargo test remote_worktree
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-worktree-heavy
    requirement: Real Git repositories and worktrees verify teardown, dirty guards, merge conflicts, hook ordering, and identity retirement outside the default suite.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/e2e_git_worktree_tests.rs
    evidence: heavy-tests output for remote_worktree cases
    rerun: cargo test --features heavy-tests --test e2e_git_worktree_tests remote_worktree
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Change: secure remote worktree operations

**Change Type**: implementation

## Problem / Context

Legacy web worktree operations expose frontend-shaped inputs, including paths and a general worktree command. A remote-control client needs stable resource identities and must not gain arbitrary command execution, teardown bypasses, or path disclosure.

This change extends the `/api/v2` resources from `add-instance-remote-control-api`. It reuses the same worktree operation service as TUI so create, delete, merge, hooks, and events cannot diverge.

## Proposed Solution

Add `/api/v2/worktrees` read resources and `create_worktree`, `delete_worktree`, and `merge_worktree` command variants.

Each observed worktree receives a process-local opaque `worktree_id`: a random 128-bit hexadecimal value allocated on first observation. Mutation targets accept only this ID. When the resource disappears, its ID is retired; recreating a worktree at the same path receives a new ID.

Responses expose `repository_id` as a 16-character hexadecimal FNV-1a 64-bit hash over the canonical repository identity already used by `repository_identity(repo_root)`. They never expose canonical/absolute repository roots. Worktree paths are repository-relative display values.

Deletion is fail-closed: unknown dirty state is represented as `dirty: null` and is not deletable; managed teardown is mandatory; `skip_teardown`, arbitrary recovery bypasses, path targets, branch targets, and generic worktree commands are not part of v2.

Merge uses the same base-merge operation, `on_merged` hook, reducer/event path, and repository locking as TUI. A Git conflict is a failed command with retained intermediate merge state and conflict file evidence; the service does not auto-abort it.

## Acceptance Criteria

1. Clients can list worktrees and use only opaque `worktree_id` values for mutation targets.
2. IDs are random, process-local, retired when resources disappear, and never reused when the same path is recreated.
3. Responses expose repository-relative paths and stable-within-repository hashed `repository_id`, never canonical/absolute root paths.
4. Dirty detection failure returns `dirty: null` and blocks deletion.
5. Remote delete always executes managed teardown and offers no teardown bypass or unsafe recovery parameter.
6. Remote merge uses TUI-equivalent base merge, lock, `on_merged` hook, state, and event semantics.
7. Merge conflicts preserve repository intermediate state and return conflict files/evidence without automatic abort.
8. Create/delete/merge require v2 authentication, idempotency keys, expected revisions for delete/merge, and existing busy/lifecycle guards.
9. V2 does not expose arbitrary external-editor commands, configured `worktree_command`, temporary sessions, or UI preferences.

## Explicit Completion Conditions

- A shared worktree operation service is used by both TUI and v2 adapters.
- V2 schema and generated OpenAPI include list/detail and the three permitted worktree commands, with no absolute path mutation fields or generic command endpoint.
- Fast repository-local tests cover identity allocation/retirement, path redaction, repository hashing, unknown dirty fail-closed behavior, target lookup, idempotency/revision guards, and adapter parity.
- Heavy tests use real Git repositories/worktrees for create, mandatory teardown delete, dirty refusal, conflict-preserving merge, successful merge plus `on_merged`, and root-busy behavior; they remain outside the default suite.
- `cargo test remote_worktree`, the targeted heavy test command, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, generated OpenAPI checks, and strict OpenSpec validation pass.

## Dependencies

Consumes the v2 command envelope, auth, revision, idempotency, command registry, resource DTO conventions, and event transport from `add-instance-remote-control-api`.

## Out of Scope

- Arbitrary commands in worktrees or launching editors.
- Temporary session worktrees.
- Absolute path or branch-name mutation targets.
- Teardown bypasses, force deletion, or unsafe recovery permissions.
- Automatic merge abort or AI conflict resolution from this endpoint.
- Cross-process stable worktree IDs.

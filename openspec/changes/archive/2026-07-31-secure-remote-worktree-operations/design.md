## Context

Worktree operations mutate Git state and can run repository hooks. Remote access increases the cost of path confusion, stale observations, teardown bypass, and accidental arbitrary command execution. The API exposes the smallest safe operation set.

## Goals / Non-Goals

### Goals

- Opaque process-local mutation identity.
- Exact list/detail routes and closed command shapes.
- Redacted direct repository/worktree observations.
- One TUI/API operation implementation.
- Fail-closed deletion with mandatory teardown.
- Conflict-preserving base merge with explicit local recovery boundary.

### Non-Goals

- General remote shell or editor launch.
- Client-selected branch/path/base commit.
- Recovery bypass or remote merge-resolution switches.
- Stable identity across restarts.
- Confidential hash-based repository identity.

## Decisions

### Decision: routes and command targets are closed

Read routes are `GET /api/v2/worktrees` and `GET /api/v2/worktrees/{worktree_id}`.

Worktree commands extend the v2 enum as follows:

```json
{"type":"create_worktree","target":{"change_id":"my-change"},"params":{},"expected_revision":12,"idempotency_key":"..."}
{"type":"delete_worktree","target":{"worktree_id":"..."},"params":{},"expected_revision":12,"idempotency_key":"..."}
{"type":"merge_worktree","target":{"worktree_id":"..."},"params":{},"expected_revision":12,"idempotency_key":"..."}
```

Unknown target/parameter fields fail schema validation. Create validates that the change exists, is managed, is not archived, is eligible for a change worktree, and has no current worktree. It derives branch/path and uses current managed base HEAD. Existing worktree returns `409 worktree_exists`.

### Decision: mutation identity is allocated, not derived from paths

A registry maps current observed worktree resources to random IDs. The registry may internally bind canonical identity and filesystem facts, but external DTOs and delete/merge targets use only the opaque ID. Removal retires the binding. This prevents stale IDs from targeting a newly created resource at the same path.

### Decision: repository ID is correlation, not confidentiality

`repository_id` applies the existing stable FNV-1a 64-bit algorithm to canonical repository identity and serializes 16 lowercase hexadecimal characters. It allows authenticated clients to group resources without direct absolute-root serialization. It is not accepted as authorization or mutation identity and does not promise resistance to dictionary inference of likely paths.

### Decision: dirty uncertainty denies deletion

Dirty detection has three states: true, false, unknown. Unknown serializes as `null`. Delete eligibility requires known false plus all existing managed-worktree guards. Observation failure cannot become permission.

### Decision: delete has no remote bypass

Remote deletion always runs teardown and uses normal branch/worktree cleanup. Existing local recovery-only `skip_teardown` capability is not represented in v2 DTOs. A client cannot request force removal or supply a filesystem path.

### Decision: merge preserves Git state and requires local recovery on conflict

The operation obtains the same root/base lock and performs the same merge used by TUI. Success executes `on_merged` exactly once and emits shared state/events. A conflict returns a failed command record with conflict file evidence while leaving `MERGE_HEAD` and index state available.

V2 capabilities and the failed command explicitly report `recovery: local_or_tui_required`. No remote resolve/abort command is added. Incompatible later mutations return `root_busy` until an existing local/TUI recovery flow resolves or aborts the merge.

## Operation Flow

```text
validate closed command shape and expected revision
  -> resolve change_id or opaque worktree_id
  -> validate current resource and repository facts
  -> acquire existing root/base mutation guard
  -> execute shared create/delete/merge operation
  -> refresh registry and reducer snapshot
  -> emit shared event and complete command record
```

For create, identity allocation occurs only after successful creation and refresh. For delete, teardown must succeed before resource retirement. For merge conflict, resource refresh reports conflict state while the command is failed.

## Error Extensions

This change extends typed v2 errors with:

- `worktree_exists`
- `worktree_not_found`
- `worktree_dirty`
- `worktree_dirty_unknown`
- `merge_conflict`

Conflict responses include current revision; merge conflict additionally includes repository-relative conflict files and local/TUI recovery guidance.

## Risks / Trade-offs

- IDs change on restart. Clients already resynchronize on `instance_id`, so this prevents stale targeting.
- FNV-1a is not collision- or inference-resistant. It is only an authenticated correlation ID; opaque worktree ID plus current registry binding controls mutation.
- Preserving conflicts leaves the root busy until resolved locally. Auto-abort would violate existing TUI semantics and erase useful evidence.

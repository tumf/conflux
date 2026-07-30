## Context

Worktree operations mutate Git state and can run repository hooks. Remote access increases the cost of path confusion, stale observations, teardown bypass, and accidental arbitrary command execution. The API should expose the smallest safe operation set.

## Goals / Non-Goals

### Goals

- Opaque process-local mutation identity.
- Redacted repository/worktree observations.
- One TUI/API operation implementation.
- Fail-closed deletion with mandatory teardown.
- Conflict-preserving base merge.

### Non-Goals

- General remote shell or editor launch.
- Recovery bypass switches.
- Stable identity across restarts.
- Automated conflict resolution.

## Decisions

### Decision: mutation identity is allocated, not derived from paths

A registry maps current observed worktree resources to random IDs. The registry may internally bind canonical identity and filesystem facts, but external DTOs and command targets use only the opaque ID. Removal retires the binding. This prevents stale IDs from targeting a newly created resource at the same path.

### Decision: repository identity is non-secret correlation, not access authority

`repository_id` applies the existing stable FNV-1a 64-bit algorithm to the canonical repository identity and serializes 16 lowercase hexadecimal characters. It allows clients to group resources without exposing the root. It is not accepted as an authorization or mutation target.

### Decision: dirty uncertainty denies deletion

Dirty detection has three states: true, false, unknown. Unknown serializes as `null`. Delete eligibility requires known false plus all existing managed-worktree guards. Observation failure cannot become permission.

### Decision: delete has no remote bypass

Remote deletion always runs teardown and uses normal branch/worktree cleanup. Existing local recovery-only `skip_teardown` capability is not represented in v2 DTOs. A client cannot request force removal or supply a filesystem path.

### Decision: merge preserves Git's recoverable intermediate state

The operation obtains the same root/base lock and performs the same merge used by TUI. Success executes `on_merged` exactly once and emits shared state/events. A conflict returns a failed command record with conflict file evidence while leaving `MERGE_HEAD` and index state available for existing resolution workflows. No automatic abort hides or discards evidence.

## Operation Flow

```text
resolve opaque worktree_id at expected revision
  -> validate current resource and repository identity
  -> acquire existing root/base mutation guard
  -> execute shared create/delete/merge operation
  -> refresh registry and reducer snapshot
  -> emit shared event and complete command record
```

For delete, teardown is part of the operation and must succeed before resource retirement. For merge conflict, resource refresh reports conflict state while the command is failed.

## Risks / Trade-offs

- IDs change on restart. Clients already resynchronize on `instance_id`, so this prevents stale targeting.
- FNV-1a is not collision-resistant. It is only a display/correlation ID; opaque worktree ID plus current registry binding controls mutation.
- Preserving conflicts leaves the root busy until resolved. Auto-abort would violate existing TUI semantics and erase useful evidence.

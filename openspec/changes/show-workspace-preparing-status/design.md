## Context

Parallel dispatch currently crosses a long unobservable boundary: a change is selected, but `GitWorkspaceManager::create_worktree` does not return until both `git worktree add` and `.wt/setup` finish. `ApplyStarted` is therefore correctly delayed, while the reducer has no intermediate active activity and continues to render queue intent.

## Goals

- Represent workspace preparation truthfully as active work before an operation agent starts.
- Keep all frontends consistent by deriving the token from the shared reducer.
- Preserve workspace-local restart routing required by `openspec/CONSTITUTION.md`.
- Make slow `.wt/setup` execution visibly alive and diagnosable.

## Non-Goals

- Persisting preparation state.
- Parsing arbitrary setup output into structured progress.
- Moving `ApplyStarted` earlier than the Apply process boundary.
- Redesigning `.wt/setup` execution or caching.

## Decisions

### Introduce `Preparing` instead of reusing `Applying`

`applying` means the Apply operation has begun and may carry an iteration. Worktree setup runs before the operation and can route to a different next phase on resume. A distinct activity avoids false agent-running claims and false iteration data.

### Emit preparation at scheduler admission

The transition must occur after dependency and capacity selection has admitted the change, but before worktree creation/recreation begins. Emitting at queue selection would label changes that are still waiting for capacity as active; emitting after `create_worktree` reproduces the current blind interval.

### Keep preparation ephemeral

`Preparing` is in-memory orchestration/UI state only. A restart observes workspace and Git evidence and chooses the next operation without consulting a persisted preparation marker. This satisfies the constitutional workspace-local workflow-state rule.

### Use one reducer-owned status token

TUI, WebUI, and `/api/v2` consume the same reducer transition. Adapters may choose presentation color or spinner behavior, but must not independently infer preparation from logs or filesystem timing.

### Treat preparation as active for safety

Once admitted, preparation can mutate a managed worktree and run project code. Stop/dequeue and deletion logic must therefore classify it as active, request cancellation through the owned execution path, and avoid concurrent destructive mutation.

### Keep setup telemetry bounded

Emit one start diagnostic and one completion or failure diagnostic per setup invocation. Completion includes elapsed duration. Raw command output may continue through existing channels, but repeated polling must not create duplicate lifecycle transitions or logs.

## State Transitions

```text
queued
  → preparing
      → applying[:iteration]
      → accepting[:iteration]
      → rejecting
      → archiving[:iteration]
      → resolving
      → error
      → stopped/not queued after confirmed cancellation
```

The selected next phase remains determined by workspace and Git evidence after preparation.

## Failure Handling

- Worktree-add failure transitions the change from `preparing` to `error` with a worktree-creation diagnostic.
- `.wt/setup` non-zero exit transitions to `error` with command failure context.
- Cancellation must terminate or confirm termination of the owned preparation task before dequeue state is applied.
- A late completion event after terminal error or confirmed stop must not resurrect active preparation.

## Verification Strategy

Use a controlled setup test double or temporary script that blocks on a synchronization primitive. While blocked, inspect reducer, TUI, Web, and API projections and assert `preparing`. Release it and assert the repository-derived next phase. Separate tests cover non-zero setup exit, cancellation, resumed non-Apply routing, event idempotence, and OpenAPI status contract generation.

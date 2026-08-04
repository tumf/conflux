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

### Emit preparation at the admitted workspace boundary

The transition must occur after the parallel slot permit is acquired and stop/terminal gates pass, immediately before force-recreate cleanup or worktree creation/recreation begins. Emitting once for every selected candidate would label changes waiting behind an earlier slow setup as active; emitting after `create_worktree` reproduces the current blind interval.

### Keep preparation ephemeral

`Preparing` is in-memory orchestration/UI state only. A restart observes workspace and Git evidence and chooses the next operation without consulting a persisted preparation marker. This satisfies the constitutional workspace-local workflow-state rule.

### Use one reducer-owned status token

TUI, WebUI, and `/api/v2` consume the same reducer transition. Adapters may choose presentation color or spinner behavior, but must not independently infer preparation from logs or filesystem timing.

### Treat preparation as active for safety

Once admitted, preparation can mutate a managed worktree and run project code. Stop/dequeue and deletion logic therefore classify it as active and avoid concurrent destructive mutation. The current inline preparation path has no termination handle until setup returns, so an immediate dequeue request is refused while its stop mark remains recorded; after preparation returns, that mark must stop execution before an operation agent starts. This change does not make setup itself killable.

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

The selected next phase remains determined by workspace and Git evidence after preparation. Preparation includes any existing operation-stagger wait after setup and before the next `*Started` event; it still truthfully means no operation agent has started.

## Failure Handling

- Worktree-add failure transitions the change from `preparing` to `error` with a worktree-creation diagnostic.
- `.wt/setup` non-zero exit transitions to `error` with command failure context.
- A stop/dequeue request during inline preparation is refused when no termination handle exists, preserves the stop mark, and prevents operation-agent startup after preparation returns.
- Every path that leaves dispatch after emitting preparation but before a next-phase event, including global cancellation and pre-spawn early return, emits a reducer-visible clearing or terminal transition so `preparing` cannot remain stale.
- A late completion event after terminal error or confirmed stop must not resurrect active preparation.

## Verification Strategy

Use a controlled setup test double or temporary script that blocks on a synchronization primitive. While blocked, inspect reducer, TUI, Web, and API projections and assert `preparing`. Release it and assert the repository-derived next phase. Separate tests cover non-zero setup exit, cancellation, resumed non-Apply routing, event idempotence, and OpenAPI status contract generation.

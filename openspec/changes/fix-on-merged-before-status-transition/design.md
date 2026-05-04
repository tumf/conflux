# Design: on_merged as a real merged-transition gate

## Premise

The current code orders `on_merged` before `MergeCompleted`, but not before merged success semantics. Because hook errors are downgraded to warnings in parallel merge success paths, Conflux can report `merged` even when a repo-mutating hook like `make bump-patch` failed and left partial release artifacts behind.

That is the wrong contract. If `continue_on_failure=false`, `on_merged` is part of the success path, not optional decoration.

## Desired behavior

For `continue_on_failure=false`:

1. Repository-visible merge succeeds.
2. Conflux verifies the root repo is safe enough to run the repo-mutating hook.
3. `on_merged` executes.
4. Only if the hook succeeds may Conflux emit `MergeCompleted` and transition to terminal `Merged`.

If step 3 fails, the change must not become merged in reducer/TUI/Web state.

## Affected paths

The proposal must cover every current parallel merged-success path that invokes `on_merged`:

- immediate post-archive merge success in `src/parallel/merge.rs`
- deferred merge retry success in `src/parallel/queue_state.rs`
- manual resolve success paths that share the same merged transition contract

Any path that currently does:

- run hook
- warn on error
- still send `MergeCompleted`

must be rewritten.

## Failure model

### Non-continuable hook failure

When `continue_on_failure=false` and `on_merged` fails:

- do not emit `MergeCompleted`
- do not mark reducer terminal state as `Merged`
- surface a visible operator-facing failure state
- preserve enough context that the operator can inspect/fix/retry safely

### Continuable hook failure

When `continue_on_failure=true`, the current semantics may continue to allow merged transition after the hook attempt. This proposal does not change that policy unless implementation work shows the code path cannot distinguish the two safely.

## Lock and write-safety diagnostics

The existing hook runner waits only for root `.git/index.lock` and proceeds even after timeout. That is not enough for a repo-mutating hook that immediately runs `cargo release` and Git writes.

Minimum improvement in this proposal:

- log whether `.git/index.lock` existed before waiting
- log whether waiting ended by release or timeout
- log that execution is proceeding after timeout, if it does
- surface the exact repo-mutating precondition failure in a way tests can assert

This is an observability and gating improvement, not a new durable workflow-control state.

## Alternatives considered

### Keep warning-only behavior

Rejected. This is how merged rows ended up with uncommitted release artifacts in `main`.

### Move `on_merged` after `MergeCompleted`

Rejected. Existing hook spec requires `on_merged` before merged status transition.

### Add hidden lock-owner state outside the repo

Rejected by constitution. Diagnostics are fine, but workflow control cannot depend on hidden durable external state.

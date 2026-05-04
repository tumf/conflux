# Design

## Overview

The issue is not that `merge wait` flickers visually. The deeper bug is that a parallel archived change can become stable `merge wait` even when nothing is occupying the merge/resolve lane. In that no-blocker case, Conflux should try to merge immediately.

`merge wait` is only correct when there is a concrete reason merge cannot proceed automatically, for example:

- base workspace is dirty and requires user action
- archive completion cannot be verified
- user explicitly requested retry from an existing manual merge wait

Without such a reason, `merge wait` incorrectly asks the user or scheduler to wait for something that does not exist.

## Existing Relevant Semantics

`openspec/specs/orchestration-state/spec.md` already states:

- active `Resolving` or active `Rejecting` are the only automatic retry blocker lane occupants
- no active blocker should start the immediate merge path
- applying/accepting/archiving/queued/blocked/terminal states must not create automatic `resolve pending`

The implementation must make the runtime path match that spec, not weaken the spec to fit the observed vibration.

## Target Flow

### No blocker

1. Change B finishes archive in parallel mode.
2. Reducer records archive completion as a transient pre-merge fact.
3. Post-archive dispatch sees no other active `Resolving` or `Rejecting` change.
4. Scheduler/orchestrator immediately attempts merge for B.
5. If merge succeeds, B becomes `merged`.
6. If merge returns `MergeDeferred(auto_resumable=false)`, B becomes `merge wait` with the deferral reason.

### Active resolving/rejecting blocker

1. Change B finishes archive in parallel mode.
2. Change A is actively `Resolving` or `Rejecting`.
3. B enters auto-resumable retry intent (`resolve pending`).
4. When the blocker clears, scheduler retries merge.

### Manual deferral

1. Immediate merge attempt detects a real manual blocker, such as dirty base workspace.
2. `MergeDeferred(auto_resumable=false)` is emitted.
3. Reducer sets `MergeWait`, clears normal queue intent, and removes resolve-wait membership.
4. User resolves the blocker and explicitly retries with `M` / `ResolveMerge`.

## Design Constraints

- Do not introduce durable state under `~/.local/state/cflx/**` as workflow control input.
- Do not treat UI display state as authoritative for next-action routing.
- Do not auto-clean, stash, or commit base workspace changes.
- Keep merge deferral reasons observable in logs/events so `merge wait` is explainable.

## Rejected Alternatives

### Keep `merge wait` as the default post-archive state

Rejected. It hides a scheduler/orchestrator dispatch miss behind a user-visible wait state. If no merge blocker exists, there is nothing meaningful to wait for.

### Auto-resume every `merge wait`

Rejected. Manual dirty-base deferrals must not become a busy retry loop. Only auto-resumable conditions belong in `resolve pending`.

### Fix only TUI display wording

Rejected. The user-observed vibration is a symptom, but the incorrect stable state is the no-blocker archive path becoming `merge wait` before a real merge attempt.

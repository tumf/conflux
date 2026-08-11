# Design: stale resolve settlement

## State ownership

Base-branch tree comparison is the authoritative completion evidence. Scheduler-local retry sets and lane occupancy are ephemeral coordination state; they may be cleared only after the shared reducer records the outcome established by repository evidence.

## Settlement table

| Evidence | Reducer result | Scheduler cleanup |
| --- | --- | --- |
| Base integration proven | terminal `merged` | clear resolve retry and release lane |
| Base integration absent | manual `merge wait` | clear the consumed reservation and release lane |
| Evidence read unknown/error | manual `merge wait` | fail closed, clear only consumed ephemeral ownership |
| Bounded resolve exhaustion | `ResolveFailed` to `merge wait` | existing change-scoped cleanup |

## Decision

Use a typed reducer transition at the stale-retry boundary instead of teaching `clear_resolve_wait_intent` to infer success. That helper is also used by non-success outcomes, so broadening it would risk marking failures as merged.

The scheduler must commit reducer settlement first and remove its local bookkeeping second. This ordering prevents a refresh from observing an idle `NotQueued` gap between accepted retry intent and terminal/retryable settlement.

## Dirty-state safety

Dirty state is a guard and an observation, not work to normalize. Settlement reads repository evidence but performs no mutation of the index, worktree, or unrelated files. Tests must verify both lifecycle status and unchanged repository content.

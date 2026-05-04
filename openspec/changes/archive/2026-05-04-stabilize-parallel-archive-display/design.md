# Design

## Overview

The bug is broader than a visual flicker. Post-archive parallel lifecycle state lacks a clean single decision table. The same archived workspace can appear as `archived`, `merge wait`, and later `merged` even when there is no active `resolving` state visible to justify the transition.

After archive completes in parallel mode, Conflux must choose one of three paths based on current facts:

| Condition | Expected visible state | Meaning |
| --- | --- | --- |
| Another merge/resolve lane is active | `resolve pending` | This change will retry automatically after the lane clears |
| Merge cannot proceed due to dirty/manual blocker | `merge wait` | User action is required before explicit retry |
| No blocker | `resolving` → `merged` | Conflux is actively merging now |

`archived` is terminal only in serial mode. In parallel mode, archive completion is a milestone before merge routing, not a stable end-user state once post-archive handling starts.

## Existing Relevant Semantics

`openspec/specs/orchestration-state/spec.md` already says:

- active `Resolving` or active `Rejecting` are automatic retry blocker lane occupants
- no active blocker should start the immediate merge path
- applying/accepting/archiving/queued/blocked/terminal states must not create automatic `resolve pending`

This proposal extends that with two missing pieces:

- no-blocker immediate merge should be visible as active `resolving`
- final `merged` must dominate later archive/refresh observations

## Target Flow

### 1. Lane occupied → resolve pending

1. Change B finishes archive in parallel mode.
2. Change A is actively merging/resolving, or otherwise in the defined automatic retry blocker lane.
3. B enters reducer-owned `ResolveWait`.
4. TUI displays `resolve pending`.
5. Scheduler retries B when the lane clears.

### 2. Manual blocker → merge wait

1. Change B finishes archive in parallel mode.
2. Conflux attempts or verifies merge readiness.
3. Merge readiness finds a concrete manual blocker, such as dirty base workspace or incomplete archive evidence.
4. `MergeDeferred(auto_resumable=false)` is emitted with a reason.
5. Reducer sets `MergeWait`, clears normal queue intent, and removes resolve-wait membership.
6. TUI displays `merge wait` and keeps the explicit retry affordance.

### 3. No blocker → resolving → merged

1. Change B finishes archive in parallel mode.
2. No merge/resolve lane blocker exists.
3. Merge readiness does not find a manual blocker.
4. Conflux emits/applies an active merge state visible as `resolving`.
5. Merge completes.
6. Reducer sets terminal `Merged`.
7. Later `ChangeArchived`, `ChangesRefreshed`, worktree observations, or cleanup events cannot regress display from `merged`.

## Vibration Invariants

The implementation should enforce these invariants in tests:

- `merged` is display-terminal. It must not alternate with `archived`.
- `merge wait` cannot be a default no-blocker post-archive state.
- `merge wait -> merged` is valid only if preceded by explicit retry or if the visible sequence records the merge/resolving work that produced `merged`.
- `archived` may be logged as an archive milestone, but in parallel mode it must not be a stable lifecycle display after post-archive routing starts.

## Design Constraints

- Do not introduce durable state under `~/.local/state/cflx/**` as workflow control input.
- Do not treat UI display state as authoritative for next-action routing.
- Do not auto-clean, stash, or commit base workspace changes.
- Keep merge deferral reasons observable in logs/events so `merge wait` is explainable.
- Keep serial mode archive-terminal behavior intact.

## Rejected Alternatives

### Keep `merge wait` as the default post-archive state

Rejected. It hides a missing merge dispatch behind a user-visible wait state. If no merge blocker exists, there is nothing meaningful to wait for.

### Jump directly from archived to merged without resolving

Rejected. It can be technically true for a fast merge, but it makes the lifecycle opaque and makes vibration bugs harder to distinguish from normal work.

### Auto-resume every `merge wait`

Rejected. Manual dirty-base deferrals must not become a busy retry loop. Only auto-resumable conditions belong in `resolve pending`.

### Fix only TUI display wording

Rejected. The observed vibration is a symptom. The lifecycle state machine must encode the correct post-archive transition and dominance rules.

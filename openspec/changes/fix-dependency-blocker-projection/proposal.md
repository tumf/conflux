---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/runtime-state/spec.md
  - src/parallel/queue_state.rs
  - src/orchestration/state.rs
  - src/runtime/snapshot.rs
verifications:
  - id: dependency-projection-tests
    requirement: "Every accepted queued change with unresolved repository dependencies is continuously projected as dependency-blocked with structured blocker details until those dependencies resolve"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/queue_state.rs
    evidence: "Named focused tests `dependency_blocker_projection_initial`, `dependency_blocker_projection_rebuild`, `dependency_blocker_projection_resolution`, `dependency_blocker_projection_capacity_only`, and `dependency_blocker_projection_tui_badge` all pass"
    rerun: "cargo test dependency_blocker_projection_initial && cargo test dependency_blocker_projection_rebuild && cargo test dependency_blocker_projection_resolution && cargo test dependency_blocker_projection_capacity_only && cargo test dependency_blocker_projection_tui_badge"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix dependency blocker projection

**Change Type**: implementation

## Premise / Context

- Queue intent and current dispatch eligibility are separate: a change may remain admitted while unresolved dependencies prevent dispatch.
- The canonical parallel-execution spec already requires dependency-ineligible queued work to display its dependency wait.
- The scheduler currently emits `DependencyBlocked` as a deduplicated transition, while TUI and typed status consume reducer/runtime projection.
- A live project showed three accepted changes as `display_status=queued`, `blocker=null`, and `parallel_eligible=true` while dependency analysis withheld all three behind an active predecessor.
- This is an observability and projection repair. Repository evidence remains the workflow authority.

## Problem / Context

A dependency wait can remain internally known to scheduler analysis while the coherent runtime snapshot loses or never receives the corresponding blocker projection. The row then appears as plain `[queued]`, reports no blocker, and may claim `parallel_eligible=true` despite an unresolved hard dependency. Operators cannot distinguish a real capacity wait from dependency exclusion, and an empty execution slot appears unexplained.

Transition deduplication must suppress repeated diagnostics, not suppress the current state needed to reconstruct a truthful snapshot after refresh, reducer replacement, or a later status observation.

## Proposed Solution

Make dependency classification a continuously reconcilable state projection:

1. During every coherent scheduler classification, publish or reconcile the current unresolved dependency set into reducer/runtime state independently of operator-diagnostic deduplication.
2. Project an admitted unresolved change as `display_status=blocked`, `execution_state=queued`, `queue_intent=queued`, `parallel_eligible=false`, with `blocker.kind=dependency` and the unresolved dependency IDs.
3. Clear that blocker only after repository-visible dependency evidence resolves, returning the retained queue intent to plain `queued` and allowing normal eligibility calculation.
4. Keep duplicate log/event suppression for unchanged blocker fingerprints, but do not use the deduplication store as current-state authority.
5. Ensure TUI and `/api/v2` typed status derive from the same reducer/runtime projection. The TUI badge must render `[blocked:dependency]` for this state.
6. Preserve dispatch behavior, configured concurrency, queue intent, execution marks, and dependency semantics.

## Acceptance Criteria

1. A queued change with one or more unresolved hard dependencies is reported as blocked in the first coherent snapshot after classification.
2. Typed status carries `display_status=blocked`, `execution_state=queued`, `queue_intent=queued`, `parallel_eligible=false`, and a structured dependency blocker naming every current unresolved dependency.
3. The TUI renders `[blocked:dependency]`, not `[queued]`, for that same projection.
4. Unchanged scheduler passes do not spam duplicate operator diagnostics, but every snapshot remains truthful even when no new diagnostic transition is emitted.
5. If runtime/reducer projection is recreated while the dependency remains unresolved, the next classification restores the blocker without requiring a changed fingerprint.
6. When all dependencies become repository-visibly resolved, the blocker is cleared once, the retained queue intent displays as `[queued]`, and the change becomes eligible subject to normal capacity and policy.
7. A genuinely ready queued change waiting only for an occupied execution slot remains `[queued]` with no blocker.
8. Existing dependency dispatch exclusion and maximum-concurrency behavior do not change.

## Explicit Completion Conditions

- Scheduler classification and reducer/runtime projection have a state-reconciliation path separate from diagnostic transition deduplication.
- Focused tests cover initial dependency blocking, unchanged reclassification, projection reconstruction, blocker-set changes, dependency resolution, and a capacity-only queued control case.
- Snapshot/API tests assert the complete structured fields, and TUI rendering tests assert the dependency badge.
- `cflx openspec validate fix-dependency-blocker-projection --archive-gate` passes.

## Out of Scope

- Changing hard dependency semantics or dependency metadata.
- Increasing or dynamically changing `max_parallelism`.
- Adding a new durable state store or using logs/metrics as workflow authority.
- Changing external-prerequisite or acceptance-stall classification.

---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/observability/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/changes/archive/2026-05-02-fix-queue-reconciliation-log-spam/proposal.md
  - src/parallel/queue_state.rs
  - scripts/cflx-log-mine.py
---

# Change: Fix missing candidate log spam

**Change Type**: implementation

## Premise / Context

- Runtime log mining after `.last-checked` found repeated `Queue reconciliation could not load reducer-queued change '<id>': candidate_not_found` warnings, including 569 repeats for `share-user-installed-skills` and 541 repeats for `add-agent-skill-inventory`.
- The bundled `scripts/cflx-log-mine.py --top 30` classified the repeated warnings as top actionable groups, not manual resolve/merge retry events.
- Existing canonical specs already allow/require bounded scheduler diagnostics in TUI-visible logs, and the archived `fix-queue-reconciliation-log-spam` proposal explicitly intended `candidate_not_found` to remain observable but deduplicated or rate-limited.
- Current `src/parallel/queue_state.rs` routes the TUI-visible diagnostic through a dedupe helper, but still emits an unconditional `tracing::warn!` on each `candidate_not_found` observation before the deduped user-visible event.
- `openspec/CONSTITUTION.md` permits observability-only suppression state, but it must not become workflow-control input.

## Requested Artifact

Implementation proposal to align the file/debug log path with the existing bounded scheduler diagnostic contract without changing queue reconciliation decisions.

## Problem

The scheduler already deduplicates the user-visible queue reconciliation diagnostic for missing reducer-queued candidates, but the same state still emits an unconditional structured warning on every scheduler loop. Because TUI logs are mirrored to debug log files and operators mine those files for regressions, this creates hundreds of duplicate warnings for a stable stale reducer-queued intent.

The repeated warning does not represent new scheduler state after the first observation. It hides more useful failures and makes log-mining results look worse than the underlying workflow behavior.

## Proposed Solution

Route missing-candidate scheduler diagnostics through one bounded observability path for both user-visible events and structured file/debug logs:

1. Use the existing `(change_id, reason)` diagnostic suppression state, or an equivalent in-memory helper, before emitting the structured `candidate_not_found` warning.
2. Preserve the first observable warning/event for each missing candidate so operators can see why queued intent was not reconciled.
3. Optionally emit repeated observations at `debug!` level or through a bounded summary/rate-limited message, as long as they do not flood WARN/TUI-visible log-mining results.
4. Keep the scheduler decision unchanged: missing candidates are not added to scheduler-local queued work, and loadable queued changes still reconcile normally.
5. Keep suppression state runtime-ephemeral and observability-only.

## Acceptance Criteria

1. Repeated reconciliation of the same `(change_id, candidate_not_found)` does not emit an unbounded sequence of `WARN cflx::parallel::queue_state: Queue reconciliation could not load reducer-queued change ... candidate_not_found` entries.
2. The first missing-candidate observation remains visible through a warning or user-visible log/event.
3. Missing-candidate suppression does not change queue insertion, active/in-flight filtering, resolve-wait retry routing, archive, or merge behavior.
4. Existing bounded diagnostic behavior for `already_active` and other queue reconciliation reasons continues to pass.
5. Tests or focused verification prove both the bounded log behavior and unchanged scheduler reconciliation semantics.

## Explicit Completion Conditions

- `src/parallel/queue_state.rs` no longer emits unconditional repeated WARN-level structured logs for identical missing reducer-queued candidates.
- The implementation uses only in-memory observability state for suppression/rate limiting and does not write durable workflow-control state.
- A regression test exercises repeated missing-candidate reconciliation and fails if duplicate user-visible or structured warning emissions are unbounded.
- Existing scheduler reconciliation tests still prove that loadable reducer-queued changes are added and missing candidates are not added.
- `cflx openspec validate fix-missing-candidate-log-spam --strict --evidence warn` passes.

## Out of Scope

- Changing reducer queue intent semantics.
- Clearing stale reducer-owned queue intent solely because the OpenSpec candidate is missing.
- Changing manual `ResolveMerge` behavior, merge-wait retry routing, archive verification, or dependency analysis.
- Changing `scripts/cflx-log-mine.py` classification rules.

---
change_type: implementation
priority: high
dependencies: []
references:
  - src/config/types.rs
  - src/config/defaults.rs
  - src/agent/runner.rs
  - src/execution/apply.rs
  - src/stall.rs
  - openspec/specs/configuration/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Change: Add apply stall escalation and diagnosis

**Change Type**: implementation

## Premise / Context

- The current apply loop stalls after a configurable number of consecutive empty WIP commits and returns an apply error.
- Session investigation showed a realistic near-miss case where a change reached `5/6` tasks complete and then repeated empty WIP snapshots until the fixed threshold (`5`) fired.
- The current runtime can only keep retrying the same `apply_command`; it cannot escalate to a stronger command profile or run a dedicated diagnosis command before declaring stall.
- Conflux configuration already models stage-specific command templates as top-level command keys, while stall policy lives under `stall_detection`.
- The Conflux Constitution requires workflow-control decisions to remain derivable from workspace/git/base-tree evidence; any escalation bookkeeping must therefore remain runtime-ephemeral and non-authoritative.

## Requested Artifact

- implementation proposal to add a configurable apply escalation command and a dedicated stall diagnosis command before empty-WIP stall is finalized
- preserve current command-template architecture and strict config validation behavior
- keep escalation/diagnosis as observability and execution policy only, not durable workflow-control state

## Problem

When apply retries keep producing empty WIP commits, the runtime currently has only two options: keep issuing the same `apply_command` or stop once the empty-WIP threshold is reached. This makes near-complete changes brittle: a change that might succeed with a stronger model / higher-think command profile instead terminates after repeated no-op retries, and operators get little structured evidence about why the final task stayed incomplete.

## Proposed Solution

Add an adaptive pre-stall recovery path for empty-WIP apply retries:

1. Introduce an optional top-level `apply_escalation_command` that acts as a drop-in replacement for `apply_command` during late-stage empty-WIP retries.
2. Extend `stall_detection` with configurable escalation policy knobs so operators can say, for example, “after 3 consecutive empty WIP commits, use escalation for the remaining 2 retries before stall.”
3. Introduce an optional top-level `apply_stall_diagnose_command` that runs once after escalation opportunities are exhausted and immediately before final stall classification.
4. Preserve the current final stall outcome semantics for now: escalation and diagnosis happen before the existing stall decision, but this change does not reclassify empty-WIP stall into a different lifecycle state by itself.
5. Keep escalation counters runtime-ephemeral; they may influence the current run's retry policy, but they must not become durable workflow-control inputs for resume routing.

## Acceptance Criteria

- Operators can configure an optional `apply_escalation_command` and an optional `apply_stall_diagnose_command` without breaking existing command-template behavior.
- Operators can configure the empty-WIP retry point at which escalation begins and how many escalation attempts may be used before stall finalization.
- When consecutive empty WIP commits reach the configured escalation boundary, the runtime switches subsequent apply retries to `apply_escalation_command` instead of the base `apply_command`.
- When escalation attempts are exhausted and the empty-WIP threshold is reached, the runtime executes `apply_stall_diagnose_command` exactly once before returning the final stall error/outcome.
- Diagnosis failure never hides or replaces the underlying stall reason; the final stall outcome still reports the empty-WIP stall as the primary failure.
- If escalation/diagnosis commands are unset, the runtime preserves current behavior.
- The change does not introduce out-of-worktree durable workflow-control state and remains consistent with `openspec/CONSTITUTION.md`.

## Explicit Completion Conditions

- `src/config/types.rs` and related config validation/defaults accept the new command keys and escalation policy knobs, including validation that escalation can only begin before the final stall threshold.
- `src/execution/apply.rs` (or the canonical apply-loop owner) records consecutive empty-WIP counts, swaps to `apply_escalation_command` at the configured boundary, and caps escalation usage per stall sequence.
- The runtime executes `apply_stall_diagnose_command` once immediately before final empty-WIP stall classification and records the diagnostic result as follow-up evidence/logging without overwriting the primary stall cause.
- Regression tests prove: default behavior is unchanged when new config is absent; escalation replaces apply on the configured late retries; diagnosis runs once on final stall; diagnosis failure still preserves the original stall outcome.
- `cflx openspec validate add-apply-stall-escalation --strict --evidence warn` passes.

## Out of Scope

- Changing the final lifecycle classification of empty-WIP stall (for example, converting it from error to `stalled`).
- Introducing provider-specific runtime flags such as “raise think level” inside Conflux itself; command templates remain user-controlled.
- Adding durable recovery state outside workspace/git/base-tree evidence.

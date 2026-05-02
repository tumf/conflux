# Design: Apply Stall Escalation and Diagnosis

## Context

The current apply loop uses a single `apply_command` across all retries and relies on empty-WIP stall detection to stop repeated no-op iterations. This is simple, but it cannot distinguish between:

- a change that is truly stuck and should stop quickly
- a change that is nearly complete and may succeed with a stronger model / higher-cost command profile

The requested behavior adds two policy stages before final stall:

1. **Escalation** — replace `apply_command` with `apply_escalation_command` for a bounded number of retries late in the empty-WIP streak.
2. **Diagnosis** — run `apply_stall_diagnose_command` once immediately before finalizing the existing stall outcome.

## Design Goals

- Preserve the existing command-template architecture: Conflux chooses *which* command template to run, while users choose the actual provider/model flags inside those templates.
- Keep workflow-control state constitutional: escalation counters are runtime-ephemeral only and must not become durable resume-routing inputs.
- Keep stall as the primary failure reason; diagnosis is supplemental evidence, not a new controlling state.
- Avoid infinite escalation/diagnose loops.

## Configuration Shape

Top-level optional command templates:

- `apply_escalation_command`
- `apply_stall_diagnose_command`

Extended stall policy under `stall_detection`:

- `enabled`
- `threshold`
- `apply_escalation_after_empty_wip`
- `apply_escalation_max_uses_per_stall`

Validation rules:

- if `apply_escalation_after_empty_wip` is set, it MUST be `< threshold`
- if `apply_escalation_max_uses_per_stall` is set, it MUST be `>= 1`
- `apply_escalation_command` is fully optional; if absent, escalation policy becomes a silent no-op and the runtime keeps using `apply_command`
- `apply_stall_diagnose_command` is fully optional; if absent, final stall follows the current path with no extra warning

## Runtime Flow

For each change/run, the apply loop already tracks empty-WIP commits. Extend that flow:

1. run normal `apply_command`
2. if a non-empty WIP commit occurs, reset the empty-WIP streak and escalation usage counter
3. if the empty-WIP streak reaches `apply_escalation_after_empty_wip` and `apply_escalation_command` is configured, switch subsequent eligible retries to `apply_escalation_command`
4. each escalation retry increments `escalation_uses_for_current_stall`
5. once the empty-WIP threshold is reached and no more escalation retries remain, run `apply_stall_diagnose_command` exactly once if it is configured
6. after diagnosis completes (or fails), or immediately if no diagnose command is configured, emit the existing empty-WIP stall outcome

This produces a single bounded sequence:

`apply_command* -> apply_escalation_command* -> diagnose? -> stall`

## History and Prompting

Escalation runs should still be recorded as apply attempts so the next prompt can see the real history. The command template changes, but the apply history remains part of the same change-local context.

Diagnosis runs should have their own attempt/log classification so they do not pollute normal apply history with misleading “apply failed” semantics.

## Evidence Handling

`apply_stall_diagnose_command` should contribute observability only:

- log entries / event stream messages
- optional persisted diagnostic text if the existing architecture already has a canonical place for such evidence

Diagnosis output must not overwrite the root cause string `Stall detected for <change_id> after N empty WIP commits (apply)`.

## Non-Goals

- This design does **not** change the final lifecycle classification of empty-WIP stall.
- It does **not** make Conflux provider-aware about “deep thinking” flags; users encode those details inside command templates.
- It does **not** introduce durable retry/escalation counters outside the current runtime.

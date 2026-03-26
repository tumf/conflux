## Context

v0.5.13 moved display ownership toward the reducer, but the TUI `F5` start/resume path still mutates local `queue_status` optimistically. In parallel mode, the startup refresh and reducer-driven display sync run immediately afterward and can overwrite those local writes before execution has actually started.

The regression is specifically about queued/blocked state ownership, not a full state-machine redesign:

- `F5` creates local `Queued` rows without updating shared reducer queue intent.
- Parallel startup emits `ChangesRefreshed`, and the runner reapplies reducer display state.
- Dependency-blocked rows can lose their queued intent if the reducer and TUI fall out of sync.

## Goals / Non-Goals

- Goals:
  - Make queued/blocked startup state reducer-owned for parallel TUI execution.
  - Preserve queued intent across the first startup refresh and dependency-block transitions.
  - Keep the fix tightly scoped to the reported regression.
- Non-Goals:
  - Reworking unrelated terminal, merge-wait, or resolve-wait precedence.
  - Changing remote/Web API contracts.
  - Replacing the current reducer architecture with a new model.

## Decisions

- Decision: `F5` start/resume/retry must update shared reducer queue intent before any reducer-driven display sync is allowed to run.
  - Rationale: local optimistic `queue_status` is no longer a safe source of truth once refresh and display are reducer-owned.
- Decision: the initial parallel startup refresh must preserve the selected start set as queued until a later execution, wait, terminal, or explicit rejection event changes it.
  - Rationale: startup refresh is observational and must not erase freshly requested execution intent.
- Decision: dependency blocking remains a wait overlay on queued intent, and dependency resolution reveals the preserved queued state automatically.
  - Rationale: the user should not need to manually requeue work that was already selected for execution.
- Decision: rejection/reset paths must be targeted by change ID rather than broad queued/blocked cleanup.
  - Rationale: stale eligibility or one rejected row must not clear neighboring queued or blocked rows.

## Alternatives Considered

- Keep local TUI queued writes and suppress refresh synchronization during startup.
  - Rejected because it preserves split ownership and keeps the same race under a different timing rule.
- Revert the reducer-driven display sync entirely.
  - Rejected because the reducer model is already the intended direction and other flows depend on it.

## Risks / Trade-offs

- Risk: command-path changes may temporarily diverge from existing serial behavior if queue intent seeding is implemented only for parallel mode.
  - Mitigation: cover both select-mode start and stopped-mode resume with targeted tests.
- Risk: broad stop/completion cleanup paths may still over-reset queued or blocked rows.
  - Mitigation: add regression tests for rejection-only clearing and dependency-block persistence.

## Migration Plan

1. Update TUI command/start paths so reducer queue intent is set before parallel orchestration begins.
2. Adjust parallel startup/reducer sync so the initial refresh cannot regress selected queued rows.
3. Tighten dependency blocked/resolved and rejection-only reset behavior.
4. Lock the behavior with focused regression tests, then run strict proposal validation.

## Open Questions

- None. The requested fix is scoped and directly grounded in the current regression report.

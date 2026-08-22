---
change_type: implementation
priority: high
dependencies: []
references:
  - src/config/mod.rs
  - src/orchestration/acceptance.rs
  - src/ai_command_runner.rs
  - openspec/specs/command-queue/spec.md
  - openspec/specs/configuration/spec.md
verifications:
  - id: acceptance-runtime-tests
    requirement: Acceptance commands have a distinct bounded absolute runtime and terminate owned process groups without retrying the same timed-out invocation
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/orchestration/acceptance.rs
    evidence: cargo test orchestration::acceptance --lib
    rerun: cargo test orchestration::acceptance --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: acceptance-runtime-config-tests
    requirement: Acceptance runtime configuration defaults, precedence, range, and zero rejection are validated
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/config/mod.rs
    evidence: "cargo test config:: --lib"
    rerun: "cargo test config:: --lib"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Bound Acceptance runtime

**Change Type**: implementation

## Problem / Context

All AI commands currently share `command_max_runtime_secs`, whose default is three hours and may be disabled with zero. Proposal validation reduces accidental heavy gates, but malformed or legacy proposals can still start an Acceptance command that runs for hours. Acceptance needs a shorter fail-closed wall-clock guard independent of Apply's larger implementation budget.

## Proposed Solution

Add `acceptance_max_runtime_secs` as an Acceptance-specific absolute deadline:

- Default: 1800 seconds.
- Valid range: 60 through 10800 seconds.
- Zero is rejected; Acceptance cannot disable its wall-clock guard.
- Configuration follows existing global, project, custom, and CLI construction precedence.
- Acceptance uses the shorter of its dedicated limit and any enabled positive `command_max_runtime_secs`; when the common limit is disabled with zero, the dedicated Acceptance limit still applies. Apply, Archive, analysis, resolution, and other command types retain `command_max_runtime_secs`.
- Expiry closes retry admission for that Acceptance invocation, terminates and reaps the owned process group through the existing cleanup path, and produces a typed actionable Acceptance failure containing the configured limit.
- A timed-out Acceptance invocation is not automatically retried. Operator-triggered recovery remains explicit and repository-derived.

## Acceptance Criteria

- Acceptance defaults to a 30-minute absolute runtime limit even when the common command limit is three hours or disabled.
- A configured valid Acceptance limit is honored, subject to any shorter enabled common safety limit, without altering other command classes.
- Continuous output does not extend the Acceptance deadline.
- Expiry proves process-group quiescence before the workflow proceeds or reports cleanup failure.
- Timeout is visible as a typed Acceptance failure and cannot be mistaken for PASS, external block, inactivity timeout, no-verdict protocol continuation, or corrective command-recovery retry.
- Existing Acceptance success and verdict parsing remain unchanged below the limit.

## Explicit Completion Conditions

- Config tests cover default, precedence, lower/upper bounds, and zero rejection.
- Acceptance runner tests cover success, continuous-output expiry, no same-invocation retry, and cleanup failure diagnostics.
- `cargo test orchestration::acceptance --lib` and `cargo test config:: --lib` pass.

## Out of Scope

- Changing the common three-hour command limit.
- Proposal verification classification.
- Apply evidence reuse.
- Automatically retrying timed-out Acceptance work.

## Verification Ownership

Focused Acceptance module tests own the bounded repository-local proof. Repository-wide checks remain hook-owned.

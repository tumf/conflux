---
change_type: hybrid
priority: high
dependencies: []
references:
  - skills/cflx-accept/SKILL.md
  - skills/cflx-accept-with-speca/SKILL.md
  - src/acceptance.rs
  - src/parallel/executor.rs
  - src/orchestration/acceptance.rs
  - src/parallel/tests/executor.rs
verifications:
  - id: acceptance-completion-regression
    requirement: Acceptance waits for owned verification work and classifies missing verdicts explicitly
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output covering acceptance skill contracts, verdict parsing, and parallel acceptance execution
    rerun: make test
    prerequisites: []
---

# Prevent Premature Acceptance Exit

**Change Type**: hybrid

## Problem / Context

An acceptance agent can start asynchronous or monitored verification, emit a narrative message saying it is waiting for completion, and then exit without receiving the completion result or emitting a canonical acceptance verdict. Conflux currently maps output without a verdict to `CONTINUE`, making a premature agent exit indistinguishable from an intentional continuation verdict and causing opaque acceptance retries.

This violates truthful completion because the acceptance decision is not based on the verification result the agent claimed it would await.

## Proposed Solution

Define acceptance guidance that requires the parent acceptance agent to retain ownership of every verification it starts, wait synchronously for completion notifications or final command results, and emit exactly one final canonical verdict before exiting. A waiting/status narrative is not a valid terminal response.

Update runtime classification so a completed acceptance command with no canonical verdict is recorded and surfaced as an explicit missing-verdict protocol failure rather than an intentional `CONTINUE`. Preserve explicit canonical `CONTINUE` behavior and its configured retry policy.

Apply the portable completion rule consistently to the standard and SPECA acceptance skills, their embedded contract checks, and serial/parallel acceptance execution paths.

## Acceptance Criteria

- An acceptance agent that starts verification MUST wait for its final result before producing the final verdict and MUST NOT terminate with only a waiting/status message.
- The standard and SPECA acceptance skills describe the same portable completion-ownership rule without depending on a runtime-specific monitoring tool.
- A successful acceptance command that exits without a canonical verdict is distinguishable from explicit `CONTINUE` in runtime results, history, and operator-visible diagnostics.
- Missing-verdict output does not consume the configured explicit-CONTINUE retry path as though the agent intentionally requested continuation.
- Explicit canonical `CONTINUE`, PASS, FAIL, and stalled-hold verdicts retain their existing semantics.
- Regression coverage reproduces a waiting/status-only acceptance exit and proves that it cannot be reported as an ordinary continuation.

## Explicit Completion Conditions

- `skills/cflx-accept/SKILL.md` and `skills/cflx-accept-with-speca/SKILL.md` require completion of owned verification before final output and prohibit status-only termination.
- Acceptance parsing/execution represents missing canonical verdict separately from `AcceptanceResult::Continue`, with both serial and parallel callers routing it as a protocol/command failure and emitting actionable diagnostics.
- Tests cover explicit `CONTINUE` versus missing verdict, status-only output, and preservation of all canonical verdict outcomes.
- Embedded-skill installation/contract tests prove installed skills retain the completion rule.
- `make test`, `make lint`, and `make typecheck` pass, or the repository's equivalent commands discovered during implementation pass.

## Scope Rationale

Skill guidance and runtime classification ship together because guidance alone cannot diagnose third-party or stale agents, while runtime classification alone would still allow bundled acceptance guidance to terminate prematurely.

## Out of Scope

- Replacing agent runtimes or their tool implementations.
- Adding durable workflow state outside the workspace.
- Changing acceptance retry limits for explicit canonical `CONTINUE`.
- Treating a missing verdict as PASS, FAIL findings about product code, or a terminal rejection.

---
change_type: implementation
priority: high
dependencies:
  - bound-acceptance-runtime
references:
  - src/config/types.rs
  - src/command_queue.rs
  - src/ai_command_runner.rs
  - src/parallel/executor.rs
  - src/parallel/dispatch.rs
  - src/orchestration/acceptance.rs
  - openspec/specs/configuration/spec.md
  - openspec/specs/parallel-execution/spec.md
verifications:
  - id: acceptance-runtime-routing-tests
    requirement: Acceptance runtime expiry is terminal for the run and bypasses command retry, Acceptance retry, and Apply re-entry
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/dispatch.rs
    evidence: "cargo test parallel:: --lib"
    rerun: "cargo test parallel:: --lib"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: acceptance-runtime-config-tests
    requirement: The dedicated Acceptance limit is validated and selected inside the common runner without changing other operation classes
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/config/types.rs
    evidence: "cargo test config:: --lib"
    rerun: "cargo test config:: --lib"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Correct Acceptance runtime routing

**Change Type**: implementation

## Problem / Context

Fable reviewed `bound-acceptance-runtime` after its proposal had already been merged and implementation started. The review found that a per-invocation 1,800-second deadline can become 5,400 seconds through the existing command retry counter. If expiry is routed as an ordinary Acceptance failure, it may also return to Apply and enter another Acceptance cycle.

The limit must terminate the current run through a distinct typed outcome rather than enter any corrective retry path.

## Proposed Solution

- Store `acceptance_max_runtime_secs` in `CommandQueueConfig`; select the effective limit inside the common runner from `operation_type == "acceptance"` without changing the runner API.
- Keep the 1,800-second default. Validate the dedicated key in `300..=10,800`; zero remains invalid. This floor applies to the dedicated key only. A shorter positive common safety limit still wins.
- Treat runtime expiry as a typed terminal Acceptance runtime outcome after process-group cleanup.
- The outcome does not increment `AcceptanceCommandRetryCounter`, set command-recovery context, consume an Acceptance retry/no-verdict cycle, or return to Apply.
- Recovery is operator-explicit. Restart may recompute and rerun Acceptance with a fresh budget because no hidden durable timeout state is introduced.
- Only the normal Acceptance operation receives the dedicated limit. Cleanup-review and other operation classes retain the common command limit.

## Acceptance Criteria

- One run admits at most one Acceptance invocation after runtime expiry; the default expiry cannot multiply through automatic retries.
- Runtime expiry bypasses command retry, no-verdict retry, Acceptance retry, and Apply re-entry.
- A shorter positive common limit, including values below 300 seconds, remains an overriding safety bound.
- Configuration validation lives in the configuration capability and runs through existing load validation.
- Cleanup-review and non-Acceptance commands keep common-limit semantics.
- Tests use injected short limits or paused time; no test waits for the production minimum.

## Explicit Completion Conditions

- Tests prove timeout leaves the consecutive command-failure count at zero and does not dispatch Apply.
- Tests cover common limit 30 seconds versus dedicated limit 1,800 seconds, common zero, and cleanup-review classification.
- Configuration tests cover default, precedence, 299/300/10,800/10,801, and zero.
- `cargo test parallel:: --lib` and `cargo test config:: --lib` pass.

## Out of Scope

- A durable cross-restart per-change time budget.
- Changing Apply, Archive, cleanup-review, or analysis runtime limits.
- Per-verification runtime budgets.

## Fable Review

Fable verdict on the predecessor proposal: `adopt-with-changes`. This change applies the required routing, capability, and test corrections.

## Verification Ownership

Parallel routing tests own terminal-state behavior. Configuration tests own validated key semantics. Repository-wide checks remain hook-owned.

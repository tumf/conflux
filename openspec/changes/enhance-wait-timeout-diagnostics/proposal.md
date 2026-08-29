---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/client/wait.rs
  - src/client/session.rs
  - src/web/remote_control_api/dto.rs
  - tests/client_cli_tests.rs
verifications:
  - id: wait-timeout-diagnostics-tests
    requirement: Timeout envelopes preserve the latest coherent target observation without mutating workflow state
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/client_cli_tests.rs
    evidence: cargo test --test client_cli_tests client_wait
    rerun: cargo test --test client_cli_tests client_wait
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Enhance wait timeout diagnostics

**Change Type**: implementation

## Problem / Context

`cflx client wait <change-id> --timeout D --json` already returns control without stopping or mutating the Conflux execution. Its timeout detail currently reports only `commands_submitted: 0`, so an agent whose expected duration was exceeded must issue additional reads before it can distinguish active progress, a quiet phase, owner-observation delay, or repository certification delay.

The wait loop already obtains coherent owner observations containing target change progress and execution facts. Returning the last observation available before expiry gives the caller actionable evidence without introducing a change-level time budget or making timeout a workflow status.

## Proposed Solution

Enrich the typed timeout envelope with:

- `timeout_ms` and measured `wait_elapsed_ms`.
- A stable `timeout_stage`: `initial_observation`, `observing_owner`, `repository_certification`, or `remote_verification`.
- `last_observation`, containing only the target change and its matching execution projection from the last coherent observation completed before the deadline.
- Existing sanitized DTO projections for blocker, lifecycle activity, and retained log data rather than new parsing or unsanitized output.

The observation includes its `observed_at`, `state_revision`, and `event_sequence`; target change status and task progress; and execution identity, state, phases, timing boundaries, latest activity, and latest retained target log.

If no coherent observation completed before expiry, `last_observation` is `null` and no owner identity is invented. Wait performs no post-deadline read to fill missing diagnostics.

## Acceptance Criteria

- Timeout JSON identifies the configured duration, measured wait duration, and stage where the single operation deadline expired.
- When a coherent target observation exists, timeout JSON carries the latest completed target-only observation and matching execution facts.
- When the first observation never completes, timeout JSON carries `last_observation: null` and no fabricated `instance_id`.
- Timeout does not submit a command, stop or retry execution, mutate repository state, or change the proposal lifecycle status.
- Deadline expiry remains the authoritative outcome; a later transport or evidence error cannot replace it.
- Existing success and non-timeout failure envelope contracts remain compatible.

## Explicit Completion Conditions

- `src/client/wait.rs` retains the latest coherent target observation and emits the enriched timeout detail on every positive-timeout expiry path.
- Timeout-stage classification covers owner observation and repository/local-or-remote certification boundaries without parsing human-readable errors.
- Focused unit/integration tests verify observed and unobserved timeout shapes, latest-observation replacement, target-only projection, deadline precedence, and zero submitted commands.
- CLI/OpenSpec documentation describes the machine-readable timeout detail and its non-mutating semantics.

## Out of Scope

- Persisting an expected duration on a proposal.
- Changing Apply or Acceptance runtime limits.
- Automatically stopping, retrying, or marking a proposal `stalled` on wait timeout.
- Fetching logs or owner state after the deadline.
- Returning the complete owner snapshot or unrestricted process output.

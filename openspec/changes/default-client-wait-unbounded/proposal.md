---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/cli.rs
  - src/client/wait.rs
  - tests/client_cli_tests.rs
  - openspec/specs/cli/spec.md
verifications:
  - id: client-wait-tests
    requirement: The omitted and zero timeout forms wait without an operation deadline while explicit positive durations still time out
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/client_cli_tests.rs
    evidence: cargo test --test client_cli_tests wait_
    rerun: cargo test --test client_cli_tests wait_
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Default client wait to unbounded

**Change Type**: implementation

## Problem / Context

`cflx client wait <change-id>` defaults to a 60-minute operation deadline. A Hermes backend task can legitimately run longer, so an omitted timeout can stop observing healthy work and force the caller to arrange another observation.

The CLI currently rejects zero and represents every wait with a mandatory `Duration`. The implementation therefore cannot distinguish an unbounded wait from an immediate timeout.

## Proposed Solution

Make the default timeout `0`, with `0` meaning no overall operation deadline:

- `cflx client wait <change-id>` waits until verified completion, a typed terminal failure, owner replacement, cancellation by the calling process, or another non-timeout terminal observation.
- `cflx client wait <change-id> --timeout 0` has the same unbounded behavior.
- An explicit positive timeout keeps the existing monotonic operation-deadline behavior and typed `timeout` outcome.
- Internal transport and Git subprocess safety bounds remain in force. Unbounded operation duration must not create unbounded child processes or disable process cleanup.
- Help and operator documentation state that the default is `0` and that zero means unbounded.

## Acceptance Criteria

- Omitting `--timeout` and passing `--timeout 0` both select unbounded operation duration.
- An unbounded wait does not synthesize a deadline or return `timeout` merely because 60 minutes elapsed.
- Explicit positive durations retain their current parsing, upper bound, deadline enforcement, child termination, and typed timeout result.
- Invalid timeout syntax and values above the existing maximum remain usage errors.
- Wait remains observation-only and uses the same repository completion oracle.

## Explicit Completion Conditions

- CLI parsing tests prove that omitted timeout and `--timeout 0` select the same unbounded representation.
- Runtime tests prove an unbounded wait survives beyond a short test interval and can still settle from owner/repository evidence.
- Existing explicit-timeout tests continue proving deadline expiry and cleanup.
- `cargo test --test client_cli_tests wait_` passes.

## Out of Scope

- Adding `cflx_wait` back to MCP.
- Replacing proposal subscriptions for durable asynchronous notifications.
- Changing completion evidence, terminal classifications, owner lifecycle, or mutation behavior.
- Making individual transport or Git subprocess operations unbounded.

## Verification Ownership

The tracked `tests/client_cli_tests.rs` integration suite owns the bounded repository-local proof. Repository-wide checks remain owned by the existing commit hooks.

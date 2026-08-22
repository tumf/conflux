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
    evidence: cargo test --test client_cli_tests
    rerun: cargo test --test client_cli_tests
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
- `cflx client wait <change-id> --timeout 0` has the same unbounded behavior, and so does every accepted spelling whose value is exactly zero (`0s`, `0ms`, `0m`, `0h`).
- An explicit positive timeout keeps the existing monotonic operation-deadline behavior and typed `timeout` outcome.
- Per-request transport limits remain in force. Git subprocesses are today bounded only by the operation deadline, so an unbounded wait introduces a finite per-invocation deadline for every Git child it spawns; inner expiry terminates and reaps the child and is a recoverable or typed evidence condition, never the operation-level `timeout` outcome. Unbounded operation duration must not create unbounded child processes or disable process cleanup.
- Help and operator documentation state that the default is `0` and that zero means unbounded.

## Acceptance Criteria

- Omitting `--timeout` and passing any exactly-zero timeout spelling both select unbounded operation duration.
- An unbounded wait does not synthesize a deadline or return `timeout` merely because 60 minutes elapsed.
- An unbounded wait bounds every Git subprocess with a finite per-invocation deadline; a stalled remote lookup is terminated and reaped without producing the operation-level `timeout` outcome.
- Explicit positive durations retain their current parsing, upper bound, deadline enforcement, child termination, and typed timeout result.
- Invalid timeout syntax and positive values below the existing minimum or above the existing maximum remain usage errors.
- Wait remains observation-only and uses the same repository completion oracle.

## Explicit Completion Conditions

- CLI parsing tests prove that omitted timeout and every exactly-zero spelling select the same unbounded representation, and the existing `--timeout 0s` usage-rejection expectation is updated accordingly.
- Runtime tests prove an unbounded wait survives beyond a short test interval and can still settle from owner/repository evidence.
- A test proves an unbounded wait terminates and reaps a stalled Git child at its finite per-invocation deadline without returning the operation-level `timeout` outcome.
- Existing explicit-timeout tests continue proving deadline expiry and cleanup.
- `cargo test --test client_cli_tests` passes.

## Out of Scope

- Adding `cflx_wait` back to MCP.
- Replacing proposal subscriptions for durable asynchronous notifications.
- Changing completion evidence, terminal classifications, owner lifecycle, or mutation behavior.
- Making individual transport or Git subprocess operations unbounded.

## Verification Ownership

The tracked `tests/client_cli_tests.rs` integration suite owns the bounded repository-local proof. Repository-wide checks remain owned by the existing commit hooks.

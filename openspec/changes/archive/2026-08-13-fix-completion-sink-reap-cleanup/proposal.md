---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/changes/archive/2026-08-13-harden-client-mcp-completion-sinks
verifications:
  - id: completion-sink-reap-cleanup
    requirement: Graceful shutdown never removes callback event artifacts without confirmed child reap
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Deterministic completion-sink tests force delayed dispatcher acknowledgement and prove cleanup waits for confirmed reap
    rerun: cargo test --test client_completion_sink
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix completion-sink reap cleanup ordering

**Change Type**: implementation

## Problem / Context

`CompletionSinkRegistry::owner_stopping()` cancels delivery after the global shutdown deadline, but waits only `REAP_GRACE` for dispatcher acknowledgement. If that second wait expires, it logs a warning and immediately drops the event directory. A delayed callback reap can therefore lose its event file while still alive, contradicting the canonical requirement that cleanup occurs only after every callback is reaped.

## Proposed Solution

- Keep the global admission and callback cancellation deadline.
- After cancellation, wait for the dispatcher acknowledgement that the active child has been explicitly reaped before dropping event artifacts.
- Treat a missing dispatcher as safe only when no callback can still be active.
- Test ordering with injected synchronization/state transitions, not elapsed-time performance assertions.

## Acceptance Criteria

- Event-directory cleanup cannot execute on a timeout-only fallback while dispatcher reap acknowledgement is outstanding.
- A callback delayed during cancellation retains its event artifact until the child is confirmed reaped.
- Shutdown still starts no new callback or event artifact after cancellation.
- Existing completion-sink and full regression tests pass.

## Explicit Completion Conditions

- `cargo test --test client_completion_sink` passes with a deterministic delayed-acknowledgement regression test.
- `cargo test` passes.
- `cargo fmt --all -- --check` and clippy pass.
- Strict and archive-gate OpenSpec validation pass.

## Out of Scope

- Changing callback delivery serialization.
- Changing the global shutdown admission/cancellation deadline.
- Durable callback delivery across owner crashes.

---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/changes/archive/2026-08-13-fix-completion-sink-reap-cleanup
verifications:
  - id: unacknowledged-callback-artifact-retention
    requirement: Missing dispatcher acknowledgement never removes callback artifacts
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/client_completion_sink.rs
    evidence: Deterministic sender-drop regression proves registry destruction retains artifacts when child reap was not acknowledged
    rerun: cargo test --test client_completion_sink
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Retain unacknowledged callback artifacts

**Change Type**: implementation

## Problem / Context

`CompletionSinkRegistry::owner_stopping()` now waits without a secondary timeout, but still calls `cleanup_events()` when the dispatcher acknowledgement sender is dropped. The registry stores its event directory as `tempfile::TempDir`, so registry destruction also deletes that directory automatically. A dropped sender proves only that the dispatcher task ended; it does not prove that a spawned callback child was explicitly reaped. This contradicts the canonical requirement that missing acknowledgement cannot authorize cleanup while a callback may remain alive.

## Proposed Solution

- Replace automatic `TempDir` cleanup with an explicitly managed owner-private event-directory path.
- Delete the directory only when the shutdown wait resolves as `Ok(Ok(()))`: positive dispatcher acknowledgement confirms callback reap.
- Treat pre-deadline sender drop, post-cancellation sender drop, and task-send failure identically: retain the directory and artifacts as the fail-safe behavior.
- Preserve randomized exclusive temporary-directory creation and disarm only automatic Drop cleanup.
- Emit one bounded warning containing only the retained directory path on every missing-acknowledgement path.
- Keep shutdown admission, cancellation, serialization, and callback limits unchanged.

## Acceptance Criteria

- Positive dispatcher acknowledgement permits event-directory cleanup.
- Missing acknowledgement, including task-send failure or sender drop, never deletes event artifacts.
- Registry destruction cannot implicitly delete an unacknowledged artifact.
- A deterministic regression test proves the sender-drop path retains the artifact.
- Retained-artifact warnings identify the directory without exposing payloads, tokens, or callback output.
- `AGENTS.md` documents fail-safe retention when reap acknowledgement is unavailable.
- Existing shutdown, completion-sink, and full regression suites pass.

## Explicit Completion Conditions

- `src/web/completion_sink.rs` uses `TempDir::keep()`-backed explicit path ownership rather than Drop cleanup, while preserving randomized exclusive creation and `0700` restriction.
- `cargo test --test client_completion_sink` includes and passes a hook-free pre-deadline sender-drop regression.
- `cargo test`, format, and clippy pass.
- Strict and archive-gate validation pass.

## Out of Scope

- Durable callback delivery across process crashes.
- Automatic cleanup of fail-safe retained files.
- A public dispatcher abort/kill test hook.
- Changes to callback serialization or deadlines.

---
change_type: implementation
priority: high
dependencies:
  - expose-authoritative-operator-snapshot
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/frontend-abstraction/spec.md
  - openspec/specs/orchestration-events/spec.md
  - openspec/specs/remote-control-api/spec.md
  - src/events.rs
  - src/tui/runner.rs
  - src/web/state.rs
  - src/web/remote_control_api/projection.rs
verifications:
  - id: projection-ownership-tests
    requirement: Each internal event produces one reducer transition, one v2 projection update, and at most one retained log in serial and parallel modes
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: Deterministic event ownership, reducer, replay, and projection test output
    rerun: cargo test --features web-monitoring --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix remote event projection ownership

**Change Type**: implementation

## Problem / Context

Execution events currently traverse reducer, TUI forwarding, legacy WebState, and v2 projection paths with overlapping ownership. This permits duplicate reducer application, divergent serial/parallel log visibility, and terminal-mode disagreement when late completion events arrive. A remote snapshot and stream cannot be authoritative while one internal event may be projected more than once or lose fields through an intermediate frontend model.

## Proposed Solution

Define one core dispatch owner that applies each execution event to reducer state once and fans the resulting state/event to frontend sinks. Make v2 projection consume the same authoritative event/state output directly rather than reprojecting a lossy frontend copy. Allocate one event sequence per internal event, retain each structured log once, and align terminal mode rules across TUI and v2.

## Acceptance Criteria

1. One internal execution event causes exactly one reducer transition and one ordered v2 event; duplicate delivery is a tested no-op.
2. `ApplyCompleted` and other counters advance once, not once per frontend path.
3. Serial and parallel AI, hook, lifecycle, warning, and structured log events reach v2 with identical ownership and at most one retained entry.
4. Late `AllCompleted` does not overwrite retained Error or Stopped modes when the TUI would preserve them.
5. Replay sequence, state revision, gap detection, and command-result ordering remain coherent after ownership consolidation.
6. Core workflow decisions remain independent of frontend projection and observability state.

## Explicit Completion Conditions

- Event producers use the single dispatch path or document a presentation-only exception with a regression test.
- TUI, v2 projection, and any retained legacy frontend adapter receive events through `EventSink` without directly reapplying reducer state.
- Deterministic tests cover every `ExecutionEvent` class, serial/parallel parity, duplicates, log retention, late terminal events, replay gaps, and no-op revisions.
- `cargo test --features web-monitoring event projection reducer` passes.

## Out of Scope

- Changing workflow routing or making event history authoritative.
- Browser-side stream rendering.
- Increasing event/log retention limits.

---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/remote-control-api/spec.md
verifications:
  - id: stop-wait-local
    requirement: A settled stopped execution releases wait even when its change display remains not queued, while idle and live changes keep observing
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/client/wait.rs
    evidence: cargo test --lib --features web-monitoring stop_wait -- --nocapture
    rerun: cargo test --lib --features web-monitoring stop_wait -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: stop-wait-cli
    requirement: Real CLI wait observes coherent stopped execution facts and exits with typed action outcome without owner mutation
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/client_cli_tests.rs
    evidence: cargo test --test client_cli_tests --features web-monitoring stop_wait -- --nocapture
    rerun: cargo test --test client_cli_tests --features web-monitoring stop_wait -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Release client wait after settled stop

**Change Type**: implementation

## Problem

The stopped execution episode can project as `execution_state=stopped` while its change row projects as `display_status=not queued`. This is intentional for process-level stop and graceful dequeue (a resumable, not-queued TUI row), but `cflx client wait` classifies only the display row and blocker. The default unbounded wait therefore never releases after a confirmed stop, even when the matching owner read repeatedly publishes stopped execution facts. Targeted force-stop has a separate terminal `stopped` display path and already releases. An isolated real-reducer/API/CLI reproduction on installed v0.6.327 yielded `2 passed, 5 failed` against the desired stop contract; see the locally retained diagnostic `/tmp/conflux-stop-wait-investigation/REPORT.md` only as background, not a required repository input.

## Supported result

For a tracked target whose *coherent same-revision* execution facts show a **settled** `stopped` state for an admitted execution episode — the matching `/api/v2/execution-status` row for the same change `id` carries a non-null `execution_id` and `execution_state=stopped` — `client wait` releases as `change_requires_action` (exit 27), regardless of an otherwise `not queued` display. The exception applies only where display classification alone would keep observing; it never overrides `rejected`, a success-claiming row, or an existing display-based release, and it is evaluated after owner replacement, fatal process failure, and first-observation absence handling. This applies both to wait already in progress and to an initial stopped observation. The result must preserve the actual display status (`not queued`) as `detail.observed_status` rather than fabricate a `stopped` row, and carry the typed stopped execution fact as `detail.execution_state` and `detail.execution_id` alongside the existing `detail.commands_submitted: 0`. Neither `Stopping` nor a globally stopped owner without target-specific stopped execution evidence is enough. No mutation or automatic restart occurs.

The terminal-success certification, rejection, owner incarnation replacement, process failure, absence handling, and timeout policies remain as documented; these must not be bypassed by a stale, missing, or wrong-target execution fact. A truly unadmitted `not queued` row — `execution_state=unknown`, or `stopped` with a `null` `execution_id` because the owner never admitted it in this incarnation, which also covers an owner predating execution identity — remains observable; a newly re-admitted episode after explicit resume must not be released by an old stopped episode. The authoritative API surfaces remain unchanged.

## Change boundary

Update the shared wait completion disposition/input in `src/client/completion.rs` and its use in `src/client/wait.rs` so the target execution fact in the coherent `Observation` participates in stop classification. Add focused unit and real-CLI tests in the existing test modules. Carry the complete existing `Observation-only completion wait` requirement text and scenarios as a MODIFIED requirement in `openspec/changes/fix-stop-wait-release/specs/cli/spec.md`, adding only the stopped-execution exception and its scenarios, so archive replaces the canonical requirement without dropping any existing contract. Do not change reducer/TUI NotQueued and mark/resume behavior, callback delivery, project owner startup, Conflux Server, or unrelated CLI behavior. Do not deploy/replace an existing owner as a side effect.

## Acceptance

1. Real reducer stop/dequeue combinations (`Stopped`, `ChangeStopped`, graceful dequeue) yield `display_status=not queued` + target `execution_state=stopped` with a non-null `execution_id`; wait started before or after the stop exits `change_requires_action`/27, carrying `detail.observed_status: not queued`, `detail.execution_state: stopped`, the settled `detail.execution_id`, and `detail.commands_submitted: 0`. A default no-timeout wait also releases without a test harness kill.
2. Fresh idle `not queued`/`unknown`, `stopped` with a `null` `execution_id`, active and stopping episodes keep observing. Targeted force-stop display `stopped` retains its existing 27 outcome. Rejected, external-blocked, and repository-certified successful cases retain their existing behavior.
3. Stop is target-scoped and incarnation/revision-coherent; absent/stale facts, another target's stop, and a newly resumed episode cannot spuriously release the waiter. All wait paths issue zero mutation commands.
4. Both focused local and real CLI test gates select nonzero cases and pass; no existing test matches the `stop_wait` filter today, so every new test name must contain `stop_wait` or the declared evidence commands select zero tests. OpenSpec strict validation and archive gate pass at their respective stages, with implementation evidence from the new tests rather than proposal validation alone.

## Delivery boundary

Implementation, tests, and canonical spec update after archive belong to the authorized Conflux change. Publishing/releasing a binary, replacing live owners, changing other sessions' worktrees, and cleaning up the incident's original watcher are separate operations and are not part of this change.

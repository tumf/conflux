---
change_type: implementation
priority: high
dependencies: []
references:
  - src/client/wait.rs
  - tests/client_cli_tests.rs
  - openspec/specs/cli/spec.md
verifications:
  - id: client-wait-external-blocker
    requirement: An external blocked row releases an unbounded completion waiter without mutation
    phase: pre-integration
    owner: conflux-acceptance
    trigger: change-implementation
    automation: tests/client_cli_tests.rs
    evidence: focused client CLI regression test output
    rerun: cargo test --test client_cli_tests wait_releases_external_blocker_without_waiting_for_operator -- --exact
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Release completion wait on external blockers

**Change Type**: implementation

## Problem / Context

`cflx client wait` currently treats every `blocked` row as automatically progressing. A validated external blocker is different: the owner has parked the change until an external prerequisite changes and an operator explicitly retries it. With the default unbounded timeout, callers remain attached forever after Conflux has already returned control as `blocked:external`.

The current owner snapshot exposes the distinction through structured blocker data. Dependency waits and other owner-progressing blocked states must keep observing.

## Proposed Solution

Classify a `blocked` row with a structured external blocker as requiring action. Return the existing `change_requires_action` outcome and exit status `27`, preserving observed status and blocker detail while submitting no command.

Keep observing blocked rows that have no external blocker classification, including dependency waits the owner can advance automatically.

## Acceptance Criteria

- An unbounded wait releases on the first coherent observation of `blocked` with `blocker.kind = external`.
- A wait already observing live work releases when a later coherent observation becomes externally blocked.
- The result uses `change_requires_action`, exit status `27`, `detail.observed_status = blocked`, available blocker/error detail, and `detail.commands_submitted = 0`.
- Generic and dependency-driven `blocked` rows continue observing.
- Wait remains observation-only and does not retry, start, dequeue, resolve, or mutate repository state.

## Explicit Completion Conditions

- `src/client/wait.rs` classifies structured external blockers separately from owner-progressing blocked states.
- `tests/client_cli_tests.rs` covers immediate and transitioned external blockers plus the retained generic-blocked behavior.
- `cargo test --test client_cli_tests wait_releases_external_blocker_without_waiting_for_operator -- --exact` reports exactly one executed and passing test.
- The existing generic-blocked wait test remains passing.
- The CLI help and canonical CLI requirement describe the external-blocker exception.

## Out of Scope

- Automatically retrying or unblocking changes.
- Changing external-blocker production or reducer semantics.
- Reclassifying dependency waits or active phases as terminal.
- Adding a new outcome or exit status.

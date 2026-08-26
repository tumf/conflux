---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - src/client/wait.rs
  - src/client/envelope.rs
  - tests/client_cli_tests.rs
verifications:
  - id: client-wait-terminal-release
    requirement: cflx client wait returns when the observed change cannot progress without new operator action
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Focused compiled-CLI tests cover initial and transitioned terminal/manual-action states plus an automatically progressing state
    rerun: cargo test --features web-monitoring --test client_cli_tests wait_
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Release client wait on terminal and manual-action statuses

**Change Type**: implementation

## Premise / Context

- `cflx client wait` is observation-only and currently exits for repository-certified success, rejection, fatal owner failure, owner replacement, or an explicit timeout.
- A per-change `error` or `merge wait` row does not match those exits, so an unbounded wait remains open even though progress now requires a new operator action.
- A change already shown in a terminal status must be classified on the initial observation rather than waiting for a future event that may never occur.
- The constitution still requires repository-verifiable evidence before reporting successful completion.

## Requested Artifact

A bounded status-classification correction to `cflx client wait`, with typed non-success results and regression tests.

## Proposed Solution

1. Classify observed per-change statuses by whether the current owner can still advance them without a new operator command.
2. Keep observing statuses that may advance automatically, including active phases and recoverable external-condition holds.
3. Return immediately for `error`, `merge wait`, `stopped`, `rejected`, and other final rows. Preserve repository certification before `merged` can return successful `completed`; if the row is final but success evidence is not yet usable, return a typed non-success result rather than hold indefinitely.
4. Add stable non-success outcome `change_requires_action` with exit status `27`, carrying `detail.observed_status`, optional `detail.error_detail`, and `detail.commands_submitted: 0`. Reuse existing `change_rejected`, `process_failed`, evidence, and completed outcomes where they already fit.
5. Apply the same classification on initial observation and every subsequent coherent snapshot. Submit no workflow command.

## Acceptance Criteria

1. An initial snapshot with `error` or `merge wait` exits immediately with the typed non-success outcome and includes the observed status.
2. A wait already in progress exits when the row transitions into `error`, `merge wait`, `stopped`, `rejected`, or another final/manual-action status.
3. An initial `merged` row is classified immediately: after at most one bounded coherent re-observation and re-certification, it returns `completed` when evidence certifies success and otherwise releases with a typed non-success result.
4. `not queued`, `queued`, `blocked`, `applying`, `accepting`, `rejecting`, `archiving`, and `resolving` continue to hold. `error`, `merge wait`, `stopped`, and `stalled` release with `change_requires_action`.
5. Every result reports `commands_submitted: 0`; no start, retry, resolve, merge, archive, queue, cleanup, or worktree mutation occurs.
6. Existing timeout, owner replacement, repository evidence, rejection, and fatal-process behavior remains compatible.

## Out of Scope

- Automatically retrying, resolving, merging, or repairing a change.
- Treating presentation status alone as successful completion.
- Changing owner orchestration, status production, or repository completion certification.
- Reclassifying recoverable external-condition holds that can clear without a new operator command.

## Rollout

This is a backward-compatible correction for automatically progressing waits. Scripts must handle the new typed non-success outcome when a change requires operator action or has settled unsuccessfully.

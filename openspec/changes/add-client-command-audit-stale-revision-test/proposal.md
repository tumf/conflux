---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/cli/spec.md
  - src/client/enqueue.rs
  - tests/client_cli_tests.rs
verifications:
  - id: stale-revision-command-audit
    requirement: one enqueue invocation preserves an exact submitted-command audit across stale-revision recomputation
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: A deterministic production-boundary test forces StaleRevision inside one invocation and compares the envelope list with recorded submissions
    rerun: cargo test --features web-monitoring --test client_cli_tests stale_revision_command_audit
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add stale-revision client command audit regression test

**Change Type**: implementation

## Premise / Context

- The command-audit implementation now records commands on actual submission rather than successful settlement.
- Independent review confirmed normal new-mark, pre-marked, and pre-submission-refusal paths.
- The existing stale-revision test advances revision after a prior invocation has settled. It does not force `StaleRevision` and recomputation inside one invocation.
- Code inspection suggests the append-only audit is correct, but this concurrency boundary is not proven.

## Requested Artifact

One deterministic production-boundary regression test, with the smallest fixture adjustment needed to force and observe one in-invocation stale-revision retry.

## Problem / Context

Without this test, a future refactor can duplicate, omit, or reorder `commands_submitted` across a stale-revision retry while all current tests pass.

## Proposed Solution

Arrange the real command endpoint or a deterministic production-boundary hook so the first mutation attempt in one `enqueue` invocation is rejected with `StaleRevision`. Let the client perform its normal authoritative reread and recomputation, then settle the next supported submission path. Compare the exact ordered command records actually submitted with `partial_intent.detail.commands_submitted` or the resulting envelope audit. Use event/state synchronization, not short wall-clock timing.

## Acceptance Criteria

1. One compiled-CLI `enqueue` invocation demonstrably receives `StaleRevision` before recomputing.
2. The test proves the reread/recompute path occurs in that same invocation.
3. `commands_submitted` equals the exact ordered sequence of commands accepted for submission across the retry, with no duplicates or omissions.
4. Attempts rejected before creation of a command record are not reported as submitted.
5. No production behavior change unless a genuine audit defect is exposed.

## Explicit Completion Conditions

- The focused test fails if stale-revision injection is removed.
- The focused test fails if audit order, duplication, or omission differs from command records.
- Existing partial-intent tests remain green.
- fmt and clippy pass.

## Out of Scope

- Changing retry count or enqueue routing.
- Timing-based concurrency tests.
- Refactoring unrelated test infrastructure.

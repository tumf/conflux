---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/cli/spec.md
  - src/client/enqueue.rs
  - tests/client_cli_tests.rs
verifications:
  - id: client-command-audit
    requirement: partial intent reports every command actually submitted by this invocation, regardless of settlement outcome
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Focused compiled-CLI tests compare the envelope audit list with the production command spy
    rerun: cargo test --features web-monitoring --test client_cli_tests partial_intent
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix client partial-intent command audit

**Change Type**: implementation

## Premise / Context

- Independent review confirmed the JSON usage, wait deadline, and bearer-token fixes.
- `Start` is sent and can settle unsuccessfully, but the client appends `start` to `commands_submitted` only after successful settlement.
- Current tests prove the spy received `Start` while expecting an envelope that omits it.
- The canonical contract requires commands actually submitted by the invocation, not only successful commands.

## Requested Artifact

A surgical audit-accounting correction and focused regression tests.

## Problem / Context

When `Start` fails after submission, `partial_intent.detail.commands_submitted` understates the external side effects. This makes the machine-readable audit trail disagree with the owner command records and can mislead recovery logic.

## Proposed Solution

Record a command in the per-invocation submitted list immediately when its POST is accepted for submission, before waiting for or interpreting settlement. Preserve the distinction between submitted commands and successful commands. Do not list commands that were skipped, including a pre-existing execution mark.

## Acceptance Criteria

1. A newly submitted mark followed by a submitted failing `Start` reports `["set_execution_mark", "start"]`.
2. A pre-existing mark followed by a submitted failing `Start` reports `["start"]`.
3. A command rejected before submission is not listed.
4. The envelope list equals the production spy's submitted command sequence for all partial-intent cases.
5. No enqueue routing, rollback, retry, or owner API semantics change.

## Explicit Completion Conditions

- Focused compiled-CLI tests assert exact equality between command-spy submissions and `partial_intent.detail.commands_submitted`.
- Existing enqueue and client CLI tests pass.
- fmt and clippy pass.

## Out of Scope

- Changing `partial_intent` outcome conditions.
- Adding retries or rollback.
- Refactoring unrelated enqueue logic.

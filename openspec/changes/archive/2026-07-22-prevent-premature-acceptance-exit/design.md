# Design: Acceptance Completion Ownership

## Decision

Treat acceptance completion as a two-part contract:

1. The portable acceptance skill requires the parent agent to wait for all verification it starts and then emit a canonical verdict.
2. The runtime treats process exit without that verdict as a protocol failure distinct from intentional `CONTINUE`.

## Runtime Flow

The output parser must preserve the difference between:

- an explicit canonical `continue` verdict, which uses the configured continuation retry policy;
- no canonical verdict, which indicates an incomplete or malformed acceptance run and produces explicit protocol-failure evidence.

Serial and parallel execution must consume the same distinction. Operator logs and attempt history should identify `missing acceptance verdict` and retain bounded stdout/stderr evidence without introducing a workspace-root report or out-of-worktree workflow state.

## Portable Skill Rule

The skills must not name or require a specific harness tool such as `Monitor`. They instead assign ownership semantically: if the parent starts a command, sub-agent, job, or monitored verification, it must await the final result. Progress prose cannot replace the final verdict.

## Alternatives Rejected

### Rely only on stronger prompt wording

Stale installed skills, custom acceptance commands, and agent noncompliance would remain silently classified as intentional continuation.

### Keep missing verdict as `CONTINUE` with better logging

This preserves the semantic collision and incorrectly consumes the explicit-CONTINUE retry budget.

### Terminate verification immediately when any verdict-like text appears

Existing canonical verdict grace handling remains appropriate. This change concerns process exit without any canonical verdict, not early finalization after a valid verdict.

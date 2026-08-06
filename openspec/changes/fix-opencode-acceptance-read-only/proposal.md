---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/agent-prompts/spec.md
  - .opencode/commands/cflx-accept.md
  - skills/cflx-accept/SKILL.md
  - src/embedded_skills.rs
  - src/config/defaults.rs
  - .opencode/commands/link.sh
verifications:
  - id: opencode-acceptance-read-only-contract
    requirement: The OpenCode acceptance adapter keeps review read-only and delegates FAIL follow-up persistence to Conflux runtime
    phase: pre-integration
    owner: conflux-acceptance
    trigger: acceptance-review
    automation: src/embedded_skills.rs
    evidence: cargo test --lib embedded_skills::tests::test_opencode_acceptance_command_is_read_only verifies required ownership language and rejects agent-side tasks.md mutation instructions
    rerun: cargo test --lib embedded_skills::tests::test_opencode_acceptance_command_is_read_only
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Change: Fix OpenCode acceptance adapter read-only ownership

**Change Type**: implementation

## Problem / Context

The canonical `agent-prompts` specification and bundled `cflx-accept` skill define Acceptance as a read-only review. Conflux runtime owns persistence of repository-fixable FAIL findings in one `## Current Acceptance Follow-up` section.

The default Acceptance command in `src/config/defaults.rs` invokes the tracked OpenCode adapter, and `.opencode/commands/link.sh` installs that adapter by symlink rather than generating another copy. The adapter still instructs the Acceptance agent to edit `tasks.md`, choose the next numbered `## Acceptance #N Failure Follow-up`, and append unchecked tasks. An agent following that adapter dirties the reviewed worktree after checking it, and the next Acceptance attempt can fail solely on the reviewer-created mutation. The numbered section also bypasses the runtime's latest-only follow-up ownership and reconciliation behavior.

## Proposed Solution

Update `.opencode/commands/cflx-accept.md` so a FAIL review returns complete actionable findings without modifying `tasks.md` or any runtime-owned follow-up section. State that Conflux runtime persists normalized repository findings after the verdict, and replace ambiguous external-dependency wording that asks for "follow-up tasks" with wording that asks for actionable findings.

Add a focused drift test in `src/embedded_skills.rs` that reads the tracked adapter through `CFLX_ACCEPT_COMMAND_MD`, requires explicit read-only/runtime-ownership language, and rejects the three exact obsolete imperative anchors without rejecting prohibition text that legitimately names `tasks.md` or numbered follow-up sections.

## Acceptance Criteria

- The OpenCode Acceptance adapter explicitly defines review as read-only.
- The adapter instructs a failing reviewer to return all findings in the verdict and not to edit `tasks.md` or `## Current Acceptance Follow-up`.
- The adapter delegates persistence and reconciliation of repository-fixable findings to Conflux runtime.
- The adapter no longer instructs reviewers to derive an Acceptance attempt number or append `## Acceptance #N Failure Follow-up` sections.
- A repository-local regression test requires positive ownership wording and rejects the exact imperative anchors present in the pre-change adapter, without using broad substrings that also match legitimate prohibitions.
- Existing verdict, blocker, scoped-review, dirty-tree, behavior-task adequacy, external-dependency, and permission-error guidance remains intact apart from replacing ambiguous "follow-up tasks" wording with "actionable findings".

## Explicit Completion Conditions

- `.opencode/commands/cflx-accept.md` contains no instruction that an Acceptance agent update or append to `tasks.md` after FAIL.
- `src/embedded_skills.rs` includes a focused test that requires explicit read-only/runtime-persistence wording and rejects exactly `After listing all findings, update openspec/changes/<change_id>/tasks.md`, `Determine the next acceptance attempt number`, and `Append or create the section for that attempt`.
- `cargo test --lib embedded_skills::tests::test_opencode_acceptance_command_is_read_only` selects one test and passes.
- Existing `embedded_skills` acceptance prompt ownership tests continue to pass.
- No runtime persistence code, task parser behavior, or portable acceptance skill contract is replaced or duplicated.

## Out of Scope

- Changing structured verdict parsing, repair budgets, stalled-hold classification, or runtime follow-up persistence.
- Rewriting `skills/cflx-accept/SKILL.md` or `skills/cflx-accept-with-speca/SKILL.md`, which already carry the read-only contract.
- Migrating existing numbered follow-up sections; current runtime reconciliation already owns that behavior.
- Implementing `fix-precomplete-apply-repair-termination` or modifying any other active change.

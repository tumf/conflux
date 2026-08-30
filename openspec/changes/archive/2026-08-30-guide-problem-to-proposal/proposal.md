---
change_type: implementation
priority: high
dependencies: []
references:
  - skills/cflx-proposal/SKILL.md
verifications:
  - id: bundled-proposal-policy
    requirement: The bundled proposal skill teaches agents to turn investigated problems into implementation-ready change contracts
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/install_skills_test.rs
    evidence: cargo test --test install_skills_test
    rerun: cargo test --test install_skills_test
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Guide problem-to-proposal decisions

**Change Type**: implementation

## Problem / Context

The bundled `cflx-proposal` skill mainly teaches proposal format and validation. It does not clearly establish that a reported problem is investigation input rather than a ready proposal, nor does it teach how to convert verified findings into a permanent change contract. Agents can therefore formalize an early hypothesis, leave design decisions to Apply, and create chains of corrective proposals.

## Proposed Solution

Add a concise, early policy section to `skills/cflx-proposal/SKILL.md` that makes proposal authoring a problem-to-contract transformation:

1. establish current behavior and root cause from read-only repository evidence, within the skill's existing proposal-only scope;
2. separate temporary diagnostics and local repairs from the permanent change;
3. choose the implementation approach and record scope-relevant rejected alternatives;
4. define the observable final state, change boundary, preserved contracts, failure behavior, and repository-local acceptance;
5. produce tasks that require no new design decisions from the implementation agent.

Explicitly reject `investigate and fix` as a proposal task: investigation must happen first, and only the resulting permanent transition belongs in the proposal.
If investigation shows that no permanent change is required, do not create a proposal.

Add a focused repository test that protects this bundled guidance from regression and confirms it remains part of the embedded skill set.

## Acceptance Criteria

- The policy appears directly after `## Scope Restrictions (Proposal-Only)` and before `## Guardrails (Match Command Behavior)`.
- A problem report is described as investigation input, not automatically as a proposal.
- The guidance defines how repository evidence becomes an implementation-ready permanent change contract.
- The guidance distinguishes temporary investigation artifacts and local repairs from production changes.
- The guidance requires observable final state, boundaries, preserved contracts, failure behavior, repository-local acceptance, and no unresolved implementation-time design decisions.
- The guidance rejects `investigate and fix` tasks.
- A repository-local test fails if the policy disappears from the bundled skill.

## Explicit Completion Conditions

- `skills/cflx-proposal/SKILL.md` contains the policy directly after `## Scope Restrictions (Proposal-Only)` and before `## Guardrails (Match Command Behavior)`.
- `tests/install_skills_test.rs` verifies that ordering and the policy's decisive behavioral markers, including separation of temporary work from the permanent change and rejection of `investigate and fix` tasks.
- `cargo test --test install_skills_test` passes.

## Out of Scope

- Changing proposal file formats or OpenSpec validation semantics.
- Changing Conflux execution, owner, monitoring, or callback behavior.

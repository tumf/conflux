---
change_type: implementation
priority: high
dependencies: []
references:
  - src/openspec_cmd/validation.rs
  - src/openspec_cmd.rs
  - openspec/specs/cflx-proposal-validation/spec.md
  - skills/cflx-proposal/SKILL.md
verifications:
  - id: proposal-gate-validation-tests
    requirement: Strict validation detects change-blocking verification declarations that bundle unrelated task evidence or heavyweight broad-suite commands while preserving legitimate shared focused gates
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/openspec_cmd/validation.rs
    evidence: cargo test openspec_cmd --lib
    rerun: cargo test openspec_cmd --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: proposal-guidance-embedding-tests
    requirement: Bundled proposal guidance and installation assertions teach the bounded verification rule
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/embedded_skills.rs
    evidence: cargo test embedded_skills --lib
    rerun: cargo test embedded_skills --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Prevent bundled verification gates

**Change Type**: implementation

## Problem / Context

A proposal can currently attach every implementation checkbox to one `change-blocking` verification. This is structurally valid even when unrelated tasks, full repository suites, Docker orchestration, fuzzing, or cross-architecture checks are bundled into one Acceptance gate. The result is duplicate work, poor failure attribution, and multi-hour Acceptance runs.

The existing `verifications`, `execution_class`, `completion_role`, and `verification-id` model already has the required vocabulary. A parallel task taxonomy or scheduler-level task selection mechanism is unnecessary.

## Proposed Solution

Extend native proposal validation and bundled proposal guidance to enforce verification cohesion using explicit structure and command metadata only:

- A change-blocking verification may be shared by multiple checkboxes only when each referenced task uses the same verification ownership marker and the same concrete rerun command.
- A change-blocking declaration is rejected when any declared command form—`evidence`, `rerun`, a task-line concrete command, or a structured argv field when present—matches a configurable denylist of structurally heavyweight execution forms. The initial native list covers Docker/container orchestration, cross-architecture emulation, benchmark commands, and explicit full/exhaustive/heavy suite selectors.
- Repetition or stability loops become validator-rejected Apply-blocking work.
- The diagnostic identifies the verification ID and affected task lines, and directs the author to split bounded requirement-specific proof from `operational-observation` or repository automation.
- Validation does not infer task meaning from prose and does not require one verification ID per checkbox when a focused test command legitimately proves several tightly coupled tasks.

## Acceptance Criteria

- Strict validation rejects a single change-blocking verification reused across task lines that declare different verification ownership markers or different concrete commands.
- Strict validation rejects structurally identified heavyweight commands in every declared command form and recommends bounded local proof plus non-blocking broad verification.
- A focused repository-local command shared by tightly coupled implementation and regression-test tasks remains valid.
- Legacy proposals are not reclassified from natural-language descriptions.
- Bundled `cflx-proposal` guidance teaches the same rule and examples.

## Explicit Completion Conditions

- Parser/validator tests cover invalid heterogeneous reuse, invalid heavyweight commands, valid focused sharing, and narrative/Future Work exclusions.
- Diagnostics name the verification ID and task line locations.
- `cargo test openspec_cmd --lib` and `cargo test embedded_skills --lib` pass.

## Out of Scope

- Adding a new task taxonomy.
- Runtime Task selection inside Acceptance.
- Executing post-integration observations.
- Evidence reuse or Acceptance process runtime limits.
- Detecting semantically unrelated tasks that intentionally declare an identical ownership marker and identical command.
- Proving that a Cargo selector corresponds semantically to the declared `automation` source path.

## Verification Ownership

`src/openspec_cmd/validation.rs` owns bounded validator regression coverage. `src/embedded_skills.rs` owns bundled-guidance installation assertions. Repository-wide checks remain owned by existing hooks.

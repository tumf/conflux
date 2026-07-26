---
change_type: hybrid
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cflx-proposal-validation/spec.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/parallel-execution/spec.md
  - src/openspec_cmd/validation.rs
  - src/execution/apply.rs
  - src/agent/prompt.rs
  - skills/cflx-apply/SKILL.md
verifications:
  - id: task-format-regression
    requirement: Task section classification and pre-accept validation prevent malformed tasks.md from reaching acceptance
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test results covering validator section classification and apply handoff behavior
    rerun: make test
    prerequisites: []
---

# Prevent Repeated Task-Format Acceptance Loops

**Change Type**: hybrid

## Problem / Context

A `tasks.md` evidence list using top-level `- ` bullets can be classified as unchecked implementation work and fail strict or archive-gate validation with `Possible task without checkbox`. The apply loop currently determines completion from checkbox progress without enforcing task-format validity before acceptance, so an apply agent can mark a runtime follow-up complete while leaving or recreating the same malformed list. Acceptance then reports the same repository finding and starts another apply cycle.

The repository also has conflicting contracts: apply guidance treats `Final Validation` and `Implementation Blocker` as non-task sections, while the native validator explicitly excludes only Future Work, Out of Scope, and Notes. This makes valid guidance-generated metadata vulnerable to the same diagnostic.

This violates the constitutional requirement that completion and acceptance be based on repository-verifiable evidence rather than checklist normalization alone.

## Proposed Solution

Define one native task-section classification shared by task counting and task validation. Active task sections continue to reject top-level non-checkbox task-like bullets. Narrative sections, including Final Validation and Implementation Blocker, permit ordinary prose/list metadata but reject checkboxes. Runtime-owned acceptance follow-up remains a distinct class with its existing ownership rules.

Align proposal/apply guidance and canonical prompt requirements with that classifier. Guidance will distinguish plain narrative content from active task bullets and require acceptance finding evidence to use the exact indented `  evidence:` form.

Before handing a completed apply result to acceptance, run deterministic worktree-local task validation. If task format is invalid, keep the change in apply, provide the actionable diagnostics to the next apply attempt, and do not spend an acceptance cycle rediscovering the same defect.

## Scope Rationale

The validator contract, agent guidance, and apply-to-acceptance gate must ship together. Updating only guidance remains probabilistic; updating only the gate would enforce the existing contradictory classifier; updating only section classification would still allow other malformed active bullets to reach acceptance.

## Acceptance Criteria

- Active task sections still reject top-level `- ` or `* ` items that look like tasks but lack checkboxes.
- Final Validation, Implementation Blocker, Future Work, Out of Scope, Notes, and Acceptance Notes use one consistent non-task classification for counting and validation.
- Non-task sections permit narrative bullets and reject checkbox tasks.
- Runtime-owned acceptance follow-up retains its dedicated parsing and validation behavior.
- Apply guidance does not suggest validator-invalid metadata and clearly distinguishes `  evidence:` from top-level `- evidence:`.
- A completed checkbox set with an invalid active-section bullet does not proceed to acceptance.
- The validation diagnostic is available to the subsequent apply attempt, and acceptance begins only after task-format validation succeeds.
- The prevention works from workspace-local files and Git state without introducing out-of-worktree workflow authority.

## Explicit Completion Conditions

- Native validator tests prove active bare bullets fail, narrative-section bullets pass, narrative-section checkboxes fail, section transitions reset correctly, and task counting uses the same classification.
- Apply-loop tests prove invalid `tasks.md` blocks acceptance handoff and supplies deterministic diagnostics for repair, while valid task files preserve the existing handoff.
- Embedded skill and prompt contract tests prove proposal/apply guidance names the canonical formatting rules and exact acceptance evidence form.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, and the relevant default-path Rust tests pass.
- `cflx openspec validate prevent-task-format-retry-loops --archive-gate` exits successfully.

## Out of Scope

- Automatically rewriting arbitrary Markdown bullets outside runtime-owned acceptance follow-up sections.
- Weakening active-section bare-task detection.
- Changing acceptance verdict semantics, retry limits, or archive behavior unrelated to task formatting.
- Introducing external or durable workflow state.

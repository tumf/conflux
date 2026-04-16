---
change_type: implementation
priority: high
dependencies: []
references:
  - src/agent/prompt.rs
  - src/templates.rs
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# refine-acceptance-archive-commitability

**Change Type**: implementation

## Problem / Context

Conflux currently requires acceptance to gate archive handoff, but the prompt builder hardcodes Rust-specific archive-readiness instructions such as `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`.

That behavior conflicts with the intended architecture of Conflux:

- Conflux must work across arbitrary language stacks and repository conventions.
- The existence of tests, linters, formatters, or pre-commit hooks is repository-specific.
- The archive-phase concern that acceptance must actually protect is not generic quality enforcement; it is whether the archive flow can successfully perform the required final commit.

Because archive does not normally route back to apply, acceptance must still catch commit-blocking archive failures before archive starts. However, it must do so without inventing hardcoded toolchain gates.

## Proposed Solution

Refine acceptance archive-readiness so that acceptance guarantees **archive commitability** rather than generic quality-gate completion.

The implementation should:

1. Keep the requirement that acceptance blocks archive when the workspace is not ready for the final archive commit.
2. Redefine readiness in terms of the **actual blockers on the archive commit path**.
3. Remove hardcoded Rust/toolchain-specific gate instructions from Conflux core prompt construction.
4. Keep fixed prompt content minimal in core prompt builders:
   - `load skills: cflx-*`
   - operation metadata / paths / revision context
   - machine-readable protocol details
5. Allow repository-specific gate logic only when it is part of the real commit path, rather than as independent inferred checks.

Examples:

- If a repository has no commit hook, acceptance only needs to ensure the archive commit can succeed.
- If normal commit execution triggers a pre-commit hook and that hook blocks the archive commit, acceptance should treat that commit-path failure as relevant.
- Acceptance must not independently require `test`, `lint`, or `format` unless they are part of the actual blocking commit path being validated.

## Acceptance Criteria

- The proposal updates canonical behavior so acceptance protects archive handoff by validating commitability, not hardcoded repo-wide quality gates.
- The proposal removes the assumption that pre-commit, test, lint, or format gates exist in every target repository.
- The proposal requires Conflux core prompt builders to remain architecture-agnostic and language-agnostic.
- The proposal preserves the rule that acceptance must stop archive before archive-phase commit blockers surface too late.
- The proposal is strict-valid and ready for later implementation work in prompt-building, templates, and tests.

## Out of Scope

- Implementing the prompt-builder/runtime changes in this proposal.
- Redesigning archive flow to introduce a new archive-to-apply recovery path.
- Defining repository-specific commitability heuristics beyond what the canonical specs require.
